//! Crash reporter module
//!
//! Provides functionality similar to the Electron crashReporter:
//! - Global panic capture
//! - Crash log written to file
//! - Crash information sent to a remote server
//! - In-process error collection

use anyhow::Result;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

/// Crash reporter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReporterConfig {
    /// Company/application name
    pub company_name: String,
    /// Whether to submit to a remote server
    pub submit_url: Option<String>,
    /// Whether to include system information
    pub include_system_info: bool,
    /// Crash log storage directory
    pub crash_dir: PathBuf,
    /// Whether to upload logs
    pub upload_to_server: bool,
    /// Extra metadata
    pub extra: std::collections::HashMap<String, String>,
}

impl Default for CrashReporterConfig {
    fn default() -> Self {
        Self {
            company_name: "Nefu".to_string(),
            submit_url: None,
            include_system_info: true,
            crash_dir: get_default_crash_dir(),
            upload_to_server: false,
            extra: std::collections::HashMap::new(),
        }
    }
}

/// Crash report data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Application version
    pub version: String,
    /// Platform
    pub platform: String,
    /// Crash time
    pub timestamp: String,
    /// Error message
    pub error_message: String,
    /// Error type
    pub error_type: String,
    /// Stack trace
    pub stack_trace: String,
    /// System information
    pub system_info: Option<String>,
    /// Extra metadata
    pub extra: std::collections::HashMap<String, String>,
}

/// Crash reporter
pub struct CrashReporter {
    /// Configuration
    config: CrashReporterConfig,
    /// Whether initialized
    initialized: AtomicBool,
    /// Previous panic hook
    previous_hook: Mutex<Option<Box<dyn Fn(&std::panic::PanicInfo<'_>) + Send + Sync + 'static>>>,
    /// Crash count
    crash_count: Mutex<u32>,
}

impl CrashReporter {
    /// Create a new crash reporter
    ///
    /// # Parameters
    /// - `config`: Crash reporter configuration
    pub fn new(config: CrashReporterConfig) -> Self {
        Self {
            config,
            initialized: AtomicBool::new(false),
            previous_hook: Mutex::new(None),
            crash_count: Mutex::new(0),
        }
    }

    /// Initialize the crash reporter
    ///
    /// Sets a global panic hook to capture all unhandled panics
    pub fn start(&self) {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return;
        }

        // Ensure the crash log directory exists
        if let Err(e) = std::fs::create_dir_all(&self.config.crash_dir) {
            error!("Failed to create crash log directory: {}", e);
        }

        // Save the previous hook
        let prev = std::panic::take_hook();
        let mut hook_guard = self.previous_hook.lock().unwrap();
        *hook_guard = Some(prev);
        drop(hook_guard);

        // Set a new panic hook
        let config = self.config.clone();
        let crash_count = Mutex::new(0u32);

        std::panic::set_hook(Box::new(move |panic_info| {
            let report = capture_panic(panic_info, &config);

            // Save to file
            if let Err(e) = save_crash_report(&report, &config.crash_dir) {
                error!("Failed to save crash report: {}", e);
            }

            // Upload to the server
            if config.upload_to_server {
                if let Some(ref url) = config.submit_url {
                    if let Err(e) = upload_crash_report(&report, url) {
                        error!("Failed to upload crash report: {}", e);
                    }
                }
            }

            // Increment the crash count
            if let Ok(mut count) = crash_count.lock() {
                *count += 1;
                error!(
                    "Application crashed #{}, error: {}, details saved to: {:?}",
                    count,
                    report.error_message,
                    config.crash_dir
                );
            } else {
                error!(
                    "Application crashed, error: {}, details saved to: {:?}",
                    report.error_message, config.crash_dir
                );
            }
        }));
    }

    /// Get the crash count
    pub fn crash_count(&self) -> u32 {
        *self.crash_count.lock().unwrap_or_else(|e| {
            error!("Crash count lock is corrupted");
            e.into_inner()
        })
    }

    /// Generate a test crash report (for testing)
    pub fn generate_test_report(&self) -> CrashReport {
        let timestamp = format_timestamp(SystemTime::now());

        CrashReport {
            version: "1.0.0".to_string(),
            platform: std::env::consts::OS.to_string(),
            timestamp,
            error_message: "Test crash report".to_string(),
            error_type: "Test".to_string(),
            stack_trace: "test\nbacktrace\nline 3".to_string(),
            system_info: if self.config.include_system_info {
                Some(get_system_info_string())
            } else {
                None
            },
            extra: self.config.extra.clone(),
        }
    }
}

/// Capture a crash report from panic information
fn capture_panic(panic_info: &std::panic::PanicInfo<'_>, config: &CrashReporterConfig) -> CrashReport {
    let timestamp = format_timestamp(SystemTime::now());

    // Extract the error message
    let (error_message, error_type) = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        (s.to_string(), "Panic".to_string())
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        (s.clone(), "Panic".to_string())
    } else {
        ("Unknown panic".to_string(), "Panic".to_string())
    };

    // Extract the location information
    let location = panic_info
        .location()
        .map(|loc| format!("{}:{}", loc.file(), loc.line()))
        .unwrap_or_else(|| "Unknown location".to_string());

    // Build the stack trace
    let stack_trace = format!("Panic at: {}\n\nError: {}", location, error_message);

    // System information
    let system_info = if config.include_system_info {
        Some(get_system_info_string())
    } else {
        None
    };

    CrashReport {
        version: "1.0.0".to_string(),
        platform: std::env::consts::OS.to_string(),
        timestamp,
        error_message,
        error_type,
        stack_trace,
        system_info,
        extra: config.extra.clone(),
    }
}

/// Save a crash report to file
fn save_crash_report(report: &CrashReport, crash_dir: &Path) -> Result<PathBuf> {
    let filename = format!(
        "crash_{}_{}.json",
        report.timestamp.replace(':', "-").replace(' ', "_"),
        std::process::id()
    );
    let file_path = crash_dir.join(&filename);

    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&file_path, json)?;

    info!("Crash report saved: {}", file_path.display());
    Ok(file_path)
}

/// Upload a crash report to a remote server
fn upload_crash_report(report: &CrashReport, url: &str) -> Result<()> {
    let json = serde_json::to_string(report)?;

    let response = ureq::post(url)
        .timeout(std::time::Duration::from_secs(30))
        .set("Content-Type", "application/json")
        .send_string(&json);

    match response {
        Ok(_) => {
            info!("Crash report uploaded to: {}", url);
            Ok(())
        }
        Err(e) => {
            Err(anyhow::anyhow!("Upload failed: {}", e))
        }
    }
}

/// Get the system information string
fn get_system_info_string() -> String {
    format!(
        "OS: {}\nArch: {}\nPID: {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id()
    )
}

/// Get the default crash log directory
fn get_default_crash_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Logs"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local/share"))
            .unwrap_or_else(|_| PathBuf::from("."))
    };

    base.join("nefu").join("crashes")
}

/// Format a timestamp
fn format_timestamp(time: SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Simple formatting
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        1970 + days / 365,
        (days % 365) / 30 + 1,
        (days % 365) % 30 + 1,
        hours,
        minutes,
        seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_report_serde() {
        let report = CrashReport {
            version: "1.0.0".to_string(),
            platform: "windows".to_string(),
            timestamp: "2026-01-01 00:00:00".to_string(),
            error_message: "test error".to_string(),
            error_type: "Panic".to_string(),
            stack_trace: "stack".to_string(),
            system_info: None,
            extra: std::collections::HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let deserialized: CrashReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.error_message, "test error");
    }

    #[test]
    fn test_crash_reporter_config_default() {
        let config = CrashReporterConfig::default();
        assert_eq!(config.company_name, "Nefu");
        assert!(!config.crash_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_format_timestamp() {
        let time = SystemTime::UNIX_EPOCH;
        let formatted = format_timestamp(time);
        assert!(formatted.contains("1970"));
    }

    #[test]
    fn test_generate_test_report() {
        let config = CrashReporterConfig::default();
        let reporter = CrashReporter::new(config);
        let report = reporter.generate_test_report();
        assert_eq!(report.error_message, "Test crash report");
    }
}
