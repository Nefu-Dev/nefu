//! NativeTheme module
//!
//! Provides functionality similar to the Electron nativeTheme:
//! - shouldUseDarkColors - whether to use the dark theme
//! - shouldUseHighContrastColors - whether to use the high contrast theme
//! - shouldUseInvertedColorScheme - whether to use an inverted color scheme
//! - themeSource - theme source (system, light, dark)
//! - onUpdated - theme update event

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// Theme source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSource {
    /// Follow the system
    System,
    /// Light theme
    Light,
    /// Dark theme
    Dark,
}

impl ThemeSource {
    fn as_u8(&self) -> u8 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }
}

/// Theme change listener
pub type ThemeListener = Box<dyn Fn() + Send + 'static>;

/// NativeTheme manager
///
/// Manages system theme preferences and changes
pub struct NativeTheme {
    /// Theme source
    theme_source: AtomicU8,
    /// Theme change listeners
    listeners: Mutex<Vec<ThemeListener>>,
}

impl NativeTheme {
    /// Create a new NativeTheme manager
    pub fn new() -> Self {
        Self {
            theme_source: AtomicU8::new(ThemeSource::System.as_u8()),
            listeners: Mutex::new(Vec::new()),
        }
    }

    /// Whether dark colors should be used
    pub fn should_use_dark_colors(&self) -> bool {
        match self.theme_source() {
            ThemeSource::Dark => true,
            ThemeSource::Light => false,
            ThemeSource::System => self.detect_system_dark_mode(),
        }
    }

    /// Whether high contrast colors should be used
    pub fn should_use_high_contrast_colors(&self) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let output = std::process::Command::new("powershell")
                .args([
                    "-Command",
                    "[System.Windows.Forms.SystemInformation]::HighContrast"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok();
            if let Some(out) = output {
                let stdout = String::from_utf8(out.stdout).unwrap_or_default();
                if stdout.trim() == "True" {
                    return true;
                }
            }
        }
        false
    }

    /// Whether an inverted color scheme should be used
    pub fn should_use_inverted_color_scheme(&self) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let output = std::process::Command::new("powershell")
                .args([
                    "-Command",
                    "(Get-ItemProperty 'HKCU:\\Software\\Microsoft\\ColorFiltering').Active"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok();
            if let Some(out) = output {
                let stdout = String::from_utf8(out.stdout).unwrap_or_default();
                if stdout.trim() == "1" {
                    return true;
                }
            }
        }
        false
    }

    /// Get the theme source
    pub fn theme_source(&self) -> ThemeSource {
        ThemeSource::from_u8(self.theme_source.load(Ordering::Relaxed))
    }

    /// Set the theme source
    ///
    /// # Parameters
    /// - `source`: Theme source
    pub fn set_theme_source(&self, source: ThemeSource) -> Result<()> {
        let old = self.theme_source.swap(source.as_u8(), Ordering::Relaxed);
        if old != source.as_u8() {
            self.notify_listeners();
        }
        log::info!("Theme source set to: {:?}", source);
        Ok(())
    }

    /// Add a theme change listener
    pub fn add_listener<F>(&self, listener: F)
    where
        F: Fn() + Send + 'static,
    {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(Box::new(listener));
        }
    }

    /// Notify all listeners
    fn notify_listeners(&self) {
        if let Ok(listeners) = self.listeners.lock() {
            for listener in listeners.iter() {
                listener();
            }
        }
    }

    /// Detect system dark mode
    fn detect_system_dark_mode(&self) -> bool {
        crate::system_info::get_system_theme() == crate::system_info::SystemTheme::Dark
    }
}

/// Global NativeTheme instance
static NATIVE_THEME_INSTANCE: std::sync::OnceLock<NativeTheme> = std::sync::OnceLock::new();

/// Get the global NativeTheme
pub fn get_native_theme() -> &'static NativeTheme {
    NATIVE_THEME_INSTANCE.get_or_init(NativeTheme::new)
}
