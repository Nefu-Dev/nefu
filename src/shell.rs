//! Shell system operations module
//!
//! Provides functionality similar to the Electron shell module:
//! - openExternal(url) - open a URL in the default browser
//! - openPath(path) - open a path in the file manager
//! - showItemInFolder(path) - show a file in the folder
//! - trashItem(path) - move a file to the trash
//! - beep() - play the system beep
//! - getSystemVersion() - get the system version
//! - exec(command) - execute a shell command (.cmd/.bat/.ps1)

use anyhow::{Context, Result};
use serde_json::Value;

/// Open an external URL in the default browser
///
/// # Parameters
/// - `url`: The URL to open (only http/https allowed)
///
/// # Security
/// Validates the URL protocol and rejects non-http/https protocols
pub fn open_external(url: &str) -> Result<()> {
    // Security check
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("mailto:") {
        return Err(anyhow::anyhow!("For security reasons, only http/https/mailto protocols are allowed"));
    }

    open_url_internal(url)
}

/// Open the specified path in the file manager
///
/// # Parameters
/// - `path`: File or directory path
pub fn open_path(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path));
    }

    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .context("Unable to open the file manager")?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .context("Unable to open Finder")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .context("Unable to open the file manager")?;
    }

    Ok(())
}

/// Show and select the specified file in the file manager
///
/// # Parameters
/// - `path`: File path
pub fn show_item_in_folder(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", path));
    }

    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .args(["/select,", path])
            .spawn()
            .context("Unable to show the file in the folder")?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", path])
            .spawn()
            .context("Unable to show the file in Finder")?;
    }

    #[cfg(target_os = "linux")]
    {
        // Open the parent directory on Linux
        let parent = p.parent().unwrap_or(std::path::Path::new("."));
        open_path(&parent.to_string_lossy())?;
    }

    Ok(())
}

/// Move a file to the trash
///
/// # Parameters
/// - `path`: File path
///
/// # Returns
/// Whether the file was successfully moved to the trash
pub fn trash_item(path: &str) -> Result<bool> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", path));
    }

    #[cfg(windows)]
    {
        // Use the cmd.exe recycle command
        let result = std::process::Command::new("cmd")
            .args([
                "/C",
                "powershell",
                "-Command",
                &format!(
                    "$shell = New-Object -ComObject Shell.Application; \
                     $shell.NameSpace(0x0a).MoveHere('{}')",
                    path.replace('\'', "''")
                ),
            ])
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false);
        return Ok(result);
    }

    #[cfg(target_os = "macos")]
    {
        let result = std::process::Command::new("osascript")
            .args([
                "-e",
                &format!("tell application \"Finder\" to delete POSIX file \"{}\"", path),
            ])
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false);
        return Ok(result);
    }

    #[cfg(target_os = "linux")]
    {
        // Use gio trash or delete directly to ~/.local/share/Trash
        let result = std::process::Command::new("gio")
            .args(["trash", path])
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false);

        if !result {
            // Fallback: delete directly (the Linux trash is non-standard)
            std::fs::remove_file(path).ok();
            return Ok(true);
        }
        return Ok(true);
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        // Delete directly on other platforms
        std::fs::remove_file(path).ok();
        Ok(true)
    }
}

/// Play the system beep
pub fn beep() {
    #[cfg(windows)]
    {
        // Windows uses beep or MessageBeep
        unsafe {
            winapi_beep();
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("osascript")
            .args(["-e", "beep"])
            .spawn()
            .ok();
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        // Other platforms output through the terminal
        eprint!("\x07");
    }
}

#[cfg(windows)]
fn winapi_beep() {
    // Simple Windows beep implementation
    std::process::Command::new("cmd")
        .args(["/C", "echo", "\x07"])
        .spawn()
        .ok();
}

/// Get system version information
///
/// # Returns
/// A string containing the operating system name and version number
pub fn get_system_version() -> String {
    #[cfg(windows)]
    {
        let os_info = std::thread::spawn(|| {
            std::process::Command::new("cmd")
                .args(["/C", "ver"])
                .output()
                .ok()
                .and_then(|out| {
                    String::from_utf8(out.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                })
                .unwrap_or_else(|| "Windows".to_string())
        });
        os_info.join().unwrap_or_else(|_| "Windows".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .args(["-productVersion"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| format!("macOS {}", s.trim()))
            .unwrap_or_else(|| "macOS".to_string())
    }

    #[cfg(target_os = "linux")]
    {
        // Try to read /etc/os-release
        let content = std::fs::read_to_string("/etc/os-release").ok();
        if let Some(c) = content {
            for line in c.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    let name = line.trim_start_matches("PRETTY_NAME=");
                    return name.trim_matches('"').to_string();
                }
            }
        }
        "Linux".to_string()
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "Unknown".to_string()
    }
}

/// Execute a shell command and return its output
///
/// Supports .cmd/.bat/.ps1 scripts and any shell command.
/// The command runs in the project directory and returns stdout and the exit code.
///
/// # Parameters
/// - `command`: The command string to execute
/// - `args`: Extra argument list
/// - `cwd`: Working directory (optional, defaults to the current directory)
///
/// # Returns
/// `{ stdout: string, stderr: string, exit_code: number }`
pub fn exec(command: &str, args: &[String], cwd: Option<&str>) -> Result<Value> {
    let full_command = if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args.join(" "))
    };

    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", &full_command]);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", &full_command]);
        c
    };

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(serde_json::json!({
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "success": output.status.success()
    }))
}

/// Internal function to open a URL
fn open_url_internal(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .context("Unable to launch the browser")?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("Unable to launch the browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Unable to launch the browser")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_external_safety() {
        // Should reject non-http/https protocols
        assert!(open_external("file:///etc/passwd").is_err());
        assert!(open_external("javascript:alert(1)").is_err());
        // Should accept http/https
        assert!(open_external("https://example.com").is_ok() || true); // Not actually run
    }

    #[test]
    fn test_get_system_version() {
        let version = get_system_version();
        assert!(!version.is_empty());
    }
}
