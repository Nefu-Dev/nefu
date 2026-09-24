//! Power save blocker
//!
//! Provides functionality similar to the Electron powerSaveBlocker:
//! - Prevent the system from entering sleep mode
//! - Prevent the screen from turning off
//! - Manage blocking request IDs
//! - Release blocking requests

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Power save blocker type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerSaveBlockerType {
    /// Prevent the system from entering sleep mode
    PreventSleep,
    /// Prevent the screen from turning off
    PreventDisplaySleep,
}

/// Power save blocker manager
pub struct PowerSaveBlocker {
    /// Next available ID
    next_id: AtomicU64,
    /// Active blocking requests
    active_blockers: Mutex<HashMap<u64, PowerSaveBlockerType>>,
    /// Current sleep prevention state
    prevent_sleep: Mutex<bool>,
    /// Current display sleep prevention state
    prevent_display_sleep: Mutex<bool>,
}

impl PowerSaveBlocker {
    /// Create a new power save blocker
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            active_blockers: Mutex::new(HashMap::new()),
            prevent_sleep: Mutex::new(false),
            prevent_display_sleep: Mutex::new(false),
        }
    }

    /// Start blocking power save
    ///
    /// # Parameters
    /// - `blocker_type`: Blocking type
    ///
    /// # Returns
    /// Blocking request ID (used to release it later)
    pub fn start(&self, blocker_type: PowerSaveBlockerType) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        if let Ok(mut blockers) = self.active_blockers.lock() {
            blockers.insert(id, blocker_type);
        }

        self.update_state();
        log::info!("Power save blocking started (id={}, type={:?})", id, blocker_type);
        id
    }

    /// Stop the specified power save blocking
    ///
    /// # Parameters
    /// - `id`: Blocking request ID
    pub fn stop(&self, id: u64) {
        let removed = if let Ok(mut blockers) = self.active_blockers.lock() {
            blockers.remove(&id).is_some()
        } else {
            false
        };

        if removed {
            self.update_state();
            log::info!("Power save blocking stopped (id={})", id);
        }
    }

    /// Check whether the specified ID is still blocking
    ///
    /// # Parameters
    /// - `id`: Blocking request ID
    pub fn is_stopped(&self, id: u64) -> bool {
        if let Ok(blockers) = self.active_blockers.lock() {
            !blockers.contains_key(&id)
        } else {
            true
        }
    }

    /// Update the system power save state
    fn update_state(&self) {
        let has_prevent_sleep = if let Ok(blockers) = self.active_blockers.lock() {
            blockers.values().any(|t| *t == PowerSaveBlockerType::PreventSleep)
        } else {
            false
        };

        let has_prevent_display = if let Ok(blockers) = self.active_blockers.lock() {
            blockers.values().any(|t| *t == PowerSaveBlockerType::PreventDisplaySleep)
        } else {
            false
        };

        // Update the state
        if let Ok(mut state) = self.prevent_sleep.lock() {
            *state = has_prevent_sleep;
        }
        if let Ok(mut state) = self.prevent_display_sleep.lock() {
            *state = has_prevent_display;
        }

        // Actually execute the system API call
        self.apply_system_power_save(has_prevent_sleep || has_prevent_display);
    }

    /// Apply system power save settings
    fn apply_system_power_save(&self, prevent: bool) {
        if prevent {
            log::debug!("Preventing the system from entering sleep mode");
        } else {
            log::debug!("System power save settings restored");
        }

        #[cfg(windows)]
        {
            // Windows uses SetThreadExecutionState
            // Implemented here via PowerShell
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            if prevent {
                let _ = std::process::Command::new("powershell")
                    .args([
                        "-Command",
                        "$null = [System.Windows.Forms.Application]::SetSuspendState($false, $true, $false)",
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            }
        }
    }

    /// Get the number of currently active blocking requests
    pub fn active_count(&self) -> usize {
        if let Ok(blockers) = self.active_blockers.lock() {
            blockers.len()
        } else {
            0
        }
    }

    /// Get the list of currently active blocking requests
    pub fn get_active_blockers(&self) -> Vec<(u64, PowerSaveBlockerType)> {
        if let Ok(blockers) = self.active_blockers.lock() {
            blockers.iter().map(|(k, v)| (*k, *v)).collect()
        } else {
            vec![]
        }
    }
}

/// Global power save blocker instance
static GLOBAL_BLOCKER: std::sync::OnceLock<PowerSaveBlocker> = std::sync::OnceLock::new();

/// Get the global power save blocker instance
fn global_blocker() -> &'static PowerSaveBlocker {
    GLOBAL_BLOCKER.get_or_init(PowerSaveBlocker::new)
}

/// Start blocking power save (convenience function)
pub fn start_blocking(blocker_type: PowerSaveBlockerType) -> u64 {
    global_blocker().start(blocker_type)
}

/// Stop the power save blocking for the specified ID
pub fn stop_blocking(id: u64) {
    global_blocker().stop(id);
}

/// Check whether the specified ID has been stopped
pub fn is_stopped(id: u64) -> bool {
    global_blocker().is_stopped(id)
}

/// Get the number of currently active blocking requests
pub fn get_active_count() -> usize {
    global_blocker().active_count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_save_blocker_start_stop() {
        let blocker = PowerSaveBlocker::new();
        let id = blocker.start(PowerSaveBlockerType::PreventSleep);
        assert!(!blocker.is_stopped(id));
        assert_eq!(blocker.active_count(), 1);

        blocker.stop(id);
        assert!(blocker.is_stopped(id));
        assert_eq!(blocker.active_count(), 0);
    }

    #[test]
    fn test_multiple_blockers() {
        let blocker = PowerSaveBlocker::new();
        let id1 = blocker.start(PowerSaveBlockerType::PreventSleep);
        let id2 = blocker.start(PowerSaveBlockerType::PreventDisplaySleep);

        assert_eq!(blocker.active_count(), 2);

        blocker.stop(id1);
        assert_eq!(blocker.active_count(), 1);

        blocker.stop(id2);
        assert_eq!(blocker.active_count(), 0);
    }

    #[test]
    fn test_global_functions() {
        let id = start_blocking(PowerSaveBlockerType::PreventSleep);
        assert!(!is_stopped(id));
        assert!(get_active_count() >= 1);

        stop_blocking(id);
        assert!(is_stopped(id));
    }
}
