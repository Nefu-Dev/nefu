//! App module
//!
//! Provides functionality similar to the Electron app module:
//! - getPath(name) - get a system path
//! - getAppPath() - get the app path
//! - getName() / getVersion() - app info
//! - quit() / exit() - quit the app
//! - relaunch() - relaunch the app
//! - focus() - focus the window
//! - getAppMetrics() - process memory stats
//! - getLocale() / getLocaleCountryCode() - locale info
//! - isPackaged - whether the app is packaged
//! - getLoginItemSettings() / setLoginItemSettings() - login items

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    /// App name
    pub name: String,
    /// Version
    pub version: String,
    /// Whether the app is packaged
    pub is_packaged: bool,
    /// App path
    pub app_path: String,
    /// Platform
    pub platform: String,
    /// Architecture
    pub arch: String,
    /// Electron (Nefu) version
    pub nefu_version: String,
}

/// App metrics (process memory usage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetrics {
    /// Process ID
    pub pid: u32,
    /// Memory usage (bytes)
    pub memory_usage_bytes: u64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Creation time
    pub creation_time: String,
    /// Whether sandboxed
    pub sandboxed: bool,
    /// Whether the GPU is integrated
    pub integrated_gpu: bool,
}

/// Login item settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginItemSettings {
    /// Whether to start at login
    pub open_at_login: bool,
    /// Whether to start hidden
    pub open_as_hidden: bool,
    /// Whether to restore the previous session at login
    pub restore_previous_session: bool,
    /// Launch path
    pub path: Option<String>,
    /// Launch arguments
    pub args: Vec<String>,
}

/// Get app info
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: get_app_name(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        is_packaged: is_packaged(),
        app_path: get_app_path().unwrap_or_default(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        nefu_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// Get the app name
fn get_app_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "NefuApp".to_string())
}

/// Whether the app is packaged
fn is_packaged() -> bool {
    // Check whether the current executable carries the Nefu packaging marker
    std::env::current_exe()
        .ok()
        .map(|p| crate::pack::is_nefu_package(&p))
        .unwrap_or(false)
}

/// Get a system path (similar to Electron's app.getPath)
///
/// Supported path names:
/// - home: the user's home directory
/// - appData: the app data directory
/// - userData: the user data directory
/// - sessionData: the session data directory
/// - temp: the temp directory
/// - exe: the current executable path
/// - module: the module path
/// - desktop: the desktop directory
/// - documents: the documents directory
/// - downloads: the downloads directory
/// - music: the music directory
/// - pictures: the pictures directory
/// - videos: the videos directory
/// - recent: the recent files directory
/// - logs: the logs directory
/// - crashDumps: the crash dumps directory
pub fn get_path(name: &str) -> Result<String> {
    let path = match name {
        "home" => dirs_home()?,
        "appData" => dirs_app_data()?,
        "userData" => dirs_user_data()?,
        "sessionData" => dirs_session_data()?,
        "temp" => std::env::temp_dir(),
        "exe" => std::env::current_exe()?,
        "module" => std::env::current_exe()?,
        "desktop" => dirs_desktop()?,
        "documents" => dirs_documents()?,
        "downloads" => dirs_downloads()?,
        "music" => dirs_music()?,
        "pictures" => dirs_pictures()?,
        "videos" => dirs_videos()?,
        "recent" => dirs_recent()?,
        "logs" => dirs_logs()?,
        "crashDumps" => dirs_crash_dumps()?,
        _ => return Err(anyhow::anyhow!("unknown path name: {}", name)),
    };
    Ok(path.to_string_lossy().to_string())
}

/// Get the app path
fn get_app_path() -> Result<String> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default())
}

/// Get the user's home directory
fn dirs_home() -> Result<PathBuf> {
    dirs_next::home_dir().ok_or_else(|| anyhow::anyhow!("failed to get the user's home directory"))
}

/// Get the app data directory
fn dirs_app_data() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    Ok(base.map_err(|_| anyhow::anyhow!("failed to get the app data directory"))?)
}

/// Get the user data directory
fn dirs_user_data() -> Result<PathBuf> {
    let base = dirs_app_data()?;
    Ok(base.join("nefu"))
}

/// Get the session data directory
fn dirs_session_data() -> Result<PathBuf> {
    let base = dirs_user_data()?;
    Ok(base.join("session"))
}

/// Get the desktop directory
fn dirs_desktop() -> Result<PathBuf> {
    dirs_next::desktop_dir().ok_or_else(|| anyhow::anyhow!("failed to get the desktop directory"))
}

/// Get the documents directory
fn dirs_documents() -> Result<PathBuf> {
    dirs_next::document_dir().ok_or_else(|| anyhow::anyhow!("failed to get the documents directory"))
}

/// Get the downloads directory
fn dirs_downloads() -> Result<PathBuf> {
    dirs_next::download_dir().ok_or_else(|| anyhow::anyhow!("failed to get the downloads directory"))
}

/// Get the music directory
fn dirs_music() -> Result<PathBuf> {
    dirs_next::audio_dir().ok_or_else(|| anyhow::anyhow!("failed to get the music directory"))
}

/// Get the pictures directory
fn dirs_pictures() -> Result<PathBuf> {
    dirs_next::picture_dir().ok_or_else(|| anyhow::anyhow!("failed to get the pictures directory"))
}

/// Get the videos directory
fn dirs_videos() -> Result<PathBuf> {
    dirs_next::video_dir().ok_or_else(|| anyhow::anyhow!("failed to get the videos directory"))
}

/// Get the recent files directory
fn dirs_recent() -> Result<PathBuf> {
    if cfg!(windows) {
        Ok(PathBuf::from(
            std::env::var("APPDATA")
                .map_err(|_| anyhow::anyhow!("failed to get APPDATA"))?
                + r"\Microsoft\Windows\Recent",
        ))
    } else {
        dirs_home()
    }
}

/// Get the logs directory
fn dirs_logs() -> Result<PathBuf> {
    let base = dirs_user_data()?;
    Ok(base.join("logs"))
}

/// Get the crash dumps directory
fn dirs_crash_dumps() -> Result<PathBuf> {
    let base = dirs_user_data()?;
    Ok(base.join("crashes"))
}

/// Get the locale
pub fn get_locale() -> String {
    // Try to get the system locale
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-Command", "Get-Culture | Select-Object -ExpandProperty Name"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();
        if let Some(out) = output {
            let locale = String::from_utf8(out.stdout).unwrap_or_default();
            let locale = locale.trim().to_string();
            if !locale.is_empty() {
                return locale;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            .ok();
        if let Some(out) = output {
            let locale = String::from_utf8(out.stdout).unwrap_or_default();
            let locale = locale.trim().to_string();
            if !locale.is_empty() {
                return locale;
            }
        }
    }
    // Fall back to environment variables
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en-US".to_string())
}

/// Get the locale country code
pub fn get_locale_country_code() -> String {
    let locale = get_locale();
    // Extract the country code from the locale format, e.g. "zh-CN" -> "CN"
    locale.split('-').nth(1).unwrap_or("US").to_string()
}

/// Get the login item settings
pub fn get_login_item_settings() -> LoginItemSettings {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let exe_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let output = std::process::Command::new("reg")
            .args([
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "NefuApp",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        let open_at_login = output
            .map(|o| String::from_utf8(o.stdout).unwrap_or_default())
            .map(|s| s.contains(&exe_path))
            .unwrap_or(false);

        LoginItemSettings {
            open_at_login,
            open_as_hidden: false,
            restore_previous_session: false,
            path: if open_at_login { Some(exe_path) } else { None },
            args: vec![],
        }
    }

    #[cfg(not(windows))]
    {
        LoginItemSettings {
            open_at_login: false,
            open_as_hidden: false,
            restore_previous_session: false,
            path: None,
            args: vec![],
        }
    }
}

/// Set the login item settings
pub fn set_login_item_settings(settings: &LoginItemSettings) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let exe_path = std::env::current_exe()?
            .to_string_lossy()
            .to_string();

        if settings.open_at_login {
            let args = if settings.open_as_hidden { " --hidden" } else { "" };
            std::process::Command::new("reg")
                .args([
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "NefuApp",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &format!("\"{}\"{}", exe_path, args),
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status()?;
        } else {
            std::process::Command::new("reg")
                .args([
                    "delete",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "NefuApp",
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status()?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = settings;
        Err(anyhow::anyhow!("setting login items is not supported on this platform"))
    }
}

/// Get process memory usage info
pub fn get_app_metrics() -> Vec<AppMetrics> {
    let pid = std::process::id();
    let mut metrics = Vec::new();

    // Use sysinfo to get info about the current process
    let mut sys = crate::system_info::get_system_instance();
    sys.refresh_process(sysinfo::Pid::from(pid as usize));
    if let Some(process) = sys.process(sysinfo::Pid::from(pid as usize)) {
        metrics.push(AppMetrics {
            pid,
            memory_usage_bytes: process.memory(),
            cpu_usage_percent: process.cpu_usage() as f64,
            creation_time: format!("{:?}", process.run_time()),
            sandboxed: false,
            integrated_gpu: false,
        });
    }

    if metrics.is_empty() {
        metrics.push(AppMetrics {
            pid,
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
            creation_time: String::new(),
            sandboxed: false,
            integrated_gpu: false,
        });
    }

    metrics
}

/// Get the system proxy settings
pub fn get_system_proxy_settings() -> serde_json::Value {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args(["-Command", "Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings' | Select-Object ProxyEnable, ProxyServer, ProxyOverride | ConvertTo-Json"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                return serde_json::json!({
                    "enabled": json["ProxyEnable"].as_u64().unwrap_or(0) == 1,
                    "server": json["ProxyServer"].as_str().unwrap_or(""),
                    "bypass": json["ProxyOverride"].as_str().unwrap_or(""),
                });
            }
        }
    }

    serde_json::json!({
        "enabled": false,
        "server": "",
        "bypass": "",
    })
}

/// Get the system version info
pub fn get_system_version_info() -> serde_json::Value {
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "locale": get_locale(),
        "country_code": get_locale_country_code(),
        "version_string": crate::shell::get_system_version(),
        "is_packaged": is_packaged(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_app_name() {
        let name = get_app_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_get_locale() {
        let locale = get_locale();
        assert!(!locale.is_empty());
    }

    #[test]
    fn test_get_path_home() {
        let path = get_path("home");
        assert!(path.is_ok());
        assert!(!path.unwrap().is_empty());
    }

    #[test]
    fn test_get_path_invalid() {
        let path = get_path("nonexistent");
        assert!(path.is_err());
    }

    #[test]
    fn test_get_app_metrics() {
        let metrics = get_app_metrics();
        assert!(!metrics.is_empty());
        assert_eq!(metrics[0].pid, std::process::id());
    }

    #[test]
    fn test_get_login_item_settings() {
        let settings = get_login_item_settings();
        // Only verify that it can be fetched, not the specific values
        assert!(!settings.open_at_login || settings.path.is_some());
    }

    #[test]
    fn test_get_system_version_info() {
        let info = get_system_version_info();
        assert!(info["os"].is_string());
        assert!(info["locale"].is_string());
    }
}
