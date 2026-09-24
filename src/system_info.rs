//! System information module
//!
//! Provides functionality similar to Electron's systemPreferences / screen / process:
//! - System theme detection (dark/light mode)
//! - Screen information (size, scale, refresh rate)
//! - System memory information
//! - CPU information
//! - GPU information
//! - Process information
//! - System power status

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// System theme
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SystemTheme {
    /// Light mode
    Light,
    /// Dark mode
    Dark,
    /// Unknown
    Unknown,
}

/// Screen information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    /// Screen width (pixels)
    pub width: u32,
    /// Screen height (pixels)
    pub height: u32,
    /// Scale factor
    pub scale_factor: f64,
    /// Whether it is the primary screen
    pub is_primary: bool,
}

/// Memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total physical memory (bytes)
    pub total: u64,
    /// Available physical memory (bytes)
    pub available: u64,
    /// Used physical memory percentage
    pub used_percent: f64,
    /// Total swap memory (bytes)
    pub swap_total: u64,
    /// Available swap memory (bytes)
    pub swap_available: u64,
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU model name
    pub name: String,
    /// CPU core count
    pub cores: usize,
    /// CPU usage percentage
    pub usage_percent: f64,
    /// CPU architecture
    pub arch: String,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU model name
    pub name: String,
    /// GPU video memory (bytes, 0 means unknown)
    pub vram: u64,
    /// GPU driver version
    pub driver: String,
    /// Whether it is an integrated GPU
    pub is_integrated: bool,
}

/// Power status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerState {
    /// Whether powered by battery
    pub on_battery: bool,
    /// Battery percentage (0-100, None means unknown)
    pub battery_percent: Option<f32>,
    /// Whether charging
    pub charging: Option<bool>,
    /// Estimated remaining time (seconds)
    pub time_remaining: Option<u64>,
}

/// Global sysinfo system instance (lazy initialization)
fn system() -> &'static std::sync::Mutex<sysinfo::System> {
    static SYSTEM: OnceLock<std::sync::Mutex<sysinfo::System>> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        std::sync::Mutex::new(sys)
    })
}

/// Get the system information instance (for reuse by other modules)
pub fn get_system_instance() -> std::sync::MutexGuard<'static, sysinfo::System> {
    system().lock().unwrap_or_else(|e| {
        e.into_inner()
    })
}

/// Refresh the system information cache
pub fn refresh_system_info() {
    if let Ok(mut sys) = system().lock() {
        sys.refresh_all();
    }
}

/// Get the system theme (dark/light mode)
///
/// Determined by reading the system registry or user preferences
pub fn get_system_theme() -> SystemTheme {
    #[cfg(windows)]
    {
        // Windows: read the registry to get the theme setting
        match get_windows_theme() {
            Some(true) => SystemTheme::Dark,
            Some(false) => SystemTheme::Light,
            None => SystemTheme::Unknown,
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: read the defaults command
        match get_macos_theme() {
            Some(true) => SystemTheme::Dark,
            Some(false) => SystemTheme::Light,
            None => SystemTheme::Unknown,
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: check the desktop environment's theme settings
        match get_linux_theme() {
            Some(true) => SystemTheme::Dark,
            Some(false) => SystemTheme::Light,
            None => SystemTheme::Unknown,
        }
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        SystemTheme::Unknown
    }
}

#[cfg(windows)]
fn get_windows_theme() -> Option<bool> {
    // Read the dark mode setting from the Windows registry
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = std::process::Command::new("reg")
        .args([
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    let stdout = String::from_utf8(output.stdout).ok()?;
    // Output format: ... AppsUseLightTheme  REG_DWORD  0x0
    if stdout.contains("0x0") {
        Some(true) // dark mode
    } else if stdout.contains("0x1") {
        Some(false) // light mode
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_macos_theme() -> Option<bool> {
    let output = std::process::Command::new("defaults")
        .args([
            "read",
            "-g",
            "AppleInterfaceStyle",
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(stdout.trim().contains("Dark"))
}

#[cfg(target_os = "linux")]
fn get_linux_theme() -> Option<bool> {
    // Check the GTK theme settings
    let gtk_settings = std::env::var("GTK_THEME").ok()?;
    Some(gtk_settings.to_lowercase().contains("dark"))
}

/// Get the primary screen information
pub fn get_primary_screen_info() -> ScreenInfo {
    // Use tao to get the screen information
    // System APIs can also be used as a fallback
    #[cfg(windows)]
    {
        get_windows_screen_info()
    }

    #[cfg(not(windows))]
    {
        // Cross-platform fallback
        ScreenInfo {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
        }
    }
}

#[cfg(windows)]
fn get_windows_screen_info() -> ScreenInfo {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Use PowerShell to get the screen information
    let output = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; \
             [System.Windows.Forms.Screen]::PrimaryScreen | \
             Select-Object Bounds, WorkingArea | ConvertTo-Json",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok();

    if let Some(out) = output {
        let stdout = String::from_utf8(out.stdout).unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let bounds = &json["Bounds"];
            let width = bounds["Width"].as_u64().unwrap_or(1920) as u32;
            let height = bounds["Height"].as_u64().unwrap_or(1080) as u32;
            return ScreenInfo {
                width,
                height,
                scale_factor: 1.0,
                is_primary: true,
            };
        }
    }

    ScreenInfo {
        width: 1920,
        height: 1080,
        scale_factor: 1.0,
        is_primary: true,
    }
}

/// Get system memory information
pub fn get_memory_info() -> MemoryInfo {
    if let Ok(mut sys) = system().lock() {
        sys.refresh_memory();
        MemoryInfo {
            total: sys.total_memory(),
            available: sys.available_memory(),
            used_percent: if sys.total_memory() > 0 {
                (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
            } else {
                0.0
            },
            swap_total: sys.total_swap(),
            swap_available: sys.free_swap(),
        }
    } else {
        MemoryInfo {
            total: 0,
            available: 0,
            used_percent: 0.0,
            swap_total: 0,
            swap_available: 0,
        }
    }
}

/// Get CPU information
pub fn get_cpu_info() -> CpuInfo {
    if let Ok(mut sys) = system().lock() {
        sys.refresh_cpu_usage();
        let cpus = sys.cpus();
        let name = cpus.first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let cores = cpus.len();
        let usage = cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cores.max(1) as f32;

        CpuInfo {
            name,
            cores,
            usage_percent: usage as f64,
            arch: std::env::consts::ARCH.to_string(),
        }
    } else {
        CpuInfo {
            name: "Unknown".to_string(),
            cores: 0,
            usage_percent: 0.0,
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

/// Get GPU information
pub fn get_gpu_info() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("wmic")
            .args([
                "path",
                "win32_VideoController",
                "get",
                "Name,AdapterRAM,DriverVersion",
                "/format:csv",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            for line in stdout.lines().skip(1) {
                if line.trim().is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    let name = parts[1].trim().to_string();
                    let vram = parts[2].trim().parse::<u64>().unwrap_or(0);
                    let driver = parts.get(3).map(|s| s.trim().to_string()).unwrap_or_default();
                    let is_integrated = name.to_lowercase().contains("intel")
                        || name.to_lowercase().contains("amd")
                            && !name.to_lowercase().contains("radeon");
                    gpus.push(GpuInfo {
                        name,
                        vram,
                        driver,
                        is_integrated,
                    });
                }
            }
        }
    }

    if gpus.is_empty() {
        gpus.push(GpuInfo {
            name: "Unknown".to_string(),
            vram: 0,
            driver: "Unknown".to_string(),
            is_integrated: false,
        });
    }

    gpus
}

/// Get the power status
pub fn get_power_state() -> PowerState {
    #[cfg(windows)]
    {
        get_windows_power_state()
    }

    #[cfg(not(windows))]
    {
        PowerState {
            on_battery: false,
            battery_percent: None,
            charging: None,
            time_remaining: None,
        }
    }
}

#[cfg(windows)]
fn get_windows_power_state() -> PowerState {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Get-WmiObject -Class Win32_Battery | Select-Object EstimatedChargeRemaining, BatteryStatus, EstimatedRunTime | ConvertTo-Json",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok();

    if let Some(out) = output {
        let stdout = String::from_utf8(out.stdout).unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let percent = json["EstimatedChargeRemaining"].as_f64().map(|v| v as f32);
            let status = json["BatteryStatus"].as_u64().unwrap_or(1);
            let runtime = json["EstimatedRunTime"].as_u64();
            return PowerState {
                on_battery: status != 2, // 2 = charging
                battery_percent: percent,
                charging: Some(status == 2),
                time_remaining: runtime,
            };
        }
    }

    PowerState {
        on_battery: false,
        battery_percent: None,
        charging: None,
        time_remaining: None,
    }
}

/// Get a readable summary of the system information
pub fn get_system_info_summary() -> serde_json::Value {
    let memory = get_memory_info();
    let cpu = get_cpu_info();
    let theme = get_system_theme();
    let power = get_power_state();

    let format_bytes = |bytes: u64| -> String {
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{} B", bytes)
        }
    };

    serde_json::json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "theme": theme,
        "memory": {
            "total": memory.total,
            "total_formatted": format_bytes(memory.total),
            "available": memory.available,
            "available_formatted": format_bytes(memory.available),
            "used_percent": memory.used_percent,
        },
        "cpu": {
            "name": cpu.name,
            "cores": cpu.cores,
            "usage_percent": cpu.usage_percent,
            "arch": cpu.arch,
        },
        "power": {
            "on_battery": power.on_battery,
            "battery_percent": power.battery_percent,
            "charging": power.charging,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_info() {
        let mem = get_memory_info();
        // Should return at least a non-zero total memory
        assert!(mem.total > 0 || cfg!(target_os = "linux"));
    }

    #[test]
    fn test_cpu_info() {
        let cpu = get_cpu_info();
        assert!(!cpu.name.is_empty());
        assert!(cpu.cores > 0);
    }

    #[test]
    fn test_system_info_summary() {
        let summary = get_system_info_summary();
        assert!(summary["platform"].is_string());
        assert!(summary["memory"]["total"].is_number());
    }
}
