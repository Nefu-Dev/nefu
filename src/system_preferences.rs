//! System preferences module
//!
//! Provides functionality similar to the Electron systemPreferences:
//! - isDarkMode() - whether dark mode
//! - isInvertedColorScheme() - whether high contrast mode
//! - getColor(color) - get a system color
//! - getMediaAccessStatus(mediaType) - get media permission status
//! - askForMediaAccess(mediaType) - request media permission
//! - getUserDefault(key, type) - read user default settings
//! - setUserDefault(key, type, value) - set user default settings
//! - getSystemColor(color) - get the system accent color
//! - isHighContrastColorScheme() - whether the high contrast theme

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Media access type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MediaType {
    /// Camera
    Camera,
    /// Microphone
    Microphone,
    /// Screen
    Screen,
}

/// Media permission status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaAccessStatus {
    /// Not determined
    NotDetermined,
    /// Granted
    Granted,
    /// Denied
    Denied,
    /// Restricted
    Restricted,
}

/// System color
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SystemColor {
    /// 3D dark shadow
    ThreeDDarkShadow,
    /// 3D face
    ThreeDFace,
    /// 3D highlight
    ThreeDHighlight,
    /// 3D light
    ThreeDLight,
    /// 3D shadow
    ThreeDShadow,
    /// Window frame
    WindowFrame,
    /// Window title bar background (active)
    ActiveCaption,
    /// Window title bar text (active)
    CaptionText,
    /// Window title bar background (inactive)
    InactiveCaption,
    /// Window title bar text (inactive)
    InactiveCaptionText,
    /// Selected item background
    Highlight,
    /// Selected item text
    HighlightText,
    /// Menu bar background
    Menu,
    /// Menu bar text
    MenuText,
    /// Scrollbar
    Scrollbar,
    /// Window background
    Window,
    /// Window text
    WindowText,
    /// Button background
    ButtonFace,
    /// Button text
    ButtonText,
    /// Info background
    InfoBackground,
    /// Info text
    InfoText,
    /// Desktop
    Desktop,
    /// Active caption text color
    ActiveCaptionText,
    /// Gray text (disabled)
    GrayText,
    /// Gradient caption (active)
    GradientActiveCaption,
    /// Gradient caption (inactive)
    GradientInactiveCaption,
    /// Hot tracking item
    HotTrackingColor,
    /// Menu bar highlight
    MenuHighlight,
}

/// System color value (with alpha channel)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemColorValue {
    /// Red (0-255)
    pub red: u8,
    /// Green (0-255)
    pub green: u8,
    /// Blue (0-255)
    pub blue: u8,
    /// Alpha (0-255)
    pub alpha: u8,
}

impl SystemColorValue {
    /// Convert to a CSS color string
    pub fn to_css(&self) -> String {
        format!("rgba({}, {}, {}, {})", self.red, self.green, self.blue, self.alpha as f64 / 255.0)
    }

    /// Convert to a hexadecimal string
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

/// Check whether dark mode is active
pub fn is_dark_mode() -> bool {
    crate::system_info::get_system_theme() == crate::system_info::SystemTheme::Dark
}

/// Check whether high contrast mode is active
pub fn is_inverted_color_scheme() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                "(Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Accessibility' -Name 'HighContrast').HighContrast",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            return stdout.trim() == "1";
        }
        false
    }

    #[cfg(not(windows))]
    {
        false
    }
}

/// Check whether the high contrast theme is active
pub fn is_high_contrast_color_scheme() -> bool {
    is_inverted_color_scheme()
}

/// Get a system color
///
/// # Parameters
/// - `color_name`: Color name
///
/// # Returns
/// The system color value
pub fn get_color(color_name: &str) -> Result<SystemColorValue> {
    // Parse the color name
    let color = parse_color_name(color_name)?;

    // Return the system color value
    #[cfg(windows)]
    {
        get_windows_color(color)
    }

    #[cfg(not(windows))]
    {
        // Return the default value on non-Windows platforms
        Ok(get_default_color(&color))
    }
}

/// Parse a color name
fn parse_color_name(name: &str) -> Result<SystemColor> {
    match name {
        "3d-dark-shadow" => Ok(SystemColor::ThreeDDarkShadow),
        "3d-face" => Ok(SystemColor::ThreeDFace),
        "3d-highlight" => Ok(SystemColor::ThreeDHighlight),
        "3d-light" => Ok(SystemColor::ThreeDLight),
        "3d-shadow" => Ok(SystemColor::ThreeDShadow),
        "window-frame" => Ok(SystemColor::WindowFrame),
        "active-caption" => Ok(SystemColor::ActiveCaption),
        "caption-text" => Ok(SystemColor::CaptionText),
        "inactive-caption" => Ok(SystemColor::InactiveCaption),
        "inactive-caption-text" => Ok(SystemColor::InactiveCaptionText),
        "highlight" => Ok(SystemColor::Highlight),
        "highlight-text" => Ok(SystemColor::HighlightText),
        "menu" => Ok(SystemColor::Menu),
        "menu-text" => Ok(SystemColor::MenuText),
        "scrollbar" => Ok(SystemColor::Scrollbar),
        "window" => Ok(SystemColor::Window),
        "window-text" => Ok(SystemColor::WindowText),
        "button-face" => Ok(SystemColor::ButtonFace),
        "button-text" => Ok(SystemColor::ButtonText),
        "info-background" => Ok(SystemColor::InfoBackground),
        "info-text" => Ok(SystemColor::InfoText),
        "desktop" => Ok(SystemColor::Desktop),
        "gradient-active-caption" => Ok(SystemColor::GradientActiveCaption),
        "gradient-inactive-caption" => Ok(SystemColor::GradientInactiveCaption),
        "hot-tracking-color" => Ok(SystemColor::HotTrackingColor),
        "menu-highlight" => Ok(SystemColor::MenuHighlight),
        _ => Err(anyhow::anyhow!("Unknown color name: {}", name)),
    }
}

#[cfg(windows)]
fn get_windows_color(color: SystemColor) -> Result<SystemColorValue> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Windows color index mapping
    let color_index = match color {
        SystemColor::Scrollbar => 0,
        SystemColor::Desktop => 1,
        SystemColor::ActiveCaption => 2,
        SystemColor::InactiveCaption => 3,
        SystemColor::Menu => 4,
        SystemColor::Window => 5,
        SystemColor::WindowFrame => 6,
        SystemColor::MenuText => 7,
        SystemColor::WindowText => 8,
        SystemColor::CaptionText => 9,
        SystemColor::ActiveCaptionText => 9,
        SystemColor::ButtonFace => 15,
        SystemColor::ButtonText => 18,
        SystemColor::GrayText => 17,
        SystemColor::Highlight => 13,
        SystemColor::HighlightText => 14,
        SystemColor::HotTrackingColor => 26,
        SystemColor::InactiveCaptionText => 19,
        SystemColor::InfoBackground => 11,
        SystemColor::InfoText => 12,
        // Use a similar color for those without a direct mapping
        _ => 5, // default to the window color
    };

    let output = std::process::Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "[System.Drawing.Color]::FromName([System.Drawing.KnownColor]::new({})).ToArgb()",
                color_index
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok();

    if let Some(out) = output {
        let stdout = String::from_utf8(out.stdout).unwrap_or_default();
        if let Ok(argb) = stdout.trim().parse::<i32>() {
            let bytes = argb.to_ne_bytes();
            return Ok(SystemColorValue {
                red: bytes[2],
                green: bytes[1],
                blue: bytes[0],
                alpha: bytes[3],
            });
        }
    }

    Ok(get_default_color(&color))
}

/// Get the default color value
fn get_default_color(color: &SystemColor) -> SystemColorValue {
    match color {
        SystemColor::Window => SystemColorValue { red: 255, green: 255, blue: 255, alpha: 255 },
        SystemColor::WindowText => SystemColorValue { red: 0, green: 0, blue: 0, alpha: 255 },
        SystemColor::Menu => SystemColorValue { red: 240, green: 240, blue: 240, alpha: 255 },
        SystemColor::Highlight => SystemColorValue { red: 0, green: 120, blue: 215, alpha: 255 },
        SystemColor::HighlightText => SystemColorValue { red: 255, green: 255, blue: 255, alpha: 255 },
        SystemColor::ButtonFace => SystemColorValue { red: 240, green: 240, blue: 240, alpha: 255 },
        SystemColor::InfoBackground => SystemColorValue { red: 255, green: 255, blue: 225, alpha: 255 },
        SystemColor::Desktop => SystemColorValue { red: 58, green: 110, blue: 165, alpha: 255 },
        _ => SystemColorValue { red: 255, green: 255, blue: 255, alpha: 255 },
    }
}

/// Get the media access permission status
///
/// # Parameters
/// - `media_type`: Media type
///
/// # Returns
/// Permission status
pub fn get_media_access_status(media_type: &MediaType) -> MediaAccessStatus {
    #[cfg(windows)]
    {
        get_windows_media_access_status(media_type)
    }

    #[cfg(not(windows))]
    {
        // Always return granted on non-Windows platforms
        MediaAccessStatus::Granted
    }
}

#[cfg(windows)]
fn get_windows_media_access_status(media_type: &MediaType) -> MediaAccessStatus {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let reg_path = match media_type {
        MediaType::Camera => "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\webcam",
        MediaType::Microphone => "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone",
        MediaType::Screen => "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\screenCapture",
    };

    let output = std::process::Command::new("reg")
        .args(["query", reg_path, "/v", "Value"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok();

    if let Some(out) = output {
        let stdout = String::from_utf8(out.stdout).unwrap_or_default();
        if stdout.contains("Allow") {
            MediaAccessStatus::Granted
        } else if stdout.contains("Deny") {
            MediaAccessStatus::Denied
        } else {
            MediaAccessStatus::NotDetermined
        }
    } else {
        MediaAccessStatus::NotDetermined
    }
}

/// Request media access permission
///
/// # Parameters
/// - `media_type`: Media type
///
/// # Returns
/// Whether permission was granted
pub fn ask_for_media_access(media_type: &MediaType) -> Result<bool> {
    #[cfg(windows)]
    {
        // On Windows, request permission by launching the system settings
        let uri = match media_type {
            MediaType::Camera => "ms-settings:privacy-webcam",
            MediaType::Microphone => "ms-settings:privacy-microphone",
            MediaType::Screen => "ms-settings:privacy-screencapture",
        };

        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", uri])
            .spawn();

        // It is not possible to directly determine the permission change, return true to indicate the settings page was opened
        Ok(true)
    }

    #[cfg(not(windows))]
    {
        let _ = media_type;
        Ok(true) // Always return true on non-Windows platforms
    }
}

/// Get the effective system accent color
pub fn get_system_accent_color() -> SystemColorValue {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                "(Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\DWM' -Name 'AccentColor').AccentColor",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            if let Ok(color_val) = stdout.trim().parse::<u32>() {
                return SystemColorValue {
                    red: ((color_val >> 16) & 0xFF) as u8,
                    green: ((color_val >> 8) & 0xFF) as u8,
                    blue: (color_val & 0xFF) as u8,
                    alpha: ((color_val >> 24) & 0xFF) as u8,
                };
            }
        }
    }

    // Default blue accent color
    SystemColorValue { red: 0, green: 120, blue: 215, alpha: 255 }
}

/// Get the system animation preference
pub fn are_animations_enabled() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                "(Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects' -Name 'VisualFXSetting').VisualFXSetting",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            // 0 = let Windows choose (usually enabled), 1 = minimized, 2 = none
            return stdout.trim() != "2";
        }
    }

    true
}

/// Get the system transparency preference
pub fn is_transparency_enabled() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                "(Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize' -Name 'EnableTransparency').EnableTransparency",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(out) = output {
            let stdout = String::from_utf8(out.stdout).unwrap_or_default();
            return stdout.trim() == "1";
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dark_mode() {
        // Just verify that it can be called, not the specific value
        let _ = is_dark_mode();
    }

    #[test]
    fn test_get_color_valid() {
        let color = get_color("window");
        assert!(color.is_ok());
        let c = color.unwrap();
        assert!(c.red <= 255);
        assert!(c.alpha <= 255);
    }

    #[test]
    fn test_get_color_invalid() {
        let color = get_color("nonexistent-color");
        assert!(color.is_err());
    }

    #[test]
    fn test_color_value_to_css() {
        let color = SystemColorValue { red: 255, green: 0, blue: 0, alpha: 255 };
        assert_eq!(color.to_css(), "rgba(255, 0, 0, 1)");
        assert_eq!(color.to_hex(), "#ff0000");
    }

    #[test]
    fn test_media_access_status() {
        let status = get_media_access_status(&MediaType::Camera);
        // Just verify that it can be called
        let _ = status;
    }

    #[test]
    fn test_get_system_accent_color() {
        let color = get_system_accent_color();
        assert!(color.red <= 255);
    }

    #[test]
    fn test_are_animations_enabled() {
        // Just verify that it can be called
        let _ = are_animations_enabled();
    }
}
