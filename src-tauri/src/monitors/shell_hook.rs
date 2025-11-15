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

    // Default based on platform
    #[cfg(target_os = "macos")]
    {
        Ok(ShellType::Zsh)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(ShellType::Bash)
    }
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

/// Get the path to the LocalMind CLI script
fn get_cli_script_path() -> String {
    // Try multiple locations to find the script
    // 1. Relative to current directory (development)
    if let Ok(current_dir) = std::env::current_dir() {
        let script_path = current_dir.join("src-tauri").join("localmind-cli.sh");
        if script_path.exists() {
            return script_path.display().to_string();
        }
        // Try in data directory (production)
        let script_path = current_dir.join("data").join("local-mind").join("localmind-cli.sh");
        if script_path.exists() {
            return script_path.display().to_string();
        }
    }
    // 2. Try to get executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let script_path = exe_dir.join("localmind-cli.sh");
            if script_path.exists() {
                return script_path.display().to_string();
            }
        }
    }
    // Fallback: use a path that will be resolved at runtime
    // The script should be copied to a known location during installation
    "$HOME/.localmind/localmind-cli.sh".to_string()
}

/// Parse a shortcut string (e.g., "Alt+C", "Ctrl+Shift+C") to shell-specific key binding
/// Returns (binding_string, comment) for the given shell
fn parse_shortcut_to_binding(shortcut: &str, shell: ShellType) -> (String, String) {
    let shortcut_lower = shortcut.to_lowercase();
    let parts: Vec<&str> = shortcut_lower.split('+').map(|s| s.trim()).collect();
    
    // Extract modifiers and key
    let mut has_ctrl = false;
    let mut has_shift = false;
    let mut has_alt = false;
    let mut key = None;
    
    for part in parts {
        match part {
            "ctrl" | "control" => has_ctrl = true,
            "shift" => has_shift = true,
            "alt" | "option" | "meta" => has_alt = true,
            _ => {
                if key.is_none() {
                    key = Some(part);
                }
            }
        }
    }
    
    let key_char = key.unwrap_or("c");
    let key_upper = key_char.to_uppercase();
    
    match shell {
        ShellType::Zsh => {
            if has_ctrl && has_shift {
                // Ctrl+Shift+C -> '^[C' (escape sequence for Ctrl+Shift)
                // Note: This requires terminal to send proper escape sequence
                // Alternative: Use Ctrl+; which is more reliable
                if key_char == "c" {
                    // Use Ctrl+; instead to avoid conflict with interrupt
                    (r"'^;'".to_string(), "Ctrl+; (works on macOS without config)".to_string())
                } else {
                    (format!(r"'^[{}'", key_upper), format!("Ctrl+Shift+{}", key_upper))
                }
            } else if has_ctrl {
                // Ctrl+C -> '^C' (but conflicts with interrupt, so use Ctrl+;)
                if key_char == "c" {
                    (r"'^;'".to_string(), "Ctrl+; (default, works on macOS)".to_string())
                } else {
                    (format!(r"'^{}'", key_char), format!("Ctrl+{}", key_upper))
                }
            } else if has_alt {
                // Alt+C -> '^[c' (requires terminal config on macOS)
                (format!(r"'^[{}'", key_char), format!("Alt+{} (requires 'Use Option as Meta' in terminal)", key_upper))
            } else {
                // Fallback to Ctrl+;
                (r"'^;'".to_string(), "Ctrl+; (fallback)".to_string())
            }
        }
        ShellType::Bash => {
            if has_ctrl && has_shift {
                if key_char == "c" {
                    // Use Ctrl+; to avoid conflict
                    (r#"\C-;"#.to_string(), "Ctrl+; (works on macOS without config)".to_string())
                } else {
                    (format!(r#"\e{}"#, key_upper), format!("Ctrl+Shift+{}", key_upper))
                }
            } else if has_ctrl {
                if key_char == "c" {
                    (r#"\C-;"#.to_string(), "Ctrl+; (default, works on macOS)".to_string())
                } else {
                    (format!(r#"\C-{}"#, key_char), format!("Ctrl+{}", key_upper))
                }
            } else if has_alt {
                (format!(r#"\e{}"#, key_char), format!("Alt+{} (requires terminal config)", key_upper))
            } else {
                (r#"\C-;"#.to_string(), "Ctrl+; (fallback)".to_string())
            }
        }
        ShellType::Fish => {
            if has_ctrl && has_shift {
                if key_char == "c" {
                    (r"\c;".to_string(), "Ctrl+; (works on macOS without config)".to_string())
                } else {
                    (format!(r"\e{}", key_upper), format!("Ctrl+Shift+{}", key_upper))
                }
            } else if has_ctrl {
                if key_char == "c" {
                    (r"\c;".to_string(), "Ctrl+; (default, works on macOS)".to_string())
                } else {
                    (format!(r"\c{}", key_char), format!("Ctrl+{}", key_upper))
                }
            } else if has_alt {
                (format!(r"\e{}", key_char), format!("Alt+{} (requires terminal config)", key_upper))
            } else {
                (r"\c;".to_string(), "Ctrl+; (fallback)".to_string())
            }
        }
    }
}

/// Generate shell hook code for the given shell type
fn generate_hook_code(shell: ShellType, shortcut: &str) -> String {
    let log_path = match get_terminal_log_path() {
        Ok(path) => path.display().to_string(),
        Err(_) => "$HOME/.localmind/terminal.log".to_string(),
    };
    
    let cli_path = get_cli_script_path();
    
    // Parse shortcut to shell-specific binding
    let (binding, binding_comment) = parse_shortcut_to_binding(shortcut, shell);

    match shell {
        ShellType::Zsh => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
__lm_log="{}"
__lm_cli="{}"

preexec() {{
  echo "$(date +%s)|CMD_START|$PWD|$1" >> "$__lm_log"
}}

precmd() {{
  local exit_code=$?
  echo "$(date +%s)|CMD_END|$PWD|$exit_code" >> "$__lm_log"
}}

# LocalMind Command Picker
lm-pick-command() {{
  local cmd
  if command -v fzf &> /dev/null; then
    # Use fzf if available (better UX with real-time search)
    # fzf shows dropdown, user types to filter, selects with Enter
    cmd=$("$__lm_cli" --cwd "$PWD" 2>/dev/null | fzf --height=40% --reverse --header="LocalMind: Select a command (type to search)" --prompt="> " --bind "enter:accept")
  else
    # Fallback to simple select menu
    local commands
    mapfile -t commands < <("$__lm_cli" --cwd "$PWD" 2>/dev/null)
    if [ ${{#commands[@]}} -eq 0 ]; then
      echo "No commands found in LocalMind"
      return 1
    fi
    select cmd in "${{commands[@]}}"; do
      [ -n "$cmd" ] && break
    done
  fi
  
  if [ -n "$cmd" ]; then
    # Insert command into buffer and execute immediately
    BUFFER="$cmd"
    CURSOR=${{#BUFFER}}
    zle accept-line  # Execute the command
  fi
}}
zle -N lm-pick-command
# Bind to {} - {}
bindkey {} lm-pick-command
# === End LocalMind Monitor ===
"#,
            log_path, cli_path, binding_comment, binding, binding
        ),

        ShellType::Bash => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
__lm_log="{}"
__lm_cli="{}"

__lm_preexec() {{
  echo "$(date +%s)|CMD_START|$PWD|$BASH_COMMAND" >> "$__lm_log"
}}

__lm_precmd() {{
  local exit_code=$?
  echo "$(date +%s)|CMD_END|$PWD|$exit_code" >> "$__lm_log"
}}

PROMPT_COMMAND='__lm_precmd'
trap '__lm_preexec' DEBUG

# LocalMind Command Picker
lm-pick-command() {{
  local cmd
  if command -v fzf &> /dev/null; then
    # Use fzf if available (better UX with real-time search)
    # fzf shows dropdown, user types to filter, selects with Enter
    cmd=$("$__lm_cli" --cwd "$PWD" 2>/dev/null | fzf --height=40% --reverse --header="LocalMind: Select a command (type to search)" --prompt="> " --bind "enter:accept")
  else
    local commands
    mapfile -t commands < <("$__lm_cli" --cwd "$PWD" 2>/dev/null)
    if [ ${{#commands[@]}} -eq 0 ]; then
      echo "No commands found in LocalMind"
      return 1
    fi
    select cmd in "${{commands[@]}}"; do
      [ -n "$cmd" ] && break
    done
  fi
  
  if [ -n "$cmd" ]; then
    # Insert command into buffer and execute immediately
    # For bash, we need to add to history and execute
    history -s "$cmd"
    # Execute the command directly
    eval "$cmd"
  fi
}}
# Bind to {} - {}
bind -x '"{}": lm-pick-command'
# === End LocalMind Monitor ===
"#,
            log_path, cli_path, binding_comment, binding, binding
        ),

        ShellType::Fish => format!(
            r#"
# === LocalMind Terminal Monitor (Auto-generated) ===
set __lm_log "{}"
set __lm_cli "{}"

function __lm_preexec --on-event fish_preexec
  echo (date +%s)"|CMD_START|$PWD|$argv" >> $__lm_log
end

function __lm_postexec --on-event fish_postexec
  echo (date +%s)"|CMD_END|$PWD|$status" >> $__lm_log
end

# LocalMind Command Picker
function lm-pick-command
  set -l cmd
  if command -v fzf > /dev/null 2>&1
    # Use fzf if available (better UX with real-time search)
    # fzf shows dropdown, user types to filter, selects with Enter
    set cmd ($__lm_cli --cwd $PWD 2>/dev/null | fzf --height=40% --reverse --header="LocalMind: Select a command (type to search)" --prompt="> " --bind "enter:accept")
  else
    set -l commands ($__lm_cli --cwd $PWD 2>/dev/null)
    if [ (count $commands) -eq 0 ]
      echo "No commands found in LocalMind"
      return 1
    end
    set -l choice (printf '%s\n' $commands | nl -w2 -s'. ')
    read -p 'echo "Select command: "' choice_num
    set cmd $commands[$choice_num]
  end
  
  if [ -n "$cmd" ]
    # Insert command and execute immediately
    commandline -r "$cmd"
    commandline -f execute
  end
end
# Bind to {} - {}
bind {} lm-pick-command
# === End LocalMind Monitor ===
"#,
            log_path, cli_path, binding_comment, binding, binding
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
    // Get shortcut from settings
    let shortcut = crate::settings::get_cached_settings()
        .map(|s| s.command_picker_shortcut.clone())
        .unwrap_or_else(|| "Ctrl+R".to_string());
    
    install_hooks_with_shortcut(shell, &shortcut)
}

/// Install monitoring hooks with a specific shortcut
pub fn install_hooks_with_shortcut(shell: ShellType, shortcut: &str) -> Result<String> {
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

    // Generate hook code with shortcut
    let hook_code = generate_hook_code(shell, shortcut);

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
        let zsh_code = generate_hook_code(ShellType::Zsh, "Ctrl+Shift+C");
        assert!(zsh_code.contains("preexec"));
        assert!(zsh_code.contains("precmd"));

        let bash_code = generate_hook_code(ShellType::Bash, "Ctrl+Shift+C");
        assert!(bash_code.contains("PROMPT_COMMAND"));
        assert!(bash_code.contains("trap"));

        let fish_code = generate_hook_code(ShellType::Fish, "Ctrl+Shift+C");
        assert!(fish_code.contains("fish_preexec"));
        assert!(fish_code.contains("fish_postexec"));
    }
}
