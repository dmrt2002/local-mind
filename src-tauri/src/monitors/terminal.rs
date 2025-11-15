use anyhow::{Context, Result};
use log;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

use crate::db::sqlite;
use crate::job_queue::{PersistentJobQueue, Priority};
use crate::monitors::shell_hook::get_terminal_log_path;

#[derive(Debug, Clone)]
pub struct CommandFilter {
    pub blocklist: HashSet<String>,
    pub allowlist: HashSet<String>,
    pub min_length: usize,
}

impl CommandFilter {
    pub fn new(
        blocklist: Vec<String>,
        allowlist: Vec<String>,
        min_length: usize,
    ) -> Self {
        Self {
            blocklist: blocklist.into_iter().collect(),
            allowlist: allowlist.into_iter().collect(),
            min_length,
        }
    }

    pub fn default() -> Self {
        Self {
            blocklist: HashSet::from([
                "ls".to_string(),
                "cd".to_string(),
                "pwd".to_string(),
                "clear".to_string(),
                "exit".to_string(),
                "history".to_string(),
                "which".to_string(),
                "type".to_string(),
            ]),
            allowlist: HashSet::from([
                "docker".to_string(),
                "git".to_string(),
                "kubectl".to_string(),
                "npm".to_string(),
                "cargo".to_string(),
                "python".to_string(),
                "python3".to_string(),
                "node".to_string(),
                "ffmpeg".to_string(),
                "curl".to_string(),
                "wget".to_string(),
                "aws".to_string(),
                "gcloud".to_string(),
                "az".to_string(),
                "terraform".to_string(),
                "ansible".to_string(),
                "ssh".to_string(),
                "scp".to_string(),
                "rsync".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "bun".to_string(),
                "go".to_string(),
                "rust".to_string(),
                "rustc".to_string(),
                "javac".to_string(),
                "java".to_string(),
                "gcc".to_string(),
                "make".to_string(),
                "cmake".to_string(),
                "mvn".to_string(),
                "gradle".to_string(),
                "pytest".to_string(),
                "jest".to_string(),
                "mocha".to_string(),
                "rails".to_string(),
                "rake".to_string(),
                "bundle".to_string(),
                "composer".to_string(),
                "php".to_string(),
                "perl".to_string(),
                "ruby".to_string(),
            ]),
            min_length: 30,
        }
    }

    pub fn should_save(&self, command: &str) -> bool {
        let trimmed = command.trim();

        if trimmed.is_empty() {
            return false;
        }

        // Get the base command (first word)
        let cmd_base = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("");

        // 1. Skip if in blocklist
        if self.blocklist.contains(cmd_base) {
            return false;
        }

        // 2. Always save if in allowlist
        if self.allowlist.contains(cmd_base) {
            return true;
        }

        // 3. Apply heuristics for complex or interesting commands
        // - Long commands (reduced threshold from 60 to 30)
        // - Commands with pipes, logical operators, or redirects
        // - Commands with sudo
        // - Commands with multiple arguments (likely intentional)
        // - Commands with flags/options
        trimmed.len() > self.min_length
            || trimmed.contains('|')
            || trimmed.contains("&&")
            || trimmed.contains("||")
            || trimmed.contains(">>")
            || trimmed.contains(">")
            || trimmed.starts_with("sudo")
            || trimmed.split_whitespace().count() >= 3
            || trimmed.contains("--")
            || (trimmed.contains('-') && trimmed.split_whitespace().count() >= 2)
    }
}

#[derive(Debug, Clone)]
struct PendingCommand {
    command: String,
    working_directory: String,
    timestamp: i64,
}

pub struct TerminalMonitor {
    log_path: PathBuf,
    filter: Arc<CommandFilter>,
    pending_command: Arc<Mutex<Option<PendingCommand>>>,
    last_position: Arc<Mutex<u64>>,
    job_queue: Arc<PersistentJobQueue>,
    dedup_cache: Arc<Mutex<HashMap<String, Instant>>>,
}

impl TerminalMonitor {
    pub fn new(job_queue: Arc<PersistentJobQueue>, filter: CommandFilter) -> Result<Self> {
        let log_path = get_terminal_log_path()?;

        Ok(Self {
            log_path,
            filter: Arc::new(filter),
            pending_command: Arc::new(Mutex::new(None)),
            last_position: Arc::new(Mutex::new(0)),
            job_queue,
            dedup_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Start monitoring the terminal log file
    pub async fn start_monitoring(&self) -> Result<()> {
        log::info!("🔍 Starting terminal monitoring");
        log::info!("   Log file: {}", self.log_path.display());

        // Ensure log file exists
        if !self.log_path.exists() {
            std::fs::write(&self.log_path, "")
                .context("Failed to create terminal log file")?;
        }

        // Start file monitoring loop
        let log_path = self.log_path.clone();
        let filter = self.filter.clone();
        let pending_command = self.pending_command.clone();
        let last_position = self.last_position.clone();
        let job_queue = self.job_queue.clone();
        let dedup_cache = self.dedup_cache.clone();

        tokio::spawn(async move {
            loop {
                // Read new lines from log file
                if let Err(e) = Self::process_log_updates(
                    &log_path,
                    &filter,
                    &pending_command,
                    &last_position,
                    &job_queue,
                    &dedup_cache,
                )
                .await
                {
                    log::error!("Error processing terminal log: {}", e);
                }

                // Check every 500ms
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Clean old entries from dedup cache (older than 5 minutes)
                let mut cache = dedup_cache.lock().await;
                cache.retain(|_, instant| instant.elapsed().as_secs() < 300);
            }
        });

        Ok(())
    }

    async fn process_log_updates(
        log_path: &PathBuf,
        filter: &Arc<CommandFilter>,
        pending_command: &Arc<Mutex<Option<PendingCommand>>>,
        last_position: &Arc<Mutex<u64>>,
        job_queue: &Arc<PersistentJobQueue>,
        dedup_cache: &Arc<Mutex<HashMap<String, Instant>>>,
    ) -> Result<()> {
        use std::io::{Read, Seek, SeekFrom};
        use std::fs::File;

        let mut file = File::open(log_path)?;
        let file_len = file.metadata()?.len();

        let mut pos = last_position.lock().await;

        // If file was truncated, reset position
        if *pos > file_len {
            *pos = 0;
        }

        // Seek to last read position
        file.seek(SeekFrom::Start(*pos))?;

        // Read new content
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;

        // Update position
        *pos = file_len;
        drop(pos);

        // Process new lines
        for line in buffer.lines() {
            if let Err(e) = Self::process_log_line(
                line,
                filter,
                pending_command,
                job_queue,
                dedup_cache,
            )
            .await
            {
                log::error!("Error processing log line: {}", e);
            }
        }

        Ok(())
    }

    async fn process_log_line(
        line: &str,
        filter: &Arc<CommandFilter>,
        pending_command: &Arc<Mutex<Option<PendingCommand>>>,
        job_queue: &Arc<PersistentJobQueue>,
        _dedup_cache: &Arc<Mutex<HashMap<String, Instant>>>,
    ) -> Result<()> {
        let parts: Vec<&str> = line.split('|').collect();

        if parts.len() < 4 {
            return Ok(()); // Invalid line format
        }

        let timestamp: i64 = parts[0].parse().unwrap_or(0);
        let event_type = parts[1];
        let cwd = parts[2];
        let data = parts[3];

        match event_type {
            "CMD_START" => {
                // Store pending command
                let mut pending = pending_command.lock().await;
                *pending = Some(PendingCommand {
                    command: data.to_string(),
                    working_directory: cwd.to_string(),
                    timestamp,
                });
            }
            "CMD_END" => {
                // Match with pending command and save if passes filter
                let mut pending = pending_command.lock().await;

                if let Some(cmd) = pending.take() {
                    let exit_code: i32 = data.parse().unwrap_or(-1);

                    // Apply filter
                    if filter.should_save(&cmd.command) {
                        // Save command (deduplication is handled in save_command)
                        Self::save_command(cmd, exit_code, job_queue).await?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn save_command(
        cmd: PendingCommand,
        exit_code: i32,
        job_queue: &Arc<PersistentJobQueue>,
    ) -> Result<()> {
        use chrono::Utc;

        let pool = sqlite::get_pool().await?;
        let now = Utc::now();

        // Create a unique identifier combining command and working directory
        let unique_content = format!("{}:{}", cmd.command, cmd.working_directory);
        let content_hash = crate::dedup::calculate_content_hash(&unique_content);

        // Check if this exact command in this directory already exists
        let existing_id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT id FROM snippets
            WHERE type = 'command' AND content_hash = ?
            LIMIT 1
            "#,
        )
        .bind(&content_hash)
        .fetch_optional(&pool)
        .await
        .context("Failed to check for duplicate command")?;

        if let Some(existing_id) = existing_id {
            log::info!("⏭️  Command already exists (ID: {}), skipping duplicate: {}", existing_id, cmd.command);
            return Ok(());
        }

        // Generate summary for the command
        let summary = crate::db::sqlite::generate_summary(&cmd.command);

        // Save to snippets table with summary and content_hash using INSERT OR IGNORE
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO snippets (content, summary, type, working_directory, exit_code, created_at, updated_at, content_hash)
            VALUES (?, ?, 'command', ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&cmd.command)
        .bind(&summary)
        .bind(&cmd.working_directory)
        .bind(exit_code)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(&content_hash)
        .execute(&pool)
        .await
        .context("Failed to insert command snippet")?;

        // Check if insert was ignored (duplicate detected)
        let snippet_id = if result.rows_affected() == 0 {
            // Duplicate detected - fetch existing ID
            log::info!("⏭️  Command duplicate detected (race condition), fetching existing ID");
            let existing_id: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT id FROM snippets
                WHERE type = 'command' AND content_hash = ?
                LIMIT 1
                "#,
            )
            .bind(&content_hash)
            .fetch_optional(&pool)
            .await
            .context("Failed to fetch existing command ID")?;

            match existing_id {
                Some(id) => {
                    log::info!("✅ Using existing command snippet ID: {}", id);
                    id
                }
                None => {
                    anyhow::bail!("Command duplicate detected but existing ID not found");
                }
            }
        } else {
            // Insert succeeded - get the ID
            result.last_insert_rowid()
        };

        log::info!("✅ Saved command {}: {}", snippet_id, cmd.command);

        // Queue for embedding (uses existing all-MiniLM-L6-v2 model)
        job_queue
            .push(
                snippet_id,
                cmd.command.clone(),
                Some(summary), // Use the generated summary instead of truncated command
                Priority::Normal,
            )
            .await?;

        Ok(())
    }

    pub fn update_filter(&mut self, filter: CommandFilter) {
        self.filter = Arc::new(filter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_filter_blocklist() {
        let filter = CommandFilter::default();

        assert!(!filter.should_save("ls"));
        assert!(!filter.should_save("cd /home"));
        assert!(!filter.should_save("pwd"));
        assert!(!filter.should_save("clear"));
    }

    #[test]
    fn test_command_filter_allowlist() {
        let filter = CommandFilter::default();

        assert!(filter.should_save("docker ps"));
        assert!(filter.should_save("git status"));
        assert!(filter.should_save("kubectl get pods"));
        assert!(filter.should_save("npm install"));
    }

    #[test]
    fn test_command_filter_heuristics() {
        let filter = CommandFilter::default();

        // Long command
        assert!(filter.should_save(&"x".repeat(40)));

        // Pipes
        assert!(filter.should_save("cat file.txt | grep pattern"));

        // Logical operators
        assert!(filter.should_save("mkdir test && cd test"));
        assert!(filter.should_save("command1 || command2"));

        // Redirects
        assert!(filter.should_save("echo hello > file.txt"));
        assert!(filter.should_save("cat file >> output.log"));

        // Sudo
        assert!(filter.should_save("sudo apt update"));

        // Commands with multiple arguments
        assert!(filter.should_save("mv file1.txt file2.txt"));

        // Commands with flags
        assert!(filter.should_save("rm -rf temp"));
        assert!(filter.should_save("find . --name test"));
    }
}
