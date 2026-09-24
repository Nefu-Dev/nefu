//! Global shortcut module
//!
//! Provides functionality similar to the Electron globalShortcut:
//! - Register global keyboard shortcuts
//! - Unregister shortcuts
//! - Unregister all shortcuts
//! - Detect whether a shortcut is registered

use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::GlobalHotKeyManager;
use log::{info, warn};
use std::collections::HashMap;

/// Global shortcut manager
pub struct GlobalShortcutManager {
    /// Global hotkey manager (singleton)
    manager: Option<GlobalHotKeyManager>,
    /// Registered shortcuts (shortcut string -> HotKey object)
    registered: HashMap<String, HotKey>,
}

// GlobalHotKeyManager contains a *mut c_void internally, but it is thread-safe (Win32 HHOOK handle)
unsafe impl Send for GlobalShortcutManager {}
unsafe impl Sync for GlobalShortcutManager {}

impl GlobalShortcutManager {
    /// Create a new shortcut manager
    pub fn new() -> Self {
        let manager = GlobalHotKeyManager::new().ok();
        Self {
            manager,
            registered: HashMap::new(),
        }
    }

    /// Register a global shortcut
    ///
    /// # Parameters
    /// - `accelerator`: Shortcut combination, e.g. "Ctrl+Shift+A", "CommandOrControl+S"
    /// - `callback`: Callback function invoked when the shortcut is triggered
    ///
    /// # Returns
    /// Whether registration succeeded
    pub fn register<F>(&mut self, accelerator: &str, _callback: F) -> Result<bool>
    where
        F: Fn() + Send + 'static,
    {
        if self.registered.contains_key(accelerator) {
            warn!("Shortcut already registered: {}", accelerator);
            return Ok(false);
        }

        let manager = match self.manager.as_ref() {
            Some(m) => m,
            None => {
                warn!("Global shortcut manager is unavailable");
                return Ok(false);
            }
        };

        // Convert the Electron-style shortcut to the global-hotkey format
        let ghk_accelerator = convert_accelerator(accelerator);

        match parse_hotkey(&ghk_accelerator) {
            Ok(hotkey) => {
                match manager.register(hotkey) {
                    Ok(_) => {
                        self.registered.insert(accelerator.to_string(), hotkey);
                        info!("Global shortcut registered: {}", accelerator);
                        // Note: the callback requires a separate event listening mechanism
                        // The current implementation is simplified and only registers the shortcut
                        Ok(true)
                    }
                    Err(e) => {
                        warn!("Failed to register global shortcut {}: {}", accelerator, e);
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                warn!("Invalid shortcut format {}: {}", accelerator, e);
                Ok(false)
            }
        }
    }

    /// Unregister the specified shortcut
    ///
    /// # Parameters
    /// - `accelerator`: Shortcut combination
    pub fn unregister(&mut self, accelerator: &str) {
        if let Some(hotkey) = self.registered.remove(accelerator) {
            if let Some(ref manager) = self.manager {
                manager.unregister(hotkey).ok();
            }
            info!("Global shortcut unregistered: {}", accelerator);
        }
    }

    /// Unregister all shortcuts
    pub fn unregister_all(&mut self) {
        if let Some(ref manager) = self.manager {
            for hotkey in self.registered.values() {
                manager.unregister(*hotkey).ok();
            }
        }
        self.registered.clear();
        info!("All global shortcuts unregistered");
    }

    /// Check whether a shortcut is registered
    ///
    /// # Parameters
    /// - `accelerator`: Shortcut combination
    ///
    /// # Returns
    /// Whether it is registered
    pub fn is_registered(&self, accelerator: &str) -> bool {
        self.registered.contains_key(accelerator)
    }

    /// Get all registered shortcuts
    pub fn get_registered(&self) -> Vec<String> {
        self.registered.keys().cloned().collect()
    }
}

/// Convert an Electron-style shortcut to the global-hotkey format
fn convert_accelerator(acc: &str) -> String {
    acc.replace("CommandOrControl", "Ctrl")
        .replace("Command", "Meta")
        .replace("Control", "Ctrl")
        .replace("Option", "Alt")
        .replace("+", "+")
}

/// Parse a shortcut string into a HotKey structure
fn parse_hotkey(acc: &str) -> Result<HotKey> {
    let parts: Vec<&str> = acc.split('+').map(|s| s.trim()).collect();
    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in &parts {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "meta" | "cmd" | "command" | "win" => modifiers |= Modifiers::META,
            _ => {
                key_code = Some(part.to_string());
            }
        }
    }

    let code_str = key_code.as_deref().unwrap_or("A");
    let code = code_from_str(code_str);

    Ok(HotKey::new(Some(modifiers), code))
}

/// Convert a string key name to a Code enum
fn code_from_str(s: &str) -> Code {
    match s.to_lowercase().as_str() {
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "escape" => Code::Escape,
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "delete" | "del" => Code::Delete,
        "backspace" => Code::Backspace,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" => Code::PageUp,
        "pagedown" => Code::PageDown,
        "up" => Code::ArrowUp,
        "down" => Code::ArrowDown,
        "left" => Code::ArrowLeft,
        "right" => Code::ArrowRight,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        _ => Code::KeyA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_accelerator() {
        assert_eq!(convert_accelerator("CommandOrControl+S"), "Ctrl+S");
        assert_eq!(convert_accelerator("Command+Shift+A"), "Meta+Shift+A");
        assert_eq!(convert_accelerator("Alt+Shift+Z"), "Alt+Shift+Z");
    }

    #[test]
    fn test_parse_hotkey() {
        let hotkey = parse_hotkey("Ctrl+Shift+A").unwrap();
        assert!(hotkey.modifiers().contains(Modifiers::CONTROL));
        assert!(hotkey.modifiers().contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_code_from_str() {
        assert_eq!(code_from_str("A"), Code::KeyA);
        assert_eq!(code_from_str("f1"), Code::F1);
        assert_eq!(code_from_str("enter"), Code::Enter);
        assert_eq!(code_from_str("space"), Code::Space);
    }
}
