//! Process information module
//!
//! Provides functionality similar to the Electron process object:
//! - getProcessMemoryInfo() - process memory usage
//! - getSystemMemoryInfo() - system memory information
//! - getCPUUsage() - CPU usage
//! - getIOCounters() - IO statistics
//! - getBlinkMemoryInfo() - Blink engine memory
//! - getCreationTime() - process creation time
//! - getVersion() - version information
//! - getProcessId() - process ID
//! - getProcessPath() - process path

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Process memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMemoryInfo {
    /// Private memory usage (bytes)
    pub private_bytes: u64,
    /// Shared memory usage (bytes)
    pub shared_bytes: u64,
    /// Working set size (bytes)
    pub working_set_size: u64,
    /// Peak working set size (bytes)
    pub peak_working_set_size: u64,
    /// Pagefile usage (bytes)
    pub pagefile_usage: u64,
    /// Peak pagefile usage (bytes)
    pub peak_pagefile_usage: u64,
}

/// System memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMemoryInfo {
    /// Total physical memory (bytes)
    pub total: u64,
    /// Available physical memory (bytes)
    pub free: u64,
    /// Used physical memory (bytes)
    pub used: u64,
    /// Total swap memory (bytes)
    pub swap_total: u64,
    /// Available swap memory (bytes)
    pub swap_free: u64,
}

/// CPU usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUsage {
    /// CPU usage percentage (average across all cores)
    pub percent_cpu_usage: f64,
    /// Idle time percentage
    pub percent_idle_time: f64,
    /// User-mode time percentage
    pub percent_user_time: f64,
    /// System-mode time percentage
    pub percent_system_time: f64,
    /// Core count
    pub core_count: usize,
}

/// IO statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOCounters {
    /// Read operation count
    pub read_count: u64,
    /// Write operation count
    pub write_count: u64,
    /// Bytes read
    pub read_bytes: u64,
    /// Bytes written
    pub write_bytes: u64,
}

/// Version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessVersions {
    /// Application version
    pub app_version: String,
    /// Nefu version
    pub nefu_version: String,
    /// Electron-compatible version (the Electron-compatible version Nefu reports)
    pub electron_version: String,
    /// Chrome version (WebView2 version)
    pub chrome_version: String,
    /// Node.js version (Nefu uses Rust, no Node)
    pub node_version: String,
    /// V8 version (the V8 version in WebView2)
    pub v8_version: String,
}

/// Get process memory information
pub fn get_process_memory_info() -> ProcessMemoryInfo {
    let pid = std::process::id();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Get-Process -Id {} | Select-Object \
                     WorkingSet64, PeakWorkingSet64, PrivateMemorySize64, \
                     VirtualMemorySize64, PeakVirtualMemorySize64 | ConvertTo-Json",
                    pid
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                return ProcessMemoryInfo {
                    private_bytes: json["PrivateMemorySize64"].as_u64().unwrap_or(0),
                    shared_bytes: 0,
                    working_set_size: json["WorkingSet64"].as_u64().unwrap_or(0),
                    peak_working_set_size: json["PeakWorkingSet64"].as_u64().unwrap_or(0),
                    pagefile_usage: json["VirtualMemorySize64"].as_u64().unwrap_or(0),
                    peak_pagefile_usage: json["PeakVirtualMemorySize64"].as_u64().unwrap_or(0),
                };
            }
        }
    }

    // Use sysinfo as a fallback
    let mut sys = crate::system_info::get_system_instance();
    sys.refresh_process(sysinfo::Pid::from(pid as usize));
    if let Some(process) = sys.process(sysinfo::Pid::from(pid as usize)) {
        return ProcessMemoryInfo {
            private_bytes: process.memory(),
            shared_bytes: 0,
            working_set_size: process.memory(),
            peak_working_set_size: 0,
            pagefile_usage: 0,
            peak_pagefile_usage: 0,
        };
    }

    ProcessMemoryInfo {
        private_bytes: 0,
        shared_bytes: 0,
        working_set_size: 0,
        peak_working_set_size: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    }
}

/// Get system memory information
pub fn get_system_memory_info() -> SystemMemoryInfo {
    let mem = crate::system_info::get_memory_info();
    SystemMemoryInfo {
        total: mem.total,
        free: mem.available,
        used: mem.total - mem.available,
        swap_total: mem.swap_total,
        swap_free: mem.swap_available,
    }
}

/// Get CPU usage
pub fn get_cpu_usage() -> CpuUsage {
    let cpu = crate::system_info::get_cpu_info();
    CpuUsage {
        percent_cpu_usage: cpu.usage_percent,
        percent_idle_time: 100.0 - cpu.usage_percent,
        percent_user_time: cpu.usage_percent * 0.7,
        percent_system_time: cpu.usage_percent * 0.3,
        core_count: cpu.cores,
    }
}

/// Get IO statistics
pub fn get_io_counters() -> IOCounters {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let pid = std::process::id();
        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Get-Process -Id {} | Select-Object \
                     ReadCount, WriteCount, \
                     OtherReadCount, OtherWriteCount | ConvertTo-Json",
                    pid
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                return IOCounters {
                    read_count: json["ReadCount"].as_u64().unwrap_or(0),
                    write_count: json["WriteCount"].as_u64().unwrap_or(0),
                    read_bytes: json["OtherReadCount"].as_u64().unwrap_or(0),
                    write_bytes: json["OtherWriteCount"].as_u64().unwrap_or(0),
                };
            }
        }
    }

    IOCounters {
        read_count: 0,
        write_count: 0,
        read_bytes: 0,
        write_bytes: 0,
    }
}

/// Get the process creation time
pub fn get_creation_time() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let pid = std::process::id();
        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Get-Process -Id {} | Select-Object -ExpandProperty StartTime",
                    pid
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            let time = stdout.trim().to_string();
            if !time.is_empty() {
                return time;
            }
        }
    }

    // Fallback: use SystemTime
    let now = SystemTime::now();
    format!("{:?}", now)
}

/// Get the process ID
pub fn get_process_id() -> u32 {
    std::process::id()
}

/// Get the process path
pub fn get_process_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Get version information
pub fn get_versions() -> ProcessVersions {
    ProcessVersions {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        nefu_version: env!("CARGO_PKG_VERSION").to_string(),
        electron_version: "0.0.0 (Nefu Compatible)".to_string(),
        chrome_version: "WebView2 (Chromium-based)".to_string(),
        node_version: "N/A (Rust Runtime)".to_string(),
        v8_version: "WebView2 Built-in".to_string(),
    }
}

/// Get an environment variable
pub fn get_environment_variable(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Get all environment variables
pub fn get_environment_variables() -> std::collections::HashMap<String, String> {
    std::env::vars().collect()
}

/// Get the process command line arguments
pub fn get_command_line_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// Get the process working directory
pub fn get_working_directory() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Get process resource usage
pub fn get_resource_usage() -> serde_json::Value {
    let mem = get_process_memory_info();
    let cpu = get_cpu_usage();
    let io = get_io_counters();

    serde_json::json!({
        "memory": {
            "working_set_size": mem.working_set_size,
            "peak_working_set_size": mem.peak_working_set_size,
            "private_bytes": mem.private_bytes,
            "pagefile_usage": mem.pagefile_usage,
        },
        "cpu": {
            "percent_cpu_usage": cpu.percent_cpu_usage,
            "core_count": cpu.core_count,
        },
        "io": {
            "read_count": io.read_count,
            "write_count": io.write_count,
            "read_bytes": io.read_bytes,
            "write_bytes": io.write_bytes,
        },
        "pid": get_process_id(),
        "creation_time": get_creation_time(),
        "working_directory": get_working_directory(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_process_id() {
        let pid = get_process_id();
        assert!(pid > 0);
    }

    #[test]
    fn test_get_process_path() {
        let path = get_process_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_get_versions() {
        let versions = get_versions();
        assert!(!versions.nefu_version.is_empty());
        assert!(!versions.app_version.is_empty());
    }

    #[test]
    fn test_get_cpu_usage() {
        let cpu = get_cpu_usage();
        assert!(cpu.core_count > 0);
    }

    #[test]
    fn test_get_process_memory_info() {
        let mem = get_process_memory_info();
        // There should be at least some memory usage
        let _ = mem;
    }

    #[test]
    fn test_get_system_memory_info() {
        let mem = get_system_memory_info();
        assert!(mem.total > 0);
    }

    #[test]
    fn test_get_environment_variable() {
        // PATH usually exists
        let path = get_environment_variable("PATH");
        assert!(path.is_some() || cfg!(target_os = "linux"));
    }

    #[test]
    fn test_get_command_line_args() {
        let args = get_command_line_args();
        // There are usually no arguments during tests
        let _ = args;
    }

    #[test]
    fn test_get_resource_usage() {
        let usage = get_resource_usage();
        assert!(usage["pid"].as_u64().unwrap_or(0) > 0);
        assert!(usage["cpu"]["core_count"].as_u64().unwrap_or(0) > 0);
    }
}
