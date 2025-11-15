use anyhow::{Context, Result};
use std::path::PathBuf;
use std::fs;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Zsh,
    Bash,
    Fish,
}

impl ShellType {
    pub fn as_str(&self) -> &str {
        match self {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::Fish => "fish",
        }
    }
}

/// Detect the user's current shell
pub fn detect_shell() -> Result<ShellType> {
    // Check SHELL environment variable
    if let Ok(shell_path) = env::var("SHELL") {
        if shell_path.contains("zsh") {
            return Ok(ShellType::Zsh);
        } else if shell_path.contains("bash") {
            return Ok(ShellType::Bash);
        } else if shell_path.contains("fish") {
            return Ok(ShellType::Fish);
        }
    }

    // Default to zsh on macOS
    #[cfg(target_os = "macos")]
    {
        return Ok(ShellType::Zsh);
    }

    // Default to bash on Linux
    #[cfg(target_os = "linux")]
    {
        return Ok(ShellType::Bash);
    }

    // Fallback
    Ok(ShellType::Bash)
}

/// Get the shell config file path for a given shell type
pub fn get_shell_config_path(shell: ShellType) -> Result<PathBuf> {
    let home = env::var("HOME")
        .context("HOME environment variable not set")?;

    let config_file = match shell {
        ShellType::Zsh => ".zshrc",
        ShellType::Bash => ".bashrc",
        ShellType::Fish => ".config/fish/config.fish",
    };

    Ok(PathBuf::from(home).join(config_file))
}

/// Get the LocalMind log directory path
pub fn get_log_dir() -> Result<PathBuf> {
    let home = env::var("HOME")
        .context("HOME environment variable not set")?;

    let log_dir = PathBuf::from(home).join(".localmind");

    // Ensure directory exists
    fs::create_dir_all(&log_dir)
        .context("Failed to create .localmind directory")?;

    Ok(log_dir)
}

/// Get the terminal log file path
pub fn get_terminal_log_path() -> Result<PathBuf> {
    Ok(get_log_dir()?.join("terminal.log"))
}

/// Generate shell hook code for the given shell type
fn generate_hook_code(shell: ShellType) -> String {
    let log_path = match get_terminal_log_path() {
        Ok(path) => path.display().to_string(),
        Err(_) => "$HOME/.localmind/terminal.log".to_string(),
    };

    match shell {
        ShellType::Zsh => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
__lm_log="{}"

preexec() {{
  echo "$(date +%s)|CMD_START|$PWD|$1" >> "$__lm_log"
}}

precmd() {{
  local exit_code=$?
  echo "$(date +%s)|CMD_END|$PWD|$exit_code" >> "$__lm_log"
}}
# === End LocalMind Monitor ===
"#,
            log_path
        ),

        ShellType::Bash => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
__lm_log="{}"

__lm_preexec() {{
  echo "$(date +%s)|CMD_START|$PWD|$BASH_COMMAND" >> "$__lm_log"
}}

__lm_precmd() {{
  local exit_code=$?
  echo "$(date +%s)|CMD_END|$PWD|$exit_code" >> "$__lm_log"
}}

PROMPT_COMMAND='__lm_precmd'
trap '__lm_preexec' DEBUG
# === End LocalMind Monitor ===
"#,
            log_path
        ),

        ShellType::Fish => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
set __lm_log "{}"

function __lm_preexec --on-event fish_preexec
  echo (date +%s)"|CMD_START|$PWD|$argv" >> $__lm_log
end

function __lm_postexec --on-event fish_postexec
  echo (date +%s)"|CMD_END|$PWD|$status" >> $__lm_log
end
# === End LocalMind Monitor ===
"#,
            log_path
        ),
    }
}

/// Check if hooks are already installed in the config file
pub fn are_hooks_installed(shell: ShellType) -> Result<bool> {
    let config_path = get_shell_config_path(shell)?;

    if !config_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read shell config file")?;

    Ok(content.contains("LocalMind Terminal Monitor"))
}

/// Install monitoring hooks into the user's shell config
pub fn install_hooks(shell: ShellType) -> Result<String> {
    let config_path = get_shell_config_path(shell)?;

    // Create config file if it doesn't exist
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }
        fs::write(&config_path, "")
            .context("Failed to create config file")?;
    }

    // Check if already installed
    if are_hooks_installed(shell)? {
        return Ok(format!(
            "Hooks already installed in {}",
            config_path.display()
        ));
    }

    // Create backup
    let backup_path = config_path.with_extension("backup");
    fs::copy(&config_path, &backup_path)
        .context("Failed to create backup of shell config")?;

    // Read existing content
    let existing_content = fs::read_to_string(&config_path)
        .context("Failed to read shell config file")?;

    // Generate hook code
    let hook_code = generate_hook_code(shell);

    // Append hooks to config
    let new_content = format!("{}\n{}", existing_content, hook_code);
    fs::write(&config_path, new_content)
        .context("Failed to write hooks to shell config")?;

    // Ensure log file exists
    let log_path = get_terminal_log_path()?;
    if !log_path.exists() {
        fs::write(&log_path, "")
            .context("Failed to create terminal log file")?;
    }

    log::info!("✅ Installed shell hooks in {}", config_path.display());
    log::info!("   Backup created at {}", backup_path.display());
    log::info!("   Log file: {}", log_path.display());

    Ok(format!(
        "Hooks installed successfully in {}. Please restart your terminal or run: source {}",
        config_path.display(),
        config_path.display()
    ))
}

/// Uninstall monitoring hooks from the user's shell config
pub fn uninstall_hooks(shell: ShellType) -> Result<String> {
    let config_path = get_shell_config_path(shell)?;

    if !config_path.exists() {
        return Ok("Shell config file does not exist".to_string());
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read shell config file")?;

    if !content.contains("LocalMind Terminal Monitor") {
        return Ok("Hooks not installed".to_string());
    }

    // Remove hook section
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut in_hook_section = false;

    for line in lines {
        if line.contains("=== LocalMind Terminal Monitor") {
            in_hook_section = true;
            continue;
        }
        if line.contains("=== End LocalMind Monitor ===") {
            in_hook_section = false;
            continue;
        }
        if !in_hook_section {
            new_lines.push(line);
        }
    }

    let new_content = new_lines.join("\n");
    fs::write(&config_path, new_content)
        .context("Failed to write updated shell config")?;

    log::info!("✅ Uninstalled shell hooks from {}", config_path.display());

    Ok(format!(
        "Hooks uninstalled successfully from {}",
        config_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        let shell = detect_shell();
        assert!(shell.is_ok());
    }

    #[test]
    fn test_generate_hook_code() {
        let zsh_code = generate_hook_code(ShellType::Zsh);
        assert!(zsh_code.contains("preexec"));
        assert!(zsh_code.contains("precmd"));

        let bash_code = generate_hook_code(ShellType::Bash);
        assert!(bash_code.contains("PROMPT_COMMAND"));
        assert!(bash_code.contains("trap"));

        let fish_code = generate_hook_code(ShellType::Fish);
        assert!(fish_code.contains("fish_preexec"));
        assert!(fish_code.contains("fish_postexec"));
    }
}
