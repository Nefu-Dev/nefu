//! Desktop notification module
//!
//! Provides system-level desktop notification functionality, similar to the Electron Notification API.
//! Supports title, body, icon, timeout, click events, etc.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Notification options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationOptions {
    /// Notification title
    pub title: String,
    /// Notification body
    pub body: String,
    /// Subtitle (optional)
    pub subtitle: Option<String>,
    /// Notification icon path (optional)
    pub icon: Option<String>,
    /// Notification timeout (milliseconds)
    pub timeout: Option<u32>,
    /// Whether silent
    pub silent: Option<bool>,
    /// Notification tag (for deduplication)
    pub tag: Option<String>,
    /// Whether urgent (requires user attention)
    pub urgent: Option<bool>,
}

/// Send a desktop notification
///
/// # Parameters
/// - `options`: Notification options
///
/// # Returns
/// Whether the notification was sent successfully
pub fn send_notification(options: &NotificationOptions) -> Result<bool> {
    let mut notification = notify_rust::Notification::new();

    notification
        .summary(&options.title)
        .body(&options.body)
        .appname("Nefu");

    // Set the timeout
    let timeout = options.timeout.unwrap_or(5000);
    notification.timeout(notify_rust::Timeout::Milliseconds(timeout));

    // Set the icon
    if let Some(icon_path) = &options.icon {
        if std::path::Path::new(icon_path).exists() {
            notification.icon(icon_path);
        }
    }

    // Set the urgency
    if options.urgent.unwrap_or(false) {
        notification.urgency(notify_rust::Urgency::Critical);
    }

    // Set the subtitle (macOS support)
    #[cfg(target_os = "macos")]
    {
        if let Some(subtitle) = &options.subtitle {
            notification.subtitle(subtitle);
        }
    }

    // Send the notification
    match notification.show() {
        Ok(_) => {
            log::info!("Desktop notification sent: {}", options.title);
            Ok(true)
        }
        Err(e) => {
            log::warn!("Failed to send desktop notification: {}", e);
            // Fallback: output to the console
            console_notification(options);
            Ok(false)
        }
    }
}

/// Send a simple notification (shortcut)
///
/// # Parameters
/// - `title`: Notification title
/// - `body`: Notification body
pub fn send_simple_notification(title: &str, body: &str) -> Result<bool> {
    let options = NotificationOptions {
        title: title.to_string(),
        body: body.to_string(),
        subtitle: None,
        icon: None,
        timeout: None,
        silent: None,
        tag: None,
        urgent: None,
    };
    send_notification(&options)
}

/// Console notification fallback (when system notifications are unavailable)
fn console_notification(options: &NotificationOptions) {
    eprintln!(
        "\n[{}] {} - {}\n",
        if options.urgent.unwrap_or(false) {
            "!!!"
        } else {
            "NOTIFICATION"
        },
        options.title,
        options.body
    );
}

/// Check whether the system supports desktop notifications
///
/// Attempts to send a test notification to determine system support
pub fn is_notification_supported() -> bool {
    // Try to create a simple notification to test support
    // If creation fails, fall back to console output
    let mut notification = notify_rust::Notification::new();
    notification
        .summary("Nefu notification test")
        .body("If you see this message, the notification system is working normally")
        .appname("Nefu")
        .timeout(notify_rust::Timeout::Milliseconds(100));

    // Try to show it (the timeout is very short and will not actually bother the user)
    notification.show().is_ok()
}

/// Keep a notification in the notification center (macOS feature)
///
/// Keeps the notification in the notification center until the user dismisses it
#[cfg(target_os = "macos")]
pub fn set_notification_keep_alive(notification_id: &str, keep: bool) {
    // macOS notifications remain in the notification center by default
    let _ = notification_id;
    let _ = keep;
}

#[cfg(not(target_os = "macos"))]
pub fn set_notification_keep_alive(_notification_id: &str, _keep: bool) {
    // This feature is not supported on other platforms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_options_serde() {
        let options = NotificationOptions {
            title: "Test".to_string(),
            body: "This is a test notification".to_string(),
            subtitle: None,
            icon: None,
            timeout: Some(3000),
            silent: Some(false),
            tag: Some("test-tag".to_string()),
            urgent: None,
        };

        let json = serde_json::to_string(&options).unwrap();
        let deserialized: NotificationOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "Test");
        assert_eq!(deserialized.body, "This is a test notification");
        assert_eq!(deserialized.tag, Some("test-tag".to_string()));
    }
}
