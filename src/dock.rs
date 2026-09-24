//! Dock module
//!
//! Provides functionality similar to the Electron app.dock:
//! - setBadge(text) - set the badge
//! - bounce(type) - bounce the Dock icon
//! - cancelBounce(id) - cancel the bounce
//! - setMenu(menu) - set the Dock menu
//! - show() - show the Dock icon
//! - hide() - hide the Dock icon
//! - isVisible() - check whether the Dock is visible

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

/// Bounce type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BounceType {
    /// Bounce once
    Critical,
    /// Bounce until the user clicks
    Informational,
}

/// Dock menu item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockMenuItem {
    /// Label
    pub label: String,
    /// Whether enabled
    pub enabled: bool,
    /// Submenu
    pub submenu: Option<Vec<DockMenuItem>>,
}

/// Dock manager
///
/// Manages the badge, bounce, and menu of the macOS Dock icon
pub struct DockManager {
    /// Current badge text
    badge: Mutex<String>,
    /// Whether visible
    visible: AtomicBool,
    /// Bounce counter
    bounce_id: AtomicU64,
    /// Dock menu
    menu: Mutex<Vec<DockMenuItem>>,
}

impl DockManager {
    /// Create a new Dock manager
    pub fn new() -> Self {
        Self {
            badge: Mutex::new(String::new()),
            visible: AtomicBool::new(true),
            bounce_id: AtomicU64::new(0),
            menu: Mutex::new(Vec::new()),
        }
    }

    /// Set the Dock badge
    ///
    /// # Parameters
    /// - `text`: Badge text (empty string clears it)
    pub fn set_badge(&self, text: &str) -> Result<()> {
        if let Ok(mut badge) = self.badge.lock() {
            *badge = text.to_string();
        }

        #[cfg(target_os = "macos")]
        self.set_badge_macos(text);

        log::info!("Dock badge set: '{}'", text);
        Ok(())
    }

    /// Get the current badge text
    pub fn get_badge(&self) -> String {
        self.badge.lock()
            .map(|b| b.clone())
            .unwrap_or_default()
    }

    /// Bounce the Dock icon
    ///
    /// # Returns
    /// Bounce ID (can be used to cancel the bounce)
    pub fn bounce(&self, _bounce_type: BounceType) -> u64 {
        let id = self.bounce_id.fetch_add(1, Ordering::SeqCst);

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to set the bouncing of dock preferences to true"])
                .output();
        }

        log::info!("Dock icon bounce triggered (id={})", id);
        id
    }

    /// Cancel the bounce
    pub fn cancel_bounce(&self, _id: u64) {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to set the bouncing of dock preferences to false"])
                .output();
        }

        log::info!("Dock icon bounce cancelled");
    }

    /// Set the Dock menu
    pub fn set_menu(&self, items: Vec<DockMenuItem>) -> Result<()> {
        let len = items.len();
        if let Ok(mut menu) = self.menu.lock() {
            *menu = items;
        }
        log::info!("Dock menu updated ({} items)", len);
        Ok(())
    }

    /// Show the Dock icon
    pub fn show(&self) -> Result<()> {
        self.visible.store(true, Ordering::Relaxed);

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to set the visible of dock preferences to true"])
                .output();
        }

        log::info!("Dock icon shown");
        Ok(())
    }

    /// Hide the Dock icon
    pub fn hide(&self) -> Result<()> {
        self.visible.store(false, Ordering::Relaxed);

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to set the visible of dock preferences to false"])
                .output();
        }

        log::info!("Dock icon hidden");
        Ok(())
    }

    /// Check whether the Dock is visible
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Relaxed)
    }
}

/// Global Dock manager instance
static DOCK_INSTANCE: std::sync::OnceLock<DockManager> = std::sync::OnceLock::new();

/// Get the global Dock manager
pub fn get_dock() -> &'static DockManager {
    DOCK_INSTANCE.get_or_init(DockManager::new)
}
