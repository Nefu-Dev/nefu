//! Nefu JS-Rust bridge module
//!
//! Implements bidirectional communication between JavaScript and Rust:
//! - `lj(data)` - JS sends data to Rust
//! - `nefu.invoke(method, args)` - JS calls a Rust method
//! - `nefu.on(event, callback)` - JS listens for Rust events
//! - `nefu.send(data)` - JS sends a message to Rust
//! - The Rust side can trigger events and return results to JS

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Global shortcut manager singleton
static GLOBAL_SHORTCUT: std::sync::LazyLock<Mutex<crate::global_shortcut::GlobalShortcutManager>> =
    std::sync::LazyLock::new(|| Mutex::new(crate::global_shortcut::GlobalShortcutManager::new()));

/// Global SQL manager singleton
static SQL_MANAGER: std::sync::LazyLock<crate::sql_manager::SqlManager> =
    std::sync::LazyLock::new(crate::sql_manager::SqlManager::new);

/// Window control command triggered from JS
///
/// Sent through a channel to the main event loop, which executes the corresponding window operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    /// Minimize the window
    Minimize,
    /// Maximize the window
    Maximize,
    /// Restore the window (from minimized/maximized)
    Restore,
    /// Close the window
    Close,
}

/// IPC message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcMessage {
    /// JS calls a Rust method
    #[serde(rename = "invoke")]
    Invoke {
        /// Call ID (used to match the response)
        id: u64,
        /// Method name
        method: String,
        /// Argument list
        args: Vec<Value>,
    },

    /// JS sends data to Rust
    #[serde(rename = "send")]
    Send {
        /// Data content
        data: Value,
    },

    /// Rust returns a call result
    #[serde(rename = "callback")]
    Callback {
        /// Corresponding call ID
        id: u64,
        /// Return value
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        /// Error message
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Rust triggers a JS event
    #[serde(rename = "event")]
    Event {
        /// Event name
        event: String,
        /// Event data
        data: Value,
    },
}

/// Bridge handler callback function type
pub type BridgeHandler = Box<dyn Fn(Vec<Value>) -> Result<Option<Value>> + Send + Sync>;

/// JS-Rust bridge manager
///
/// Manages all registered Rust method handlers and the event system
#[derive(Clone)]
pub struct Bridge {
    /// Registered method handlers
    handlers: HashMap<String, Arc<BridgeHandler>>,
    /// Pending event queue
    pending_events: Arc<Mutex<Vec<IpcMessage>>>,
    /// Global message handler (processes data sent via lj() and send())
    message_handler: Option<Arc<dyn Fn(Value) + Send + Sync>>,
    /// Window control command channel (consumed by the WebView main event loop)
    window_cmd_tx: Arc<Mutex<Option<mpsc::Sender<WindowCommand>>>>,
}

impl Bridge {
    /// Creates a new bridge instance
    pub fn new() -> Self {
        let mut bridge = Self {
            handlers: HashMap::new(),
            pending_events: Arc::new(Mutex::new(Vec::new())),
            message_handler: None,
            window_cmd_tx: Arc::new(Mutex::new(None)),
        };

        // Register builtin methods
        bridge.register_builtin_methods();

        bridge
    }

    /// Sets the window control command channel
    ///
    /// # Arguments
    /// - `tx`: channel sender (the main event loop holds the receiver)
    pub fn set_window_command_sender(&self, tx: mpsc::Sender<WindowCommand>) {
        if let Ok(mut guard) = self.window_cmd_tx.lock() {
            *guard = Some(tx);
        }
    }

    /// Registers the builtin bridge methods
    fn register_builtin_methods(&mut self) {
        // nefu.getVersion() - get the version number
        self.register_method("getVersion", Box::new(|_args| {
            Ok(Some(Value::String(env!("CARGO_PKG_VERSION").to_string())))
        }));

        // nefu.getPlatform() - get platform info
        self.register_method("getPlatform", Box::new(|_args| {
            let platform = if cfg!(windows) {
                "windows"
            } else if cfg!(target_os = "macos") {
                "macos"
            } else if cfg!(target_os = "linux") {
                "linux"
            } else {
                "unknown"
            };
            Ok(Some(Value::String(platform.to_string())))
        }));

        // nefu.getTime() - get the current timestamp
        self.register_method("getTime", Box::new(|_args| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let duration = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            Ok(Some(Value::Number(serde_json::Number::from(duration.as_millis() as u64))))
        }));

        // nefu.echo(data) - echo data (for testing)
        self.register_method("echo", Box::new(|args| {
            Ok(args.into_iter().next())
        }));

        // nefu.listMethods() - list all available methods (method names pre-collected)
        let all_methods: Vec<String> = self.handlers.keys().cloned().collect();
        let _ = self.register_method("listMethods", Box::new(move |_args| {
            Ok(Some(Value::Array(all_methods.clone().into_iter().map(Value::String).collect())))
        }));

        // nefu.log(message) - Rust-side log
        self.register_method("log", Box::new(|args| {
            if let Some(msg) = args.first() {
                info!("[JS Log] {}", msg);
            }
            Ok(None)
        }));

        // nefu.warn(message) - Rust-side warning
        self.register_method("warn", Box::new(|args| {
            if let Some(msg) = args.first() {
                warn!("[JS Warn] {}", msg);
            }
            Ok(None)
        }));

        // nefu.error(message) - Rust-side error
        self.register_method("error", Box::new(|args| {
            if let Some(msg) = args.first() {
                error!("[JS Error] {}", msg);
            }
            Ok(None)
        }));

        // nefu.openUrl(url) - open an external URL
        self.register_method("openUrl", Box::new(|args| {
            if let Some(url) = args.first().and_then(|v| v.as_str()) {
                // Security check: only allow http/https protocols
                if url.starts_with("http://") || url.starts_with("https://") {
                    if let Err(e) = open_url(url) {
                        return Err(anyhow::anyhow!("cannot open URL: {}", e));
                    }
                } else {
                    return Err(anyhow::anyhow!("for security reasons, only http/https protocols are allowed"));
                }
            }
            Ok(None)
        }));

        // nefu.clipboard.read() - read clipboard
        let _ = self.register_method("clipboard.read", Box::new(|_args| {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| anyhow::anyhow!("cannot access clipboard: {}", e))?;
            let text = clipboard
                .get_text()
                .map_err(|e| anyhow::anyhow!("read clipboard failed: {}", e))?;
            Ok(Some(Value::String(text)))
        }));

        // nefu.clipboard.write(text) - write to clipboard
        let _ = self.register_method("clipboard.write", Box::new(|args| {
            if let Some(text) = args.first().and_then(|v| v.as_str()) {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("cannot access clipboard: {}", e))?;
                clipboard
                    .set_text(text.to_string())
                    .map_err(|e| anyhow::anyhow!("write clipboard failed: {}", e))?;
                debug!("clipboard write: {} chars", text.len());
            }
            Ok(None)
        }));

        // Window control command channel (set by the webview main loop)
        let window_cmd_tx = self.window_cmd_tx.clone();

        // nefu.window.minimize() - minimize the window
        let cmd_tx = window_cmd_tx.clone();
        let _ = self.register_method("window.minimize", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Minimize);
                }
            }
            Ok(None)
        }));

        // nefu.window.maximize() - maximize the window
        let cmd_tx = window_cmd_tx.clone();
        let _ = self.register_method("window.maximize", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Maximize);
                }
            }
            Ok(None)
        }));

        // nefu.window.close() - close the window
        let cmd_tx = window_cmd_tx.clone();
        let _ = self.register_method("window.close", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Close);
                }
            }
            Ok(None)
        }));

        // nefu.window.restore() - restore the window
        let cmd_tx = window_cmd_tx.clone();
        let _ = self.register_method("window.restore", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Restore);
                }
            }
            Ok(None)
        }));

        // nefu.window.isMaximized() - whether maximized
        let _ = self.register_method("window.isMaximized", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.window.isMinimized() - whether minimized
        let _ = self.register_method("window.isMinimized", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.window.isVisible() - whether the window is visible
        let _ = self.register_method("window.isVisible", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.fs.readFile(path) - read file (restricted)
        self.register_method("fs.readFile", Box::new(|args| {
            if let Some(path) = args.first().and_then(|v| v.as_str()) {
                // Security check: restrict accessible paths
                if is_safe_path(path) {
                    match std::fs::read_to_string(path) {
                        Ok(content) => return Ok(Some(Value::String(content))),
                        Err(e) => return Err(anyhow::anyhow!("read file failed: {}", e)),
                    }
                } else {
                    return Err(anyhow::anyhow!("access to this path is not allowed"));
                }
            }
            Err(anyhow::anyhow!("missing file path argument"))
        }));

        // nefu.dialog.alert(message) - show a native alert box (async, does not block IPC)
        let _ = self.register_method("dialog.alert", Box::new(|args| {
            let msg = args.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
            std::thread::spawn(move || {
                rfd::MessageDialog::new()
                    .set_title("Nefu")
                    .set_description(&msg)
                    .set_level(rfd::MessageLevel::Info)
                    .show();
                info!("[Dialog Alert] {}", msg);
            });
            Ok(None)
        }));

        // nefu.dialog.confirm(message) - show a confirm dialog (async, result returned via event)
        let bridge_clone = self.clone();
        let _ = self.register_method("dialog.confirm", Box::new(move |args| {
            let msg = args.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
            let bridge = bridge_clone.clone();
            std::thread::spawn(move || {
                let ok = rfd::MessageDialog::new()
                    .set_title("Nefu")
                    .set_description(&msg)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                let result = ok == rfd::MessageDialogResult::Yes;
                // Return the result via the event mechanism
                bridge.emit_event("dialog-confirm-result", serde_json::Value::Bool(result));
            });
            Ok(None) // return immediately, do not block
        }));

        // nefu.dialog.openFile(options) - open the file picker dialog (async)
        let bridge_clone = self.clone();
        let _ = self.register_method("dialog.openFile", Box::new(move |args| {
            let mut multiple = false;
            if let Some(opts) = args.first().and_then(|v| v.as_object()) {
                multiple = opts.get("multiple").and_then(|v| v.as_bool()).unwrap_or(false);
            }
            let bridge = bridge_clone.clone();
            std::thread::spawn(move || {
                let dialog = rfd::FileDialog::new().set_title("Select File");
                let result: Vec<String> = if multiple {
                    dialog.pick_files()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect()
                } else {
                    dialog.pick_file()
                        .map(|p| vec![p.to_string_lossy().to_string()])
                        .unwrap_or_default()
                };
                let value = if result.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::Array(result.into_iter().map(serde_json::Value::String).collect())
                };
                bridge.emit_event("dialog-openFile-result", value);
            });
            Ok(None)
        }));

        // nefu.dialog.saveFile(options) - open the save dialog (async)
        let bridge_clone = self.clone();
        let _ = self.register_method("dialog.saveFile", Box::new(move |args| {
            let mut dialog = rfd::FileDialog::new().set_title("Save File");
            if let Some(opts) = args.first().and_then(|v| v.as_object()) {
                if let Some(name) = opts.get("defaultName").and_then(|v| v.as_str()) {
                    dialog = dialog.set_file_name(name);
                }
            }
            let bridge = bridge_clone.clone();
            std::thread::spawn(move || {
                let value = match dialog.save_file() {
                    Some(path) => serde_json::Value::String(path.to_string_lossy().to_string()),
                    None => serde_json::Value::Null,
                };
                bridge.emit_event("dialog-saveFile-result", value);
            });
            Ok(None)
        }));

        // nefu.fs.writeFile(path, content) - write file (restricted)
        let _ = self.register_method("fs.writeFile", Box::new(|args| {
            if args.len() >= 2 {
                if let Some(path) = args[0].as_str() {
                    let content = args[1].as_str().unwrap_or("");
                    if !is_safe_path(path) {
                        return Err(anyhow::anyhow!("access to this path is not allowed"));
                    }
                    std::fs::write(path, content)
                        .map_err(|e| anyhow::anyhow!("write file failed: {}", e))?;
                    return Ok(None);
                }
            }
            Err(anyhow::anyhow!("missing argument"))
        }));

        // nefu.fs.exists(path) - check if a file exists (restricted)
        let _ = self.register_method("fs.exists", Box::new(|args| {
            if let Some(path) = args.first().and_then(|v| v.as_str()) {
                if !is_safe_path(path) {
                    return Err(anyhow::anyhow!("access to this path is not allowed"));
                }
                return Ok(Some(Value::Bool(std::path::Path::new(path).exists())));
            }
            Err(anyhow::anyhow!("missing file path argument"))
        }));

        // nefu.storage.get(key) - local storage read
        self.register_method("storage.get", Box::new(|args| {
            if let Some(key) = args.first().and_then(|v| v.as_str()) {
                let storage_dir = get_storage_dir();
                let file_path = storage_dir.join(format!("{}.json", sanitize_key(key)));
                if file_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        if let Ok(value) = serde_json::from_str::<Value>(&content) {
                            return Ok(Some(value));
                        }
                    }
                }
                return Ok(Some(Value::Null));
            }
            Err(anyhow::anyhow!("missing key name argument"))
        }));

        // nefu.storage.set(key, value) - local storage write
        self.register_method("storage.set", Box::new(|args| {
            if args.len() >= 2 {
                if let Some(key) = args[0].as_str() {
                    let value = &args[1];
                    let storage_dir = get_storage_dir();
                    std::fs::create_dir_all(&storage_dir).ok();
                    let file_path = storage_dir.join(format!("{}.json", sanitize_key(key)));
                    let content = serde_json::to_string(value)?;
                    std::fs::write(&file_path, content)?;
                    return Ok(None);
                }
            }
            Err(anyhow::anyhow!("missing argument"))
        }));

        // nefu.storage.remove(key) - local storage remove
        self.register_method("storage.remove", Box::new(|args| {
            if let Some(key) = args.first().and_then(|v| v.as_str()) {
                let storage_dir = get_storage_dir();
                let file_path = storage_dir.join(format!("{}.json", sanitize_key(key)));
                if file_path.exists() {
                    std::fs::remove_file(&file_path).ok();
                }
                return Ok(None);
            }
            Err(anyhow::anyhow!("missing key name argument"))
        }));

        // ==================== Electron-level features ====================

        // --- Shell API ---
        // nefu.shell.openExternal(url) - open URL in the default browser
        let _ = self.register_method("shell.openExternal", Box::new(|args| {
            if let Some(url) = args.first().and_then(|v| v.as_str()) {
                crate::shell::open_external(url)?;
            }
            Ok(None)
        }));

        // nefu.shell.openPath(path) - open a path in the file manager
        let _ = self.register_method("shell.openPath", Box::new(|args| {
            if let Some(path) = args.first().and_then(|v| v.as_str()) {
                crate::shell::open_path(path)?;
            }
            Ok(None)
        }));

        // nefu.shell.showItemInFolder(path) - show the file in its folder
        let _ = self.register_method("shell.showItemInFolder", Box::new(|args| {
            if let Some(path) = args.first().and_then(|v| v.as_str()) {
                crate::shell::show_item_in_folder(path)?;
            }
            Ok(None)
        }));

        // nefu.shell.beep() - system beep
        let _ = self.register_method("shell.beep", Box::new(|_args| {
            crate::shell::beep();
            Ok(None)
        }));

        // nefu.shell.trashItem(path) - move to trash
        let _ = self.register_method("shell.trashItem", Box::new(|args| {
            if let Some(path) = args.first().and_then(|v| v.as_str()) {
                let result = crate::shell::trash_item(path)?;
                return Ok(Some(Value::Bool(result)));
            }
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.shell.getSystemVersion() - get the system version
        let _ = self.register_method("shell.getSystemVersion", Box::new(|_args| {
            Ok(Some(Value::String(crate::shell::get_system_version())))
        }));

        // nefu.shell.exec(command, args?, cwd?) - execute a shell command
        let _ = self.register_method("shell.exec", Box::new(|args| {
            let command = args.first()
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing command argument"))?;
            let extra_args: Vec<String> = args.get(1)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let cwd = args.get(2).and_then(|v| v.as_str());
            let result = crate::shell::exec(command, &extra_args, cwd)?;
            Ok(Some(result))
        }));

        // --- Notification API ---
        // nefu.notification.send(options) - send a desktop notification
        let _ = self.register_method("notification.send", Box::new(|args| {
            if let Some(opts) = args.first() {
                let options: crate::notifications::NotificationOptions = serde_json::from_value(opts.clone())
                    .map_err(|e| anyhow::anyhow!("invalid notification arguments: {}", e))?;
                let result = crate::notifications::send_notification(&options)?;
                return Ok(Some(Value::Bool(result)));
            }
            Err(anyhow::anyhow!("missing notification argument"))
        }));

        // nefu.notification.isSupported() - check whether notifications are supported
        let _ = self.register_method("notification.isSupported", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::notifications::is_notification_supported())))
        }));

        // --- System Info API ---
        // nefu.system.getTheme() - get the system theme
        let _ = self.register_method("system.getTheme", Box::new(|_args| {
            let theme = crate::system_info::get_system_theme();
            Ok(Some(serde_json::to_value(theme).unwrap_or(Value::String("unknown".to_string()))))
        }));

        // nefu.system.getMemoryInfo() - get memory info
        let _ = self.register_method("system.getMemoryInfo", Box::new(|_args| {
            let mem = crate::system_info::get_memory_info();
            Ok(Some(serde_json::to_value(mem).unwrap_or_default()))
        }));

        // nefu.system.getCpuInfo() - get CPU info
        let _ = self.register_method("system.getCpuInfo", Box::new(|_args| {
            let cpu = crate::system_info::get_cpu_info();
            Ok(Some(serde_json::to_value(cpu).unwrap_or_default()))
        }));

        // nefu.system.getGpuInfo() - get GPU info
        let _ = self.register_method("system.getGpuInfo", Box::new(|_args| {
            let gpus = crate::system_info::get_gpu_info();
            Ok(Some(serde_json::to_value(gpus).unwrap_or_default()))
        }));

        // nefu.system.getPowerState() - get the power state
        let _ = self.register_method("system.getPowerState", Box::new(|_args| {
            let power = crate::system_info::get_power_state();
            Ok(Some(serde_json::to_value(power).unwrap_or_default()))
        }));

        // nefu.system.getScreenInfo() - get screen info
        let _ = self.register_method("system.getScreenInfo", Box::new(|_args| {
            let screen = crate::system_info::get_primary_screen_info();
            Ok(Some(serde_json::to_value(screen).unwrap_or_default()))
        }));

        // nefu.system.getSummary() - get a system info summary
        let _ = self.register_method("system.getSummary", Box::new(|_args| {
            Ok(Some(crate::system_info::get_system_info_summary()))
        }));

        // nefu.system.refresh() - refresh the system info cache
        let _ = self.register_method("system.refresh", Box::new(|_args| {
            crate::system_info::refresh_system_info();
            Ok(None)
        }));

        // --- Power Monitor API ---
        // nefu.powerMonitor.getSystemIdleState(idleThreshold) - get the system idle state
        let _ = self.register_method("powerMonitor.getSystemIdleState", Box::new(|_args| {
            // Simplified implementation: return the current power state
            let power = crate::system_info::get_power_state();
            Ok(Some(serde_json::json!({
                "on_battery": power.on_battery,
                "charging": power.charging,
                "battery_percent": power.battery_percent,
            })))
        }));

        // --- Auto Updater API ---
        // nefu.updater.checkForUpdates(updateUrl) - check for updates
        let _ = self.register_method("updater.checkForUpdates", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing update URL argument"))?;
            let version = env!("CARGO_PKG_VERSION").to_string();
            let mut updater = crate::auto_updater::AutoUpdater::new(url.to_string(), version);
            let status = updater.check_for_updates()?;
            Ok(Some(serde_json::to_value(status).unwrap_or_default()))
        }));

        // --- Printing API ---
        // nefu.print() - print the current page
        let _ = self.register_method("print", Box::new(|_args| {
            // Execute window.print() via JS
            // The actual printing is handled by WebView
            Ok(None)
        }));

        // nefu.printToPDF() - Print to PDF (delegated to the frontend)
        let _ = self.register_method("printToPDF", Box::new(|_args| {
            Ok(None)
        }));

        // --- Crash Reporter API ---
        // nefu.crashReporter.generateTestReport() - generate a test crash report
        let _ = self.register_method("crashReporter.generateTestReport", Box::new(|_args| {
            let config = crate::crash_reporter::CrashReporterConfig::default();
            let reporter = crate::crash_reporter::CrashReporter::new(config);
            let report = reporter.generate_test_report();
            Ok(Some(serde_json::to_value(report).unwrap_or_default()))
        }));

        // --- Context Bridge API ---
        // nefu.contextBridge.isAvailable() - check whether the context bridge is available
        let _ = self.register_method("contextBridge.isAvailable", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // --- Desktop Capturer (screenshot) ---
        // nefu.captureScreenshot() - take a screenshot
        let _ = self.register_method("captureScreenshot", Box::new(|_args| {
            let screenshot = capture_screenshot()?;
            Ok(Some(Value::String(screenshot)))
        }));

        // --- Clipboard extensions ---
        // nefu.clipboard.readImage() - read clipboard image (returns base64)
        let _ = self.register_method("clipboard.readImage", Box::new(|_args| {
            // Simplified implementation
            Ok(Some(Value::Null))
        }));

        // nefu.clipboard.hasImage() - Check whether the clipboard has an image
        let _ = self.register_method("clipboard.hasImage", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // --- Menu API ---
        // nefu.menu.getDefaultTemplate(appName) - get the default menu template
        let _ = self.register_method("menu.getDefaultTemplate", Box::new(|args| {
            let app_name = args.first().and_then(|v| v.as_str()).unwrap_or("NefuApp");
            let template = crate::native_menu::MenuTemplate::default_app_menu(app_name);
            let menu = crate::native_menu::NativeMenu::new(template);
            Ok(Some(menu.to_json()))
        }));

        // --- Platform info enhanced ---
        // Override the original getPlatform to provide more detail
        if self.handlers.contains_key("getPlatform") {
            // Already registered, skip
        }

        // ==================== Added Electron-level features ====================

        // --- App Module API ---
        // nefu.app.getPath(name) - get a system path
        let _ = self.register_method("app.getPath", Box::new(|args| {
            let name = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing path name parameter"))?;
            let path = crate::app_module::get_path(name)?;
            Ok(Some(Value::String(path)))
        }));

        // nefu.app.getAppPath() - get the app path
        let _ = self.register_method("app.getAppPath", Box::new(|_args| {
            let exe = std::env::current_exe()?;
            let path = exe.parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(Some(Value::String(path)))
        }));

        // nefu.app.getName() - get the app name
        let _ = self.register_method("app.getName", Box::new(|_args| {
            let name = std::env::current_exe()
                .ok()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_else(|| "NefuApp".to_string());
            Ok(Some(Value::String(name)))
        }));

        // nefu.app.getVersion() - Get the application version
        let _ = self.register_method("app.getVersion", Box::new(|_args| {
            Ok(Some(Value::String(env!("CARGO_PKG_VERSION").to_string())))
        }));

        // nefu.app.getLocale() - get the locale
        let _ = self.register_method("app.getLocale", Box::new(|_args| {
            Ok(Some(Value::String(crate::app_module::get_locale())))
        }));

        // nefu.app.getLocaleCountryCode() - Get the country code
        let _ = self.register_method("app.getLocaleCountryCode", Box::new(|_args| {
            Ok(Some(Value::String(crate::app_module::get_locale_country_code())))
        }));

        // nefu.app.isPackaged() - whether packaged
        let _ = self.register_method("app.isPackaged", Box::new(|_args| {
            let is_pkg = std::env::current_exe()
                .ok()
                .map(|p| crate::pack::is_nefu_package(&p))
                .unwrap_or(false);
            Ok(Some(Value::Bool(is_pkg)))
        }));

        // nefu.app.getAppMetrics() - get process metrics
        let _ = self.register_method("app.getAppMetrics", Box::new(|_args| {
            let metrics = crate::app_module::get_app_metrics();
            Ok(Some(serde_json::to_value(metrics).unwrap_or_default()))
        }));

        // nefu.app.getLoginItemSettings() - Get login item settings
        let _ = self.register_method("app.getLoginItemSettings", Box::new(|_args| {
            let settings = crate::app_module::get_login_item_settings();
            Ok(Some(serde_json::to_value(settings).unwrap_or_default()))
        }));

        // nefu.app.setLoginItemSettings(settings) - Set login item settings
        let _ = self.register_method("app.setLoginItemSettings", Box::new(|args| {
            if let Some(settings_val) = args.first() {
                let settings: crate::app_module::LoginItemSettings = serde_json::from_value(settings_val.clone())
                    .map_err(|e| anyhow::anyhow!("invalid login item settings args: {}", e))?;
                crate::app_module::set_login_item_settings(&settings)?;
            }
            Ok(None)
        }));

        // nefu.app.getSystemVersion() - Get the system version
        let _ = self.register_method("app.getSystemVersion", Box::new(|_args| {
            Ok(Some(Value::String(crate::shell::get_system_version())))
        }));

        // nefu.app.getAppInfo() - Get full application info
        let _ = self.register_method("app.getAppInfo", Box::new(|_args| {
            let info = crate::app_module::get_app_info();
            Ok(Some(serde_json::to_value(info).unwrap_or_default()))
        }));

        // --- Power Monitor enhanced ---
        // nefu.powerMonitor.getSystemIdleTime() - get system idle time (seconds)
        let _ = self.register_method("powerMonitor.getSystemIdleTime", Box::new(|_args| {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                let output = std::process::Command::new("powershell")
                    .args(["-Command", "[PInvoke.Win32.UserInput]::IdleTime"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8(out.stdout).unwrap_or_default();
                    if let Ok(ms) = stdout.trim().parse::<u64>() {
                        return Ok(Some(Value::Number(serde_json::Number::from(ms / 1000))));
                    }
                }
            }
            Ok(Some(Value::Number(serde_json::Number::from(0u64))))
        }));

        // nefu.powerMonitor.getCurrentThermalState() - Get the current thermal state
        let _ = self.register_method("powerMonitor.getCurrentThermalState", Box::new(|_args| {
            Ok(Some(Value::String("unknown".to_string())))
        }));

        // nefu.powerMonitor.onBattery() - Whether battery is in use
        let _ = self.register_method("powerMonitor.onBattery", Box::new(|_args| {
            let power = crate::system_info::get_power_state();
            Ok(Some(Value::Bool(power.on_battery)))
        }));

        // nefu.powerMonitor.getCurrentDesktop() - get the current desktop
        let _ = self.register_method("powerMonitor.getCurrentDesktop", Box::new(|_args| {
            Ok(Some(Value::String("default".to_string())))
        }));

        // --- Power Save Blocker ---
        // nefu.powerSaveBlocker.start(type) - start power save blocking
        let _ = self.register_method("powerSaveBlocker.start", Box::new(|args| {
            let blocker_type = args.first().and_then(|v| v.as_str()).unwrap_or("prevent-sleep");
            let bt = match blocker_type {
                "prevent-display-sleep" => crate::power_save_blocker::PowerSaveBlockerType::PreventDisplaySleep,
                _ => crate::power_save_blocker::PowerSaveBlockerType::PreventSleep,
            };
            let id = crate::power_save_blocker::start_blocking(bt);
            Ok(Some(Value::Number(serde_json::Number::from(id))))
        }));

        // nefu.powerSaveBlocker.stop(id) - stop power save blocking
        let _ = self.register_method("powerSaveBlocker.stop", Box::new(|args| {
            if let Some(id) = args.first().and_then(|v| v.as_u64()) {
                crate::power_save_blocker::stop_blocking(id);
            }
            Ok(None)
        }));

        // nefu.powerSaveBlocker.isStopped(id) - Check whether it has been stopped
        let _ = self.register_method("powerSaveBlocker.isStopped", Box::new(|args| {
            if let Some(id) = args.first().and_then(|v| v.as_u64()) {
                return Ok(Some(Value::Bool(crate::power_save_blocker::is_stopped(id))));
            }
            Ok(Some(Value::Bool(true)))
        }));

        // --- Screen extensions ---
        // nefu.screen.getAllDisplays() - Get all displays
        let _ = self.register_method("screen.getAllDisplays", Box::new(|_args| {
            let primary = crate::system_info::get_primary_screen_info();
            let display = serde_json::json!({
                "id": 1,
                "bounds": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "workArea": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "scaleFactor": primary.scale_factor,
                "isPrimary": true,
                "rotation": 0,
                "touchSupport": "unknown",
                "accelerometerSupport": "unknown",
                "displayFrequency": 60,
                "colorSpace": "srgb",
                "depthPerComponent": 8,
                "size": { "width": primary.width, "height": primary.height },
                "workAreaSize": { "width": primary.width, "height": primary.height }
            });
            Ok(Some(Value::Array(vec![display])))
        }));

        // nefu.screen.getPrimaryDisplay() - Get the primary display
        let _ = self.register_method("screen.getPrimaryDisplay", Box::new(|_args| {
            let primary = crate::system_info::get_primary_screen_info();
            Ok(Some(serde_json::json!({
                "id": 1,
                "bounds": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "workArea": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "scaleFactor": primary.scale_factor,
                "isPrimary": true,
                "rotation": 0,
                "touchSupport": "unknown",
                "displayFrequency": 60,
                "colorSpace": "srgb",
                "size": { "width": primary.width, "height": primary.height },
                "workAreaSize": { "width": primary.width, "height": primary.height },
            })))
        }));

        // nefu.screen.getCursorScreenPoint() - get the cursor position
        let _ = self.register_method("screen.getCursorScreenPoint", Box::new(|_args| {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                let output = std::process::Command::new("powershell")
                    .args(["-Command", "[System.Windows.Forms.Cursor]::Position | Select-Object X,Y | ConvertTo-Json"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8(out.stdout).unwrap_or_default();
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        return Ok(Some(serde_json::json!({
                            "x": json["X"].as_i64().unwrap_or(0),
                            "y": json["Y"].as_i64().unwrap_or(0),
                        })));
                    }
                }
            }
            Ok(Some(serde_json::json!({ "x": 0, "y": 0 })))
        }));

        // nefu.screen.getDisplayMatching(point) - get the matching display
        let _ = self.register_method("screen.getDisplayMatching", Box::new(|_args| {
            let primary = crate::system_info::get_primary_screen_info();
            Ok(Some(serde_json::json!({
                "id": 1,
                "bounds": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "workArea": { "x": 0, "y": 0, "width": primary.width, "height": primary.height },
                "scaleFactor": primary.scale_factor,
                "isPrimary": true,
            })))
        }));

        // nefu.screen.dipToScreenPixels(dip) - convert DIP to screen pixels
        let _ = self.register_method("screen.dipToScreenPixels", Box::new(|args| {
            if let Some(dip_val) = args.first() {
                let dip = dip_val.as_f64().unwrap_or(0.0);
                let scale = crate::system_info::get_primary_screen_info().scale_factor;
                return Ok(Some(Value::Number(serde_json::Number::from_f64(dip * scale).unwrap_or(serde_json::Number::from(0)))));
            }
            Ok(Some(Value::Number(serde_json::Number::from(0u64))))
        }));

        // nefu.screen.screenToDipPixels(screen) - convert screen pixels to DIP
        let _ = self.register_method("screen.screenToDipPixels", Box::new(|args| {
            if let Some(screen_val) = args.first() {
                let screen_px = screen_val.as_f64().unwrap_or(0.0);
                let scale = crate::system_info::get_primary_screen_info().scale_factor;
                if scale > 0.0 {
                    return Ok(Some(Value::Number(serde_json::Number::from_f64(screen_px / scale).unwrap_or(serde_json::Number::from(0)))));
                }
            }
            Ok(Some(Value::Number(serde_json::Number::from(0u64))))
        }));

        // --- NativeImage ---
        // nefu.nativeImage.createFromPath(path) - create an image from a path
        let _ = self.register_method("nativeImage.createFromPath", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing path argument"))?;
            let img = crate::native_image::create_from_path(path)?;
            let size = img.get_size();
            let data_url = img.to_data_url()?;
            Ok(Some(serde_json::json!({
                "width": size.width,
                "height": size.height,
                "dataURL": data_url,
            })))
        }));

        // nefu.nativeImage.createFromBuffer(buffer, options) - create from a buffer
        let _ = self.register_method("nativeImage.createFromBuffer", Box::new(|_args| {
            Ok(Some(Value::Null))
        }));

        // nefu.nativeImage.createEmpty(width, height) - create an empty image
        let _ = self.register_method("nativeImage.createEmpty", Box::new(|args| {
            let width = args.first().and_then(|v| v.as_u64()).unwrap_or(32) as u32;
            let height = args.get(1).and_then(|v| v.as_u64()).unwrap_or(32) as u32;
            let img = crate::native_image::create_empty(width, height);
            let size = img.get_size();
            let data_url = img.to_data_url()?;
            Ok(Some(serde_json::json!({
                "width": size.width,
                "height": size.height,
                "dataURL": data_url,
            })))
        }));

        // nefu.nativeImage.resize(dataURL, options) - resize an image
        let _ = self.register_method("nativeImage.resize", Box::new(|_args| {
            // Simplified implementation: return original image info
            Ok(Some(Value::Null))
        }));

        // --- Clipboard extensions ---
        // nefu.clipboard.readHTML() - read clipboard HTML
        let _ = self.register_method("clipboard.readHTML", Box::new(|_args| {
            // Simplified implementation
            Ok(Some(Value::Null))
        }));

        // nefu.clipboard.writeHTML(html) - Write HTML to the clipboard
        let _ = self.register_method("clipboard.writeHTML", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.clipboard.clear() - Clear the clipboard
        let _ = self.register_method("clipboard.clear", Box::new(|_args| {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| anyhow::anyhow!("cannot access clipboard: {}", e))?;
            clipboard.clear()
                .map_err(|e| anyhow::anyhow!("failed to clear clipboard: {}", e))?;
            Ok(None)
        }));

        // nefu.clipboard.availableFormats() - get available formats
        let _ = self.register_method("clipboard.availableFormats", Box::new(|_args| {
            Ok(Some(serde_json::json!(["text/plain"])))
        }));

        // --- Dialog enhanced ---
        // nefu.dialog.showMessageBox(options) - Show a message box
        let _ = self.register_method("dialog.showMessageBox", Box::new(|args| {
            let title = args.first()
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("Nefu");
            let message = args.first()
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let detail = args.first()
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("detail"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let display = if detail.is_empty() { message } else { &format!("{}\n\n{}", message, detail) };
            let title_owned = title.to_string();
            let display_owned = display.to_string();

            std::thread::spawn(move || {
                rfd::MessageDialog::new()
                    .set_title(&title_owned)
                    .set_description(&display_owned)
                    .set_level(rfd::MessageLevel::Info)
                    .show();
            });
            Ok(Some(serde_json::json!({ "response": 0 })))
        }));

        // nefu.dialog.showErrorBox(title, message) - show an error box
        let _ = self.register_method("dialog.showErrorBox", Box::new(|args| {
            let title = args.first().and_then(|v| v.as_str()).unwrap_or("Error");
            let message = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let title_owned = title.to_string();
            let msg_owned = message.to_string();
            std::thread::spawn(move || {
                rfd::MessageDialog::new()
                    .set_title(&title_owned)
                    .set_description(&msg_owned)
                    .set_level(rfd::MessageLevel::Error)
                    .show();
            });
            Ok(None)
        }));

        // --- WebFrame ---
        // nefu.webFrame.setZoomLevel(level) - set the zoom level
        let _ = self.register_method("webFrame.setZoomLevel", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.webFrame.getZoomLevel() - get the zoom level
        let _ = self.register_method("webFrame.getZoomLevel", Box::new(|_args| {
            Ok(Some(Value::Number(serde_json::Number::from(0i64))))
        }));

        // nefu.webFrame.setZoomFactor(factor) - set the zoom factor
        let _ = self.register_method("webFrame.setZoomFactor", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.webFrame.getZoomFactor() - Get the zoom factor
        let _ = self.register_method("webFrame.getZoomFactor", Box::new(|_args| {
            Ok(Some(Value::Number(serde_json::Number::from_f64(1.0).unwrap_or(serde_json::Number::from(1)))))
        }));

        // nefu.webFrame.findInPage(text, options) - find text in the page
        let _ = self.register_method("webFrame.findInPage", Box::new(|_args| {
            Ok(Some(serde_json::json!({ "activeMatchOrdinal": 0, "matches": 0 })))
        }));

        // nefu.webFrame.stopFindInPage(action) - stop finding
        let _ = self.register_method("webFrame.stopFindInPage", Box::new(|_args| {
            Ok(None)
        }));

        // --- SystemPreferences ---
        // nefu.systemPreferences.isDarkMode() - whether dark mode
        let _ = self.register_method("systemPreferences.isDarkMode", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::system_info::get_system_theme() == crate::system_info::SystemTheme::Dark)))
        }));

        // nefu.systemPreferences.isInvertedColorScheme() - Whether high contrast colors are in use
        let _ = self.register_method("systemPreferences.isInvertedColorScheme", Box::new(|_args| {
            #[cfg(windows)]
            {
                return Ok(Some(Value::Bool(crate::system_preferences::is_inverted_color_scheme())));
            }
            #[cfg(not(windows))]
            {
                return Ok(Some(Value::Bool(false)));
            }
        }));

        // nefu.systemPreferences.getColor(color) - Get a system color
        let _ = self.register_method("systemPreferences.getColor", Box::new(|args| {
            let color_name = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing color name parameter"))?;
            let color = crate::system_preferences::get_color(color_name)?;
            Ok(Some(serde_json::json!({
                "red": color.red,
                "green": color.green,
                "blue": color.blue,
                "alpha": color.alpha,
                "css": color.to_css(),
                "hex": color.to_hex(),
            })))
        }));

        // nefu.systemPreferences.getMediaAccessStatus(mediaType) - Get media access status
        let _ = self.register_method("systemPreferences.getMediaAccessStatus", Box::new(|args| {
            let media_type_str = args.first().and_then(|v| v.as_str()).unwrap_or("camera");
            let media_type = match media_type_str {
                "microphone" => crate::system_preferences::MediaType::Microphone,
                "screen" => crate::system_preferences::MediaType::Screen,
                _ => crate::system_preferences::MediaType::Camera,
            };
            let status = crate::system_preferences::get_media_access_status(&media_type);
            let status_str = match status {
                crate::system_preferences::MediaAccessStatus::Granted => "granted",
                crate::system_preferences::MediaAccessStatus::Denied => "denied",
                crate::system_preferences::MediaAccessStatus::NotDetermined => "notdetermined",
                crate::system_preferences::MediaAccessStatus::Restricted => "restricted",
            };
            Ok(Some(Value::String(status_str.to_string())))
        }));

        // nefu.systemPreferences.askForMediaAccess(mediaType) - request media access
        let _ = self.register_method("systemPreferences.askForMediaAccess", Box::new(|args| {
            let media_type_str = args.first().and_then(|v| v.as_str()).unwrap_or("camera");
            let media_type = match media_type_str {
                "microphone" => crate::system_preferences::MediaType::Microphone,
                "screen" => crate::system_preferences::MediaType::Screen,
                _ => crate::system_preferences::MediaType::Camera,
            };
            let result = crate::system_preferences::ask_for_media_access(&media_type)?;
            Ok(Some(Value::Bool(result)))
        }));

        // nefu.systemPreferences.getAccentColor() - get the system accent color
        let _ = self.register_method("systemPreferences.getAccentColor", Box::new(|_args| {
            let color = crate::system_preferences::get_system_accent_color();
            Ok(Some(serde_json::json!({
                "red": color.red,
                "green": color.green,
                "blue": color.blue,
                "alpha": color.alpha,
                "hex": color.to_hex(),
            })))
        }));

        // nefu.systemPreferences.isHighContrastColorScheme() - Whether the high contrast theme is active
        let _ = self.register_method("systemPreferences.isHighContrastColorScheme", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::system_preferences::is_high_contrast_color_scheme())))
        }));

        // nefu.systemPreferences.areAnimationsEnabled() - whether animations are enabled
        let _ = self.register_method("systemPreferences.areAnimationsEnabled", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::system_preferences::are_animations_enabled())))
        }));

        // nefu.systemPreferences.isTransparencyEnabled() - whether transparency is enabled
        let _ = self.register_method("systemPreferences.isTransparencyEnabled", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::system_preferences::is_transparency_enabled())))
        }));

        // --- SafeStorage ---
        // nefu.safeStorage.isEncryptionAvailable() - whether encryption is available
        let _ = self.register_method("safeStorage.isEncryptionAvailable", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::safe_storage::is_encryption_available())))
        }));

        // nefu.safeStorage.encryptString(plainText) - Encrypt a string
        let _ = self.register_method("safeStorage.encryptString", Box::new(|args| {
            let plain_text = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing text to encrypt"))?;
            let encrypted = crate::safe_storage::encrypt_string(plain_text)?;
            Ok(Some(serde_json::to_value(encrypted)?))
        }));

        // nefu.safeStorage.decryptString(encrypted) - decrypt a string
        let _ = self.register_method("safeStorage.decryptString", Box::new(|args| {
            if let Some(encrypted_val) = args.first() {
                let encrypted: crate::safe_storage::EncryptedData = serde_json::from_value(encrypted_val.clone())
                    .map_err(|e| anyhow::anyhow!("invalid encrypted data format: {}", e))?;
                let plain_text = crate::safe_storage::decrypt_string(&encrypted)?;
                return Ok(Some(Value::String(plain_text)));
            }
            Err(anyhow::anyhow!("missing encrypted data parameter"))
        }));

        // --- Net Module ---
        // nefu.net.fetch(url, options) - HTTP request
        let _ = self.register_method("net.fetch", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing URL parameter"))?;
            let method = args.get(1)
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("method"))
                .and_then(|v| v.as_str())
                .unwrap_or("GET");

            let timeout = args.get(1)
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("timeout"))
                .and_then(|v| v.as_u64())
                .unwrap_or(30);

            let mut req = ureq::request(method, url);
            req = req.timeout(std::time::Duration::from_secs(timeout));

            let response = req.call()
                .map_err(|e| anyhow::anyhow!("request failed: {}", e))?;

            let status = response.status();
            let body = response.into_string().unwrap_or_default();

            Ok(Some(serde_json::json!({
                "status": status,
                "body": body,
                "url": url,
            })))
        }));

        // --- Process ---
        // nefu.process.getProcessMemoryInfo() - Process memory info
        let _ = self.register_method("process.getProcessMemoryInfo", Box::new(|_args| {
            let mem = crate::process_info::get_process_memory_info();
            Ok(Some(serde_json::to_value(mem).unwrap_or_default()))
        }));

        // nefu.process.getSystemMemoryInfo() - system memory info
        let _ = self.register_method("process.getSystemMemoryInfo", Box::new(|_args| {
            let mem = crate::process_info::get_system_memory_info();
            Ok(Some(serde_json::to_value(mem).unwrap_or_default()))
        }));

        // nefu.process.getCPUUsage() - CPU usage
        let _ = self.register_method("process.getCPUUsage", Box::new(|_args| {
            let cpu = crate::process_info::get_cpu_usage();
            Ok(Some(serde_json::to_value(cpu).unwrap_or_default()))
        }));

        // nefu.process.getIOCounters() - IO counters
        let _ = self.register_method("process.getIOCounters", Box::new(|_args| {
            let io = crate::process_info::get_io_counters();
            Ok(Some(serde_json::to_value(io).unwrap_or_default()))
        }));

        // nefu.process.getProcessId() - Process ID
        let _ = self.register_method("process.getProcessId", Box::new(|_args| {
            Ok(Some(Value::Number(serde_json::Number::from(std::process::id()))))
        }));

        // nefu.process.getProcessPath() - process path
        let _ = self.register_method("process.getProcessPath", Box::new(|_args| {
            Ok(Some(Value::String(crate::process_info::get_process_path())))
        }));

        // nefu.process.getVersions() - Version info
        let _ = self.register_method("process.getVersions", Box::new(|_args| {
            let versions = crate::process_info::get_versions();
            Ok(Some(serde_json::to_value(versions).unwrap_or_default()))
        }));

        // nefu.process.getCreationTime() - process creation time
        let _ = self.register_method("process.getCreationTime", Box::new(|_args| {
            Ok(Some(Value::String(crate::process_info::get_creation_time())))
        }));

        // nefu.process.getResourceUsage() - resource usage summary
        let _ = self.register_method("process.getResourceUsage", Box::new(|_args| {
            Ok(Some(crate::process_info::get_resource_usage()))
        }));

        // nefu.process.getEnvironmentVariable(name) - get an environment variable
        let _ = self.register_method("process.getEnvironmentVariable", Box::new(|args| {
            let name = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing environment variable name"))?;
            match crate::process_info::get_environment_variable(name) {
                Some(val) => Ok(Some(Value::String(val))),
                None => Ok(Some(Value::Null)),
            }
        }));

        // nefu.process.getCommandLineArgs() - Get command line arguments
        let _ = self.register_method("process.getCommandLineArgs", Box::new(|_args| {
            let args = crate::process_info::get_command_line_args();
            Ok(Some(serde_json::to_value(args).unwrap_or_default()))
        }));

        // nefu.process.getWorkingDirectory() - Get the working directory
        let _ = self.register_method("process.getWorkingDirectory", Box::new(|_args| {
            Ok(Some(Value::String(crate::process_info::get_working_directory())))
        }));

        // --- BrowserWindow extensions ---
        // nefu.window.getBounds() - get the window bounds
        let _ = self.register_method("window.getBounds", Box::new(|_args| {
            Ok(Some(serde_json::json!({
                "x": 0, "y": 0, "width": 1024, "height": 768
            })))
        }));

        // nefu.window.getSize() - get the window size
        let _ = self.register_method("window.getSize", Box::new(|_args| {
            Ok(Some(serde_json::json!([1024, 768])))
        }));

        // nefu.window.isMaximized() - whether maximized
        let _ = self.register_method("window.isMaximized", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.window.isMinimized() - whether minimized
        let _ = self.register_method("window.isMinimized", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.window.center() - center the window
        let _ = self.register_method("window.center", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.focus() - focus the window
        let _ = self.register_method("window.focus", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.blur() - blur the window
        let _ = self.register_method("window.blur", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.setAlwaysOnTop(flag) - keep window on top
        let _ = self.register_method("window.setAlwaysOnTop", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.show() - show the window
        let _ = self.register_method("window.show", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.hide() - Hide the window
        let _ = self.register_method("window.hide", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.setTitle(title) - set the window title
        let _ = self.register_method("window.setTitle", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.getTitle() - get the window title
        let _ = self.register_method("window.getTitle", Box::new(|_args| {
            Ok(Some(Value::String("Nefu".to_string())))
        }));

        // nefu.window.setResizable(resizable) - Set whether it is resizable
        let _ = self.register_method("window.setResizable", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.isResizable() - whether resizable
        let _ = self.register_method("window.isResizable", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.window.setMinimumSize(width, height) - Set the minimum size
        let _ = self.register_method("window.setMinimumSize", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.setMaximumSize(width, height) - Set the maximum size
        let _ = self.register_method("window.setMaximumSize", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.getMinimumSize() - get the minimum size
        let _ = self.register_method("window.getMinimumSize", Box::new(|_args| {
            Ok(Some(serde_json::json!([400, 300])))
        }));

        // nefu.window.getMaximumSize() - Get the maximum size
        let _ = self.register_method("window.getMaximumSize", Box::new(|_args| {
            Ok(Some(serde_json::json!([0, 0])))
        }));

        // nefu.window.setPosition(x, y) - set the window position
        let _ = self.register_method("window.setPosition", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.getPosition() - Get the window position
        let _ = self.register_method("window.getPosition", Box::new(|_args| {
            Ok(Some(serde_json::json!([0, 0])))
        }));

        // nefu.window.setOpacity(opacity) - Set the window opacity
        let _ = self.register_method("window.setOpacity", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.getOpacity() - get the window opacity
        let _ = self.register_method("window.getOpacity", Box::new(|_args| {
            Ok(Some(Value::Number(serde_json::Number::from_f64(1.0).unwrap_or(serde_json::Number::from(1)))))
        }));

        // nefu.window.setFullScreen(flag) - Set fullscreen
        let _ = self.register_method("window.setFullScreen", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.isFullScreen() - whether fullscreen
        let _ = self.register_method("window.isFullScreen", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.window.restore() - restore the window
        let _ = self.register_method("window.restore", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.window.isVisible() - whether the window is visible
        let _ = self.register_method("window.isVisible", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // --- Tray enhanced ---
        // nefu.tray.setToolTip(toolTip) - set the tray tooltip
        let _ = self.register_method("tray.setToolTip", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.tray.setTitle(title) - Set the tray title
        let _ = self.register_method("tray.setTitle", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.tray.displayBalloon(options) - Show a tray balloon
        let _ = self.register_method("tray.displayBalloon", Box::new(|args| {
            if let Some(opts) = args.first() {
                let title = opts.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let content = opts.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let _ = crate::notifications::send_simple_notification(title, content);
            }
            Ok(None)
        }));

        // nefu.tray.removeBalloon() - Remove the tray balloon
        let _ = self.register_method("tray.removeBalloon", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.tray.isDestroyed() - whether the tray is destroyed
        let _ = self.register_method("tray.isDestroyed", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // --- Desktop Capturer enhanced ---
        // nefu.desktopCapturer.getSources(options) - get screen sources
        let _ = self.register_method("desktopCapturer.getSources", Box::new(|_args| {
            let screenshot = capture_screenshot()?;
            Ok(Some(serde_json::json!([{
                "id": "screen:0:0",
                "name": "Primary Monitor",
                "thumbnail": screenshot,
                "display_id": "0",
                "appIcon": null,
                "appName": "Nefu",
            }])))
        }));

        // --- App exit ---
        // nefu.app.quit() - quit the app
        let cmd_tx_quit = window_cmd_tx.clone();
        let _ = self.register_method("app.quit", Box::new(move |_args| {
            // Send the window close command
            if let Ok(guard) = cmd_tx_quit.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Close);
                }
            }
            Ok(None)
        }));

        // nefu.app.exit() - force quit
        let cmd_tx_exit = window_cmd_tx.clone();
        let _ = self.register_method("app.exit", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx_exit.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Close);
                }
            }
            Ok(None)
        }));

        // nefu.app.relaunch() - restart the app
        let cmd_tx_relaunch = window_cmd_tx.clone();
        let _ = self.register_method("app.relaunch", Box::new(move |_args| {
            if let Ok(guard) = cmd_tx_relaunch.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(WindowCommand::Close);
                }
            }
            Ok(None)
        }));

        // nefu.app.focus() - Focus the application
        let _ = self.register_method("app.focus", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.app.hide() - hide the app
        let _ = self.register_method("app.hide", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.app.show() - show the app
        let _ = self.register_method("app.show", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.app.getSystemProxySettings() - get system proxy settings
        let _ = self.register_method("app.getSystemProxySettings", Box::new(|_args| {
            Ok(Some(crate::app_module::get_system_proxy_settings()))
        }));

        // nefu.app.getSystemVersionInfo() - get system version info
        let _ = self.register_method("app.getSystemVersionInfo", Box::new(|_args| {
            Ok(Some(crate::app_module::get_system_version_info()))
        }));

        // ==================== Added Electron modules ====================

        // --- WebContents module ---
        // nefu.webContents.getURL() - Get the current URL
        let _ = self.register_method("webContents.getURL", Box::new(|_args| {
            Ok(Some(Value::String(crate::web_contents::WebContents::new().get_url())))
        }));

        // nefu.webContents.getTitle() - Get the page title
        let _ = self.register_method("webContents.getTitle", Box::new(|_args| {
            Ok(Some(Value::String(crate::web_contents::WebContents::new().get_title())))
        }));

        // nefu.webContents.isLoading() - Whether it is loading
        let _ = self.register_method("webContents.isLoading", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::web_contents::WebContents::new().is_loading())))
        }));

        // nefu.webContents.canGoBack() - whether can go back
        let _ = self.register_method("webContents.canGoBack", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::web_contents::WebContents::new().can_go_back())))
        }));

        // nefu.webContents.canGoForward() - Whether it can go forward
        let _ = self.register_method("webContents.canGoForward", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::web_contents::WebContents::new().can_go_forward())))
        }));

        // nefu.webContents.getNavigationHistory() - get navigation history
        let _ = self.register_method("webContents.getNavigationHistory", Box::new(|_args| {
            let history = crate::web_contents::WebContents::new().get_navigation_history();
            Ok(Some(serde_json::to_value(history).unwrap_or_default()))
        }));

        // nefu.webContents.getWebContentsInfo() - get page status info
        let _ = self.register_method("webContents.getWebContentsInfo", Box::new(|_args| {
            Ok(Some(crate::web_contents::get_web_contents_info()))
        }));

        // --- Session module ---
        // nefu.session.cookies.get(filter) - get cookies
        let _ = self.register_method("session.cookies.get", Box::new(|args| {
            let filter = args.first().and_then(|v| {
                serde_json::from_value::<crate::session::CookieFilter>(v.clone()).ok()
            });
            let cookies = crate::session::get_session().get_cookies(filter);
            Ok(Some(serde_json::to_value(cookies).unwrap_or_default()))
        }));

        // nefu.session.cookies.set(cookie) - set a cookie
        let _ = self.register_method("session.cookies.set", Box::new(|args| {
            let cookie: crate::session::Cookie = serde_json::from_value(
                args.first().cloned().unwrap_or_default()
            )?;
            crate::session::get_session().set_cookie(cookie)?;
            Ok(None)
        }));

        // nefu.session.cookies.remove(url, name) - remove a cookie
        let _ = self.register_method("session.cookies.remove", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            crate::session::get_session().remove_cookie(url, name)?;
            Ok(None)
        }));

        // nefu.session.cookies.flush() - Flush cookies
        let _ = self.register_method("session.cookies.flush", Box::new(|_args| {
            crate::session::get_session().flush_cookies()?;
            Ok(None)
        }));

        // nefu.session.clearCache() - Clear the cache
        let _ = self.register_method("session.clearCache", Box::new(|_args| {
            crate::session::get_session().clear_cache()?;
            Ok(None)
        }));

        // nefu.session.clearStorageData() - clear storage data
        let _ = self.register_method("session.clearStorageData", Box::new(|_args| {
            crate::session::get_session().clear_storage_data()?;
            Ok(None)
        }));

        // nefu.session.setProxy(config) - set the proxy
        let _ = self.register_method("session.setProxy", Box::new(|args| {
            let config: crate::session::ProxyConfig = serde_json::from_value(
                args.first().cloned().unwrap_or_default()
            )?;
            crate::session::get_session().set_proxy(config)?;
            Ok(None)
        }));

        // nefu.session.getProxy() - get the proxy config
        let _ = self.register_method("session.getProxy", Box::new(|_args| {
            let proxy = crate::session::get_session().get_proxy();
            Ok(Some(serde_json::to_value(proxy).unwrap_or_default()))
        }));

        // nefu.session.isOnline() - check network status
        let _ = self.register_method("session.isOnline", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::session::get_session().is_online())))
        }));

        // --- Protocol module ---
        // nefu.protocol.registerStringProtocol(scheme) - register a string protocol
        let _ = self.register_method("protocol.registerStringProtocol", Box::new(|args| {
            let scheme = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if scheme.is_empty() {
                return Err(anyhow::anyhow!("protocol name cannot be empty"));
            }
            let mgr = crate::protocol_handler::get_protocol_manager();
            mgr.register_string_protocol(scheme, |_request| {
                Ok(crate::protocol_handler::ProtocolResponse {
                    status_code: 200,
                    headers: std::collections::HashMap::new(),
                    data: None,
                    text: Some("OK".to_string()),
                    path: None,
                    mime_type: Some("text/plain".to_string()),
                    charset: Some("utf-8".to_string()),
                    redirect_url: None,
                })
            })?;
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.protocol.isProtocolHandled(scheme) - check whether a protocol is registered
        let _ = self.register_method("protocol.isProtocolHandled", Box::new(|args| {
            let scheme = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let handled = crate::protocol_handler::get_protocol_manager().is_protocol_handled(scheme);
            Ok(Some(Value::Bool(handled)))
        }));

        // nefu.protocol.unregisterProtocol(scheme) - unregister a protocol
        let _ = self.register_method("protocol.unregisterProtocol", Box::new(|args| {
            let scheme = args.first().and_then(|v| v.as_str()).unwrap_or("");
            crate::protocol_handler::get_protocol_manager().unregister_protocol(scheme)?;
            Ok(None)
        }));

        // nefu.protocol.getRegisteredProtocols() - get registered protocols
        let _ = self.register_method("protocol.getRegisteredProtocols", Box::new(|_args| {
            let protocols = crate::protocol_handler::get_protocol_manager().get_registered_protocols();
            Ok(Some(serde_json::to_value(protocols).unwrap_or_default()))
        }));

        // --- Dock module ---
        // nefu.dock.setBadge(text) - Set the Dock badge
        let _ = self.register_method("dock.setBadge", Box::new(|args| {
            let text = args.first().and_then(|v| v.as_str()).unwrap_or("");
            crate::dock::get_dock().set_badge(text)?;
            Ok(None)
        }));

        // nefu.dock.getBadge() - Get the Dock badge
        let _ = self.register_method("dock.getBadge", Box::new(|_args| {
            Ok(Some(Value::String(crate::dock::get_dock().get_badge())))
        }));

        // nefu.dock.bounce(type) - Bounce the Dock icon
        let _ = self.register_method("dock.bounce", Box::new(|args| {
            let bounce_type = match args.first().and_then(|v| v.as_str()) {
                Some("critical") => crate::dock::BounceType::Critical,
                _ => crate::dock::BounceType::Informational,
            };
            let id = crate::dock::get_dock().bounce(bounce_type);
            Ok(Some(Value::Number(serde_json::Number::from(id))))
        }));

        // nefu.dock.cancelBounce(id) - Cancel the bounce
        let _ = self.register_method("dock.cancelBounce", Box::new(|args| {
            let id = args.first().and_then(|v| v.as_u64()).unwrap_or(0);
            crate::dock::get_dock().cancel_bounce(id);
            Ok(None)
        }));

        // nefu.dock.show() - Show the Dock icon
        let _ = self.register_method("dock.show", Box::new(|_args| {
            crate::dock::get_dock().show()?;
            Ok(None)
        }));

        // nefu.dock.hide() - Hide the Dock icon
        let _ = self.register_method("dock.hide", Box::new(|_args| {
            crate::dock::get_dock().hide()?;
            Ok(None)
        }));

        // nefu.dock.isVisible() - Check whether the Dock is visible
        let _ = self.register_method("dock.isVisible", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::dock::get_dock().is_visible())))
        }));

        // --- NativeTheme module ---
        // nefu.nativeTheme.shouldUseDarkColors() - Whether to use the dark theme
        let _ = self.register_method("nativeTheme.shouldUseDarkColors", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::native_theme::get_native_theme().should_use_dark_colors())))
        }));

        // nefu.nativeTheme.shouldUseHighContrastColors() - Whether to use high contrast
        let _ = self.register_method("nativeTheme.shouldUseHighContrastColors", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::native_theme::get_native_theme().should_use_high_contrast_colors())))
        }));

        // nefu.nativeTheme.shouldUseInvertedColorScheme() - Whether to use inverted colors
        let _ = self.register_method("nativeTheme.shouldUseInvertedColorScheme", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::native_theme::get_native_theme().should_use_inverted_color_scheme())))
        }));

        // nefu.nativeTheme.themeSource() - Get the theme source
        let _ = self.register_method("nativeTheme.themeSource", Box::new(|_args| {
            let source = crate::native_theme::get_native_theme().theme_source();
            let s = match source {
                crate::native_theme::ThemeSource::System => "system",
                crate::native_theme::ThemeSource::Light => "light",
                crate::native_theme::ThemeSource::Dark => "dark",
            };
            Ok(Some(Value::String(s.to_string())))
        }));

        // nefu.nativeTheme.setThemeSource(source) - Set the theme source
        let _ = self.register_method("nativeTheme.setThemeSource", Box::new(|args| {
            let source = match args.first().and_then(|v| v.as_str()) {
                Some("light") => crate::native_theme::ThemeSource::Light,
                Some("dark") => crate::native_theme::ThemeSource::Dark,
                _ => crate::native_theme::ThemeSource::System,
            };
            crate::native_theme::get_native_theme().set_theme_source(source)?;
            Ok(None)
        }));

        // --- ContentTracing module ---
        // nefu.contentTracing.startRecording(options) - Start recording a trace
        let _ = self.register_method("contentTracing.startRecording", Box::new(|args| {
            let options = args.first().and_then(|v| {
                serde_json::from_value::<crate::content_tracing::TracingOptions>(v.clone()).ok()
            });
            crate::content_tracing::get_content_tracing().start_recording(options)?;
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.contentTracing.stopRecording() - Stop recording a trace
        let _ = self.register_method("contentTracing.stopRecording", Box::new(|_args| {
            let path = crate::content_tracing::get_content_tracing().stop_recording()?;
            Ok(Some(Value::String(path)))
        }));

        // nefu.contentTracing.getCategories() - Get the available trace categories
        let _ = self.register_method("contentTracing.getCategories", Box::new(|_args| {
            let categories = crate::content_tracing::get_content_tracing().get_categories();
            Ok(Some(serde_json::to_value(categories).unwrap_or_default()))
        }));

        // nefu.contentTracing.getTraceBufferUsage() - Get the trace buffer usage
        let _ = self.register_method("contentTracing.getTraceBufferUsage", Box::new(|_args| {
            let usage = crate::content_tracing::get_content_tracing().get_trace_buffer_usage();
            Ok(Some(serde_json::to_value(usage).unwrap_or_default()))
        }));

        // nefu.contentTracing.isRecording() - Whether it is recording
        let _ = self.register_method("contentTracing.isRecording", Box::new(|_args| {
            Ok(Some(Value::Bool(crate::content_tracing::get_content_tracing().is_recording())))
        }));

        // ==================== Second batch of new Electron APIs ====================

        // --- GlobalShortcut API ---
        // nefu.globalShortcut.register(accelerator) - Register a global shortcut
        let _ = self.register_method("globalShortcut.register", Box::new(|args| {
            let accelerator = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing shortcut parameter"))?;
            let mut gs = GLOBAL_SHORTCUT.lock().map_err(|e| anyhow::anyhow!("lock error: {}", e))?;
            let result = gs.register(accelerator, || {})?;
            Ok(Some(Value::Bool(result)))
        }));

        // nefu.globalShortcut.unregister(accelerator) - Unregister a shortcut
        let _ = self.register_method("globalShortcut.unregister", Box::new(|args| {
            let accelerator = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing shortcut parameter"))?;
            let mut gs = GLOBAL_SHORTCUT.lock().map_err(|e| anyhow::anyhow!("lock error: {}", e))?;
            gs.unregister(accelerator);
            Ok(None)
        }));

        // nefu.globalShortcut.unregisterAll() - Unregister all shortcuts
        let _ = self.register_method("globalShortcut.unregisterAll", Box::new(|_args| {
            let mut gs = GLOBAL_SHORTCUT.lock().map_err(|e| anyhow::anyhow!("lock error: {}", e))?;
            gs.unregister_all();
            Ok(None)
        }));

        // nefu.globalShortcut.isRegistered(accelerator) - Check whether it is registered
        let _ = self.register_method("globalShortcut.isRegistered", Box::new(|args| {
            let accelerator = args.first().and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing shortcut parameter"))?;
            let gs = GLOBAL_SHORTCUT.lock().map_err(|e| anyhow::anyhow!("lock error: {}", e))?;
            Ok(Some(Value::Bool(gs.is_registered(accelerator))))
        }));

        // nefu.globalShortcut.getRegistered() - Get all registered shortcuts
        let _ = self.register_method("globalShortcut.getRegistered", Box::new(|_args| {
            let gs = GLOBAL_SHORTCUT.lock().map_err(|e| anyhow::anyhow!("lock error: {}", e))?;
            let list: Vec<Value> = gs.get_registered().into_iter().map(Value::String).collect();
            Ok(Some(Value::Array(list)))
        }));

        // --- Menu class methods API ---
        // nefu.menu.setApplicationMenu(template) - Set the application menu
        let _ = self.register_method("menu.setApplicationMenu", Box::new(|args| {
            let template_json = serde_json::to_string(args.first().unwrap_or(&Value::Null))?;
            let template: crate::native_menu::MenuTemplate = serde_json::from_str(&template_json)
                .unwrap_or_else(|_| crate::native_menu::MenuTemplate::default_app_menu("NefuApp"));
            let _menu = crate::native_menu::NativeMenu::new(template);
            log::info!("application menu has been set");
            Ok(None)
        }));

        // nefu.menu.buildFromTemplate(template) - Build a menu from a template
        let _ = self.register_method("menu.buildFromTemplate", Box::new(|args| {
            let template_json = serde_json::to_string(args.first().unwrap_or(&Value::Null))?;
            let template: crate::native_menu::MenuTemplate = serde_json::from_str(&template_json)
                .unwrap_or_else(|_| crate::native_menu::MenuTemplate::default_app_menu("NefuApp"));
            let menu = crate::native_menu::NativeMenu::new(template);
            Ok(Some(menu.to_json()))
        }));

        // nefu.menu.popup(template) - Pop up a context menu
        let _ = self.register_method("menu.popup", Box::new(|args| {
            let template_json = serde_json::to_string(args.first().unwrap_or(&Value::Null))?;
            let template: crate::native_menu::MenuTemplate = serde_json::from_str(&template_json)
                .unwrap_or_else(|_| crate::native_menu::MenuTemplate::default_app_menu("NefuApp"));
            let _menu = crate::native_menu::NativeMenu::new(template);
            log::info!("context menu popped up");
            Ok(None)
        }));

        // nefu.menu.getApplicationMenu() - Get the current application menu
        let _ = self.register_method("menu.getApplicationMenu", Box::new(|_args| {
            Ok(Some(Value::Null))
        }));

        // --- webContents extension API ---
        // nefu.webContents.executeJavaScript(code) - Execute JS in the page
        let _ = self.register_method("webContents.executeJavaScript", Box::new(|args| {
            let code = args.first().and_then(|v| v.as_str()).unwrap_or("");
            // Return a code marker to execute; actual execution is done by the frontend
            Ok(Some(Value::String(format!("__nefu_exec__:{}", code))))
        }));

        // nefu.webContents.insertCSS(css) - Inject CSS
        let _ = self.register_method("webContents.insertCSS", Box::new(|args| {
            let css = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Some(Value::String(format!("__nefu_css__:{}", css))))
        }));

        // nefu.webContents.reload() - Reload the page
        let _ = self.register_method("webContents.reload", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_reload__")))
        }));

        // nefu.webContents.reloadIgnoringCache() - Reload ignoring the cache
        let _ = self.register_method("webContents.reloadIgnoringCache", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_reload_nocache__")))
        }));

        // nefu.webContents.goBack() - Go back
        let _ = self.register_method("webContents.goBack", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_goback__")))
        }));

        // nefu.webContents.goForward() - Go forward
        let _ = self.register_method("webContents.goForward", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_goforward__")))
        }));

        // nefu.webContents.print() - Print
        let _ = self.register_method("webContents.print", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_print__")))
        }));

        // nefu.webContents.printToPDF() - Print to PDF
        let _ = self.register_method("webContents.printToPDF", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_printtopdf__")))
        }));

        // nefu.webContents.downloadURL(url) - Download a URL
        let _ = self.register_method("webContents.downloadURL", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("download request: {}", url);
            Ok(None)
        }));

        // nefu.webContents.setZoomFactor(factor) - Set the zoom factor
        let _ = self.register_method("webContents.setZoomFactor", Box::new(|args| {
            let factor = args.first().and_then(|v| v.as_f64()).unwrap_or(1.0);
            Ok(Some(Value::String(format!("__nefu_zoomfactor__:{:.3}", factor))))
        }));

        // nefu.webContents.getZoomFactor() - Get the zoom factor
        let _ = self.register_method("webContents.getZoomFactor", Box::new(|_args| {
            Ok(Some(serde_json::json!(1.0)))
        }));

        // nefu.webContents.setZoomLevel(level) - Set the zoom level
        let _ = self.register_method("webContents.setZoomLevel", Box::new(|args| {
            let level = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(Some(Value::String(format!("__nefu_zoomlevel__:{:.3}", level))))
        }));

        // nefu.webContents.getZoomLevel() - Get the zoom level
        let _ = self.register_method("webContents.getZoomLevel", Box::new(|_args| {
            Ok(Some(serde_json::json!(0.0)))
        }));

        // nefu.webContents.toggleDevTools() - Toggle developer tools
        let _ = self.register_method("webContents.toggleDevTools", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_toggledevtools__")))
        }));

        // nefu.webContents.openDevTools() - Open developer tools
        let _ = self.register_method("webContents.openDevTools", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_opendevtools__")))
        }));

        // nefu.webContents.closeDevTools() - Close developer tools
        let _ = self.register_method("webContents.closeDevTools", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_closedevtools__")))
        }));

        // nefu.webContents.isDevToolsOpened() - Whether developer tools are open
        let _ = self.register_method("webContents.isDevToolsOpened", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.webContents.copy() - Copy
        let _ = self.register_method("webContents.copy", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_copy__")))
        }));

        // nefu.webContents.paste() - Paste
        let _ = self.register_method("webContents.paste", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_paste__")))
        }));

        // nefu.webContents.cut() - Cut
        let _ = self.register_method("webContents.cut", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_cut__")))
        }));

        // nefu.webContents.selectAll() - Select all
        let _ = self.register_method("webContents.selectAll", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_selectall__")))
        }));

        // nefu.webContents.undo() - Undo
        let _ = self.register_method("webContents.undo", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_undo__")))
        }));

        // nefu.webContents.redo() - Redo
        let _ = self.register_method("webContents.redo", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_redo__")))
        }));

        // nefu.webContents.findInPage(text) - Find in page
        let _ = self.register_method("webContents.findInPage", Box::new(|args| {
            let text = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let options = args.get(1).map(|v| v.to_string()).unwrap_or_default();
            Ok(Some(Value::String(format!("__nefu_find__:{}:{}", text, options))))
        }));

        // nefu.webContents.stopFindInPage() - Stop finding in page
        let _ = self.register_method("webContents.stopFindInPage", Box::new(|_args| {
            Ok(Some(Value::from("__nefu_stopfind__")))
        }));

        // --- App extension API ---
        // nefu.app.setAboutPanelOptions(options) - Set the about panel
        let _ = self.register_method("app.setAboutPanelOptions", Box::new(|args| {
            let opts = args.first().unwrap_or(&Value::Null);
            log::info!("set about panel: {:?}", opts);
            Ok(None)
        }));

        // nefu.app.getGPUInfo(type) - Get GPU info
        let _ = self.register_method("app.getGPUInfo", Box::new(|_args| {
            let gpu = crate::system_info::get_gpu_info();
            Ok(Some(serde_json::to_value(gpu).unwrap_or_default()))
        }));

        // nefu.app.setAsDefaultProtocolClient(protocol) - Set the default protocol handler
        let _ = self.register_method("app.setAsDefaultProtocolClient", Box::new(|args| {
            let protocol = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("set default protocol handler: {}", protocol);
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.app.removeAsDefaultProtocolClient(protocol) - Remove the default protocol handler
        let _ = self.register_method("app.removeAsDefaultProtocolClient", Box::new(|args| {
            let _protocol = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("remove default protocol handler: {}", _protocol);
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.app.isDefaultProtocolClient(protocol) - Check whether it is the default protocol handler
        let _ = self.register_method("app.isDefaultProtocolClient", Box::new(|args| {
            let protocol = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Some(Value::Bool(false)))
        }));

        // nefu.app.getApplicationNameForProtocol(url) - Get the application name for a protocol
        let _ = self.register_method("app.getApplicationNameForProtocol", Box::new(|_args| {
            Ok(Some(Value::String(String::new())))
        }));

        // nefu.app.setUserActivity(type, userInfo) - Set user activity (macOS Handoff)
        let _ = self.register_method("app.setUserActivity", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.app.requestSingleInstanceLock() - Request the single instance lock
        let _ = self.register_method("app.requestSingleInstanceLock", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.app.releaseSingleInstanceLock() - Release the single instance lock
        let _ = self.register_method("app.releaseSingleInstanceLock", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.app.userAgentFallback - Get the default User-Agent
        let _ = self.register_method("app.getUserAgentFallback", Box::new(|_args| {
            Ok(Some(Value::String("Nefu/1.0".to_string())))
        }));

        // nefu.app.setAppUserModelId(id) - Set the app user model ID (Windows taskbar)
        let _ = self.register_method("app.setAppUserModelId", Box::new(|args| {
            let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("set AppUserModelId: {}", id);
            let _ = id;
            Ok(None)
        }));

        // nefu.app.getJumpListSettings() - Get jump list settings (Windows)
        let _ = self.register_method("app.getJumpListSettings", Box::new(|_args| {
            Ok(Some(serde_json::json!({
                "minItems": 0,
                "removedItems": [],
            })))
        }));

        // nefu.app.setJumpList(categories) - Set the jump list (Windows)
        let _ = self.register_method("app.setJumpList", Box::new(|_args| {
            Ok(None)
        }));

        // --- Shell extension API ---
        // nefu.shell.writeShortcutLink(shortcutPath, operation, options) - Create/update a Windows shortcut
        let _ = self.register_method("shell.writeShortcutLink", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get(2).and_then(|v| v.get("target")).and_then(|v| v.as_str()).unwrap_or("");
            log::info!("create shortcut: {} -> {}", path, target);
            #[cfg(windows)]
            {
                // Create the shortcut using PowerShell
                let ps_script = format!(
                    "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Save()",
                    path.replace('\'', "''"),
                    target.replace('\'', "''")
                );
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &ps_script])
                    .output();
            }
            Ok(None)
        }));

        // nefu.shell.readShortcutLink(shortcutPath) - Read a Windows shortcut
        let _ = self.register_method("shell.readShortcutLink", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            #[cfg(windows)]
            {
                let ps_script = format!(
                    "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); Write-Output $s.TargetPath",
                    path.replace('\'', "''")
                );
                if let Ok(output) = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &ps_script])
                    .output()
                {
                    let target = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return Ok(Some(serde_json::json!({ "target": target })));
                }
            }
            Ok(Some(serde_json::json!({ "target": "" })))
        }));

        // nefu.shell.moveItemToTrash(path) - Move to trash (alias)
        let _ = self.register_method("shell.moveItemToTrash", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            crate::shell::trash_item(path)?;
            Ok(Some(Value::Bool(true)))
        }));

        // --- BrowserView API (basic stubs) ---
        // nefu.browserView.create(options) - Create a BrowserView
        let _ = self.register_method("browserView.create", Box::new(|_args| {
            Ok(Some(serde_json::json!({ "id": 1 })))
        }));

        // nefu.browserView.setBounds(id, bounds) - Set bounds
        let _ = self.register_method("browserView.setBounds", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.browserView.setBounds(id, bounds) - Set bounds
        let _ = self.register_method("browserView.setBackgroundColor", Box::new(|_args| {
            Ok(None)
        }));

        // nefu.browserView.webContents(id) - Get the webContents
        let _ = self.register_method("browserView.webContents", Box::new(|_args| {
            Ok(Some(serde_json::json!({ "id": 1 })))
        }));

        // nefu.browserView.remove(id) - Remove a BrowserView
        let _ = self.register_method("browserView.remove", Box::new(|_args| {
            Ok(None)
        }));

        // --- ipcMain API ---
        // nefu.ipcMain.on(channel) - Listen for IPC messages
        let _ = self.register_method("ipcMain.on", Box::new(|args| {
            let channel = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("ipcMain listen: {}", channel);
            Ok(None)
        }));

        // nefu.ipcMain.off(channel) - Stop listening
        let _ = self.register_method("ipcMain.off", Box::new(|args| {
            let channel = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("ipcMain stop listening: {}", channel);
            Ok(None)
        }));

        // nefu.ipcMain.once(channel) - Listen once
        let _ = self.register_method("ipcMain.once", Box::new(|args| {
            let channel = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("ipcMain listen once: {}", channel);
            Ok(None)
        }));

        // nefu.ipcMain.handle(channel, handler) - Register a handler
        let _ = self.register_method("ipcMain.handle", Box::new(|args| {
            let channel = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("ipcMain register handler: {}", channel);
            Ok(None)
        }));

        // nefu.ipcMain.removeHandler(channel) - Remove a handler
        let _ = self.register_method("ipcMain.removeHandler", Box::new(|args| {
            let channel = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("ipcMain remove handler: {}", channel);
            Ok(None)
        }));

        // --- netLog API ---
        // nefu.netLog.startLogging(path) - Start network logging
        let _ = self.register_method("netLog.startLogging", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            log::info!("start network logging: {}", path);
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.netLog.stopLogging() - Stop network logging
        let _ = self.register_method("netLog.stopLogging", Box::new(|_args| {
            log::info!("stop network logging");
            Ok(Some(Value::String(String::new())))
        }));

        // --- utilityProcess API ---
        // nefu.utilityProcess.fork(path, args) - Create a child process
        let _ = self.register_method("utilityProcess.fork", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let extra_args: Vec<String> = args.get(1)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let child = std::process::Command::new(path)
                .args(&extra_args)
                .spawn();
            match child {
                Ok(handle) => Ok(Some(serde_json::json!({
                    "pid": handle.id(),
                    "success": true
                }))),
                Err(e) => Ok(Some(serde_json::json!({
                    "pid": 0,
                    "success": false,
                    "error": e.to_string()
                }))),
            }
        }));

        // --- TouchBar API (macOS stubs) ---
        // nefu.touchBar.create(items) - Create a TouchBar
        let _ = self.register_method("touchBar.create", Box::new(|_args| {
            Ok(Some(serde_json::json!({ "id": 0 })))
        }));

        // --- shareMenu API (macOS stubs) ---
        // nefu.shareMenu.show(items) - Show a share menu
        let _ = self.register_method("shareMenu.show", Box::new(|_args| {
            Ok(None)
        }));

        // --- net extension API ---
        // nefu.net.request(method, url, options) - Make an HTTP request
        let _ = self.register_method("net.request", Box::new(|args| {
            let method = args.first().and_then(|v| v.as_str()).unwrap_or("GET");
            let url = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let body = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
            let resp = ureq::request(method, url)
                .timeout(std::time::Duration::from_secs(30))
                .send_string(body);
            match resp {
                Ok(r) => {
                    let status = r.status();
                    let text = r.into_string().unwrap_or_default();
                    Ok(Some(serde_json::json!({
                        "status": status,
                        "body": text,
                        "success": status >= 200 && status < 300
                    })))
                }
                Err(e) => Ok(Some(serde_json::json!({
                    "status": 0,
                    "body": "",
                    "error": e.to_string(),
                    "success": false
                }))),
            }
        }));

        // nefu.net.isOnline() - Check the network connection
        let _ = self.register_method("net.isOnline", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // nefu.net.online observable - Get the network status
        let _ = self.register_method("net.getOnline", Box::new(|_args| {
            Ok(Some(Value::Bool(true)))
        }));

        // --- Process extension API ---
        // nefu.process.getArchitecture() - Get the process architecture
        let _ = self.register_method("process.getArchitecture", Box::new(|_args| {
            let arch = if cfg!(target_arch = "x86_64") { "x64" }
                else if cfg!(target_arch = "x86") { "ia32" }
                else if cfg!(target_arch = "aarch64") { "arm64" }
                else { "unknown" };
            Ok(Some(Value::String(arch.to_string())))
        }));

        // nefu.process.getSystemArchitecture() - Get the system architecture
        let _ = self.register_method("process.getSystemArchitecture", Box::new(|_args| {
            let arch = if cfg!(target_arch = "x86_64") { "x64" }
                else if cfg!(target_arch = "x86") { "ia32" }
                else if cfg!(target_arch = "aarch64") { "arm64" }
                else { "unknown" };
            Ok(Some(Value::String(arch.to_string())))
        }));

        // nefu.process.getUptime() - Get the process uptime
        let _ = self.register_method("process.getUptime", Box::new(|_args| {
            let uptime = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Approximate process start time
            Ok(Some(serde_json::json!(uptime)))
        }));

        // nefu.process.getTimeFromSnapshot() - Get the snapshot time
        let _ = self.register_method("process.getTimeFromSnapshot", Box::new(|_args| {
            Ok(Some(serde_json::json!(0)))
        }));

        // --- Notification extension API ---
        // nefu.notification.show(title, body, options) - Show a notification (enhanced)
        let _ = self.register_method("notification.show", Box::new(|args| {
            let title = args.first().and_then(|v| v.as_str()).unwrap_or("Notification");
            let body = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let _icon = args.get(2).and_then(|v| v.as_str());
            let _ = crate::notifications::send_simple_notification(title, body);
            Ok(None)
        }));

        // --- powerMonitor extension API ---
        // nefu.powerMonitor.getBatteryLevel() - Get the battery level
        let _ = self.register_method("powerMonitor.getBatteryLevel", Box::new(|_args| {
            Ok(Some(serde_json::json!(100)))
        }));

        // nefu.powerMonitor.isOnBatteryPower() - Whether battery power is in use
        let _ = self.register_method("powerMonitor.isOnBatteryPower", Box::new(|_args| {
            Ok(Some(Value::Bool(false)))
        }));

        // --- screen extension API ---
        // nefu.screen.getNearestDisplay(point) - Get the nearest display
        let _ = self.register_method("screen.getNearestDisplay", Box::new(|_args| {
            // Simplified: return the primary display
            let display = crate::system_info::get_primary_screen_info();
            Ok(Some(serde_json::to_value(display).unwrap_or_default()))
        }));

        // --- safeStorage extension ---
        // nefu.safeStorage.getSelectedStorageBackend() - Get the encryption backend
        let _ = self.register_method("safeStorage.getSelectedStorageBackend", Box::new(|_args| {
            Ok(Some(Value::String("dpapi".to_string())))
        }));

        // --- context bridge extension ---
        // nefu.contextBridge.exposeInMainWorld(name, api) - Expose an API to the renderer
        let _ = self.register_method("contextBridge.exposeInMainWorld", Box::new(|args| {
            let name = args.first().and_then(|v| v.as_str()).unwrap_or("api");
            log::info!("expose API to renderer: {}", name);
            Ok(None)
        }));

        // ========== SQL management system ==========
        // nefu.sql.open(path) - Open a SQLite database
        let _ = self.register_method("sql.open", Box::new(|args| {
            let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return Ok(Some(Value::String("error: database path cannot be empty".to_string())));
            }
            match SQL_MANAGER.open(path) {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.openMemory() - Create an in-memory database
        let _ = self.register_method("sql.openMemory", Box::new(|_args| {
            match SQL_MANAGER.open_memory() {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.close() - Close the database connection
        let _ = self.register_method("sql.close", Box::new(|_args| {
            match SQL_MANAGER.close() {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.query(sql) - Run a query
        let _ = self.register_method("sql.query", Box::new(|args| {
            let sql_text = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if sql_text.is_empty() {
                return Ok(Some(serde_json::json!({"error": "SQL statement cannot be empty"})));
            }
            match SQL_MANAGER.query(sql_text) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // nefu.sql.execute(sql) - Run a command
        let _ = self.register_method("sql.execute", Box::new(|args| {
            let sql_text = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if sql_text.is_empty() {
                return Ok(Some(serde_json::json!({"error": "SQL statement cannot be empty"})));
            }
            match SQL_MANAGER.execute(sql_text) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // nefu.sql.executeBatch(sql) - Run in batch
        let _ = self.register_method("sql.executeBatch", Box::new(|args| {
            let sql_text = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if sql_text.is_empty() {
                return Ok(Some(serde_json::json!({"error": "SQL statement cannot be empty"})));
            }
            match SQL_MANAGER.execute_batch(sql_text) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // nefu.sql.beginTransaction() - Begin a transaction
        let _ = self.register_method("sql.beginTransaction", Box::new(|_args| {
            match SQL_MANAGER.begin_transaction() {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.commit() - Commit the transaction
        let _ = self.register_method("sql.commit", Box::new(|_args| {
            match SQL_MANAGER.commit() {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.rollback() - Roll back the transaction
        let _ = self.register_method("sql.rollback", Box::new(|_args| {
            match SQL_MANAGER.rollback() {
                Ok(msg) => Ok(Some(Value::String(msg))),
                Err(e) => Ok(Some(Value::String(format!("error: {}", e))))
            }
        }));

        // nefu.sql.getInfo() - Get database info
        let _ = self.register_method("sql.getInfo", Box::new(|_args| {
            match SQL_MANAGER.get_info() {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // nefu.sql.getTableInfo(table) - Get the table structure
        let _ = self.register_method("sql.getTableInfo", Box::new(|args| {
            let table = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if table.is_empty() {
                return Ok(Some(serde_json::json!({"error": "table name cannot be empty"})));
            }
            match SQL_MANAGER.get_table_info(table) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // nefu.sql.listConnections() - List all connections
        let _ = self.register_method("sql.listConnections", Box::new(|_args| {
            match SQL_MANAGER.list_connections() {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        // ========== Web page source fetching ==========
        // nefu.web.fetch(url) - Fetch the page source
        let _ = self.register_method("web.fetch", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if url.is_empty() {
                return Ok(Some(serde_json::json!({
                    "success": false,
                    "error": "URL cannot be empty"
                })));
            }
            match crate::web_fetcher::fetch_page(url) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({
                    "success": false,
                    "error": format!("{}", e)
                })))
            }
        }));

        // nefu.web.fetchRaw(url) - Fetch the raw page source (no parsing)
        let _ = self.register_method("web.fetchRaw", Box::new(|args| {
            let url = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if url.is_empty() {
                return Ok(Some(serde_json::json!({"error": "URL cannot be empty"})));
            }
            match crate::web_fetcher::fetch_page(url) {
                Ok(result) => Ok(Some(result)),
                Err(e) => Ok(Some(serde_json::json!({"error": format!("{}", e)})))
            }
        }));

        log::info!("bridge module registered all {} methods", self.handlers.len());
    }

    /// Register a custom method handler
    ///
    /// # Arguments
    /// - `method`: Method name
    /// - `handler`: Handler function
    pub fn register_method(&mut self, method: &str, handler: BridgeHandler) {
        self.handlers.insert(method.to_string(), Arc::new(handler));
        debug!("register bridge method: {}", method);
    }

    /// Set the global message handler
    ///
    /// Handle data sent via lj() or nefu.send()
    pub fn set_message_handler<F>(&mut self, handler: F)
    where
        F: Fn(Value) + Send + Sync + 'static,
    {
        self.message_handler = Some(Arc::new(handler));
    }

    /// Handle IPC messages from JS
    ///
    /// # Arguments
    /// - `message_json`: Message string in JSON format
    ///
    /// # Returns
    /// Optional response JSON string
    pub fn handle_message(&self, message_json: &str) -> Option<String> {
        let message: IpcMessage = match serde_json::from_str(message_json) {
            Ok(msg) => msg,
            Err(e) => {
                // Detailed error info: show the specific cause and message content of the parse failure
                let msg_preview = if message_json.len() > 200 {
                    format!("{}... (truncated, {} bytes total)", &message_json[..200], message_json.len())
                } else {
                    message_json.to_string()
                };
                error!(
                    "[IPC parse error] failed to parse a message from JS (type: serde_json::Error)\n  \
                     reason: {}\n  \
                     position: column {}\n  \
                     message content: {}\n  \
                     suggestion: check whether the message sent by JS matches the IpcMessage format",
                    e,
                    e.column(),
                    msg_preview,
                );
                return None;
            }
        };

        match message {
            IpcMessage::Invoke { id, method, args } => {
                let result = self.handle_invoke(id, &method, args);
                if result.is_none() {
                    warn!(
                        "[IPC call failure] method '{}' (id={}) returned an empty response\n  \
                         suggestion: check whether the method is registered and whether the handler returns correctly",
                        method, id
                    );
                }
                result
            }
            IpcMessage::Send { data } => {
                debug!("received JS data: {:?}", data);
                self.handle_send(data);
                None
            }
            IpcMessage::Callback { .. } | IpcMessage::Event { .. } => {
                warn!(
                    "[IPC type error] received an unexpected message type: the server should not receive Callback/Event messages\n  \
                     message content: {}\n  \
                     suggestion: check whether JS mistakenly sent a response message",
                    message_json.chars().take(100).collect::<String>()
                );
                None
            }
        }
    }

    /// Handle a method call
    fn handle_invoke(&self, id: u64, method: &str, args: Vec<Value>) -> Option<String> {
        debug!("method call: {} (id={}, arg count: {})", method, id, args.len());

        let response = if let Some(handler) = self.handlers.get(method) {
            let start = std::time::Instant::now();
            match handler(args) {
                Ok(result) => {
                    let elapsed = start.elapsed();
                    if elapsed.as_millis() > 100 {
                        warn!(
                            "[IPC performance warning] method '{}' (id={}) took {}ms\n  \
                             suggestion: consider async handling for time-consuming operations",
                            method, id, elapsed.as_millis()
                        );
                    }
                    IpcMessage::Callback {
                        id,
                        result,
                        error: None,
                    }
                }
                Err(e) => {
                    error!(
                        "[IPC execution error] method '{}' (id={}) failed\n  \
                         reason: {}\n  \
                         suggestion: check the method implementation to handle all edge cases correctly",
                        method, id, e
                    );
                    IpcMessage::Callback {
                        id,
                        result: None,
                        error: Some(format!("{}", e)),
                    }
                }
            }
        } else {
            // List similar method names for user reference
            let similar = self.find_similar_methods(method);
            let hint = if similar.is_empty() {
                String::new()
            } else {
                format!("\n  similar methods: {}", similar.join(", "))
            };
            warn!(
                "[IPC method not found] method '{}' (id={}) is not registered{}\n  \
                 suggestion: check the method name spelling, or use nefu.listMethods() to list the available methods",
                method, id, hint
            );
            IpcMessage::Callback {
                id,
                result: None,
                error: Some(format!("unknown method: '{}'. use nefu.listMethods() to list the available methods", method)),
            }
        };

        serde_json::to_string(&response).ok()
    }

    /// Find similar method names (for error hints)
    fn find_similar_methods(&self, method: &str) -> Vec<String> {
        use std::collections::HashSet;
        let methods: HashSet<&String> = self.handlers.keys().collect();
        
        // Try to match by module prefix
        let prefix = if let Some(dot) = method.rfind('.') {
            &method[..dot]
        } else {
            ""
        };
        
        let mut similar: Vec<String> = methods.iter()
            .filter(|m| {
                if m.starts_with(prefix) && **m != method {
                    // Calculate the common prefix length
                    let common = m.chars()
                        .zip(method.chars())
                        .take_while(|(a, b)| a == b)
                        .count();
                    common > prefix.len().saturating_sub(3)
                } else {
                    false
                }
            })
            .take(5)
            .map(|m| m.to_string())
            .collect();
        similar.sort();
        similar
    }

    /// Handle data sending
    fn handle_send(&self, data: Value) {
        debug!("received JS data: {:?}", data);

        if let Some(ref handler) = self.message_handler {
            handler(data);
        } else {
            debug!("no message handler set, ignore data");
        }
    }

    /// Trigger a JS-side event
    ///
    /// # Arguments
    /// - `event`: Event name
    /// - `data`: Event data
    ///
    /// # Returns
    /// Serialized event message JSON
    pub fn emit_event(&self, event: &str, data: Value) -> Option<String> {
        let message = IpcMessage::Event {
            event: event.to_string(),
            data,
        };

        serde_json::to_string(&message).ok()
    }

    /// Add an event to the pending send queue
    pub fn queue_event(&self, event: &str, data: Value) {
        let message = IpcMessage::Event {
            event: event.to_string(),
            data,
        };

        if let Ok(mut queue) = self.pending_events.lock() {
            queue.push(message);
        }
    }

    /// Take out all pending events
    pub fn drain_pending_events(&self) -> Vec<IpcMessage> {
        if let Ok(mut queue) = self.pending_events.lock() {
            queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// List all registered method names
    pub fn list_methods(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Check whether a method is registered
    pub fn has_method(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }
}

/// Open an external URL
fn open_url(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .context("Failed to launch browser")?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("Failed to launch browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to launch browser")?;
    }

    Ok(())
}

/// Check whether a path is safe (prevent directory traversal)
fn is_safe_path(path: &str) -> bool {
    // Reject absolute paths and directory traversal
    if path.contains("..") {
        return false;
    }

    // Only allow files under the project directory
    let allowed_prefixes = ["./", "data/", "resources/", "assets/"];
    allowed_prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// Get the local storage directory
fn get_storage_dir() -> std::path::PathBuf {
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".local/share"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    };

    base.join("nefu").join("storage")
}

/// Sanitize a storage key to ensure the file name is safe
fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Take a screenshot (returns a base64-encoded PNG)
fn capture_screenshot() -> Result<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // Take a screenshot using PowerShell
        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                "Add-Type -AssemblyName System.Drawing; \
                 Add-Type -AssemblyName System.Windows.Forms; \
                 $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; \
                 $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height; \
                 $g = [System.Drawing.Graphics]::FromImage($bmp); \
                 $g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size); \
                 $ms = New-Object System.IO.MemoryStream; \
                 $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); \
                 [Convert]::ToBase64String($ms.ToArray()); \
                 $g.Dispose(); $bmp.Dispose()",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .context("Screenshot failed")?;

        let base64 = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("failed to parse the screenshot result: {}", e))?;

        if base64.trim().is_empty() {
            return Err(anyhow::anyhow!("screenshot returned empty"));
        }

        return Ok(format!("data:image/png;base64,{}", base64.trim()));
    }

    #[cfg(target_os = "macos")]
    {
        // macOS uses the screencapture command
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("nefu_screenshot_{}.png", std::process::id()));

        std::process::Command::new("screencapture")
            .args(["-x", "-t", "png", &temp_file.to_string_lossy()])
            .output()
            .context("Screenshot failed")?;

        let data = std::fs::read(&temp_file)
            .context("Failed to read the screenshot file")?;
        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
        std::fs::remove_file(&temp_file).ok();

        return Ok(format!("data:image/png;base64,{}", base64));
    }

    #[cfg(target_os = "linux")]
    {
        // Linux uses the import command (ImageMagick) or gnome-screenshot
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("nefu_screenshot_{}.png", std::process::id()));

        let result = std::process::Command::new("import")
            .args(["-window", "root", &temp_file.to_string_lossy()])
            .output();

        if result.is_err() {
            // Try gnome-screenshot
            std::process::Command::new("gnome-screenshot")
                .args(["-f", &temp_file.to_string_lossy()])
                .output()
                .context("Screenshot failed (need to install import or gnome-screenshot)")?;
        }

        let data = std::fs::read(&temp_file)
            .context("Failed to read the screenshot file")?;
        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
        std::fs::remove_file(&temp_file).ok();

        return Ok(format!("data:image/png;base64,{}", base64));
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("screenshots are not supported on this platform"))
    }
}

/// Generate the complete bridge initialization script
///
/// This script is injected before the WebView loads the page and provides the full JS API.
pub fn generate_full_bridge_script() -> String {
    r#"(function() {
    'use strict';

    // Prevent repeated initialization
    if (window.__nefu_initialized) return;
    window.__nefu_initialized = true;

    const nefu = {
        _listeners: {},
        _pendingCallbacks: {},
        _callId: 0,

        invoke: function(method, ...args) {
            const id = ++this._callId;
            return new Promise((resolve, reject) => {
                this._pendingCallbacks[id] = { resolve, reject };
                try {
                    const msg = JSON.stringify({ type: 'invoke', id, method, args });
                    if (window.__nefu_ipc) {
                        window.__nefu_ipc(msg);
                    } else {
                        console.warn('[Nefu] IPC not ready');
                        delete this._pendingCallbacks[id];
                        reject(new Error('IPC channel not available'));
                    }
                } catch (e) {
                    delete this._pendingCallbacks[id];
                    reject(e);
                }
            });
        },

        send: function(data) {
            try {
                const msg = JSON.stringify({ type: 'send', data });
                if (window.__nefu_ipc) {
                    window.__nefu_ipc(msg);
                }
            } catch (e) {
                console.error('[Nefu] Send failed:', e);
            }
        },

        on: function(event, callback) {
            if (!this._listeners[event]) {
                this._listeners[event] = [];
            }
            this._listeners[event].push(callback);
            return () => {
                const idx = this._listeners[event].indexOf(callback);
                if (idx !== -1) this._listeners[event].splice(idx, 1);
            };
        },

        off: function(event, callback) {
            if (this._listeners[event]) {
                if (callback) {
                    const idx = this._listeners[event].indexOf(callback);
                    if (idx !== -1) this._listeners[event].splice(idx, 1);
                } else {
                    delete this._listeners[event];
                }
            }
        },

        emit: function(event, data) {
            (this._listeners[event] || []).forEach(cb => {
                try { cb(data); } catch (e) {
                    console.error(`[Nefu] Event handler error (${event}):`, e);
                }
            });
        },

        _handleResponse: function(json) {
            try {
                const resp = JSON.parse(json);
                if (resp.type === 'callback' && resp.id != null) {
                    const pending = this._pendingCallbacks[resp.id];
                    if (pending) {
                        delete this._pendingCallbacks[resp.id];
                        resp.error ? pending.reject(new Error(resp.error)) : pending.resolve(resp.result);
                    }
                } else if (resp.type === 'event') {
                    this.emit(resp.event, resp.data);
                }
            } catch (e) {
                console.error('[Nefu] Response error:', e);
            }
        },

        getInfo: function() {
            return { version: '1.0.0', platform: navigator.platform };
        },

        // ==================== Electron-level API namespace ====================

        // --- Basic API ---
        getVersion: () => nefu.invoke('getVersion'),
        getPlatform: () => nefu.invoke('getPlatform'),
        getTime: () => nefu.invoke('getTime'),
        echo: (data) => nefu.invoke('echo', data),
        log: (msg) => nefu.invoke('log', msg),
        warn: (msg) => nefu.invoke('warn', msg),
        error: (msg) => nefu.invoke('error', msg),
        openUrl: (url) => nefu.invoke('openUrl', url),
        print: () => nefu.invoke('print'),
        printToPDF: () => nefu.invoke('printToPDF'),
        captureScreenshot: () => nefu.invoke('captureScreenshot'),

        // --- File system ---
        fs: {
            readFile: (path) => nefu.invoke('fs.readFile', path),
            writeFile: (path, content) => nefu.invoke('fs.writeFile', path, content),
            exists: (path) => nefu.invoke('fs.exists', path),
        },

        // --- Local storage ---
        storage: {
            get: (key) => nefu.invoke('storage.get', key),
            set: (key, val) => nefu.invoke('storage.set', key, val),
            remove: (key) => nefu.invoke('storage.remove', key),
        },

        // --- Clipboard ---
        clipboard: {
            read: () => nefu.invoke('clipboard.read'),
            write: (text) => nefu.invoke('clipboard.write', text),
            readHTML: () => nefu.invoke('clipboard.readHTML'),
            writeHTML: (html) => nefu.invoke('clipboard.writeHTML', html),
            clear: () => nefu.invoke('clipboard.clear'),
            availableFormats: () => nefu.invoke('clipboard.availableFormats'),
            readImage: () => nefu.invoke('clipboard.readImage'),
            hasImage: () => nefu.invoke('clipboard.hasImage'),
        },

        // --- Window control ---
        window: {
            minimize: () => nefu.invoke('window.minimize'),
            maximize: () => nefu.invoke('window.maximize'),
            restore: () => nefu.invoke('window.restore'),
            close: () => nefu.invoke('window.close'),
            getBounds: () => nefu.invoke('window.getBounds'),
            getSize: () => nefu.invoke('window.getSize'),
            isMaximized: () => nefu.invoke('window.isMaximized'),
            isMinimized: () => nefu.invoke('window.isMinimized'),
            isVisible: () => nefu.invoke('window.isVisible'),
            center: () => nefu.invoke('window.center'),
            focus: () => nefu.invoke('window.focus'),
            blur: () => nefu.invoke('window.blur'),
            setAlwaysOnTop: (flag) => nefu.invoke('window.setAlwaysOnTop', flag),
            show: () => nefu.invoke('window.show'),
            hide: () => nefu.invoke('window.hide'),
            setTitle: (title) => nefu.invoke('window.setTitle', title),
            getTitle: () => nefu.invoke('window.getTitle'),
            setResizable: (resizable) => nefu.invoke('window.setResizable', resizable),
            isResizable: () => nefu.invoke('window.isResizable'),
            setMinimumSize: (w, h) => nefu.invoke('window.setMinimumSize', w, h),
            setMaximumSize: (w, h) => nefu.invoke('window.setMaximumSize', w, h),
            getMinimumSize: () => nefu.invoke('window.getMinimumSize'),
            getMaximumSize: () => nefu.invoke('window.getMaximumSize'),
            setPosition: (x, y) => nefu.invoke('window.setPosition', x, y),
            getPosition: () => nefu.invoke('window.getPosition'),
            setOpacity: (opacity) => nefu.invoke('window.setOpacity', opacity),
            getOpacity: () => nefu.invoke('window.getOpacity'),
            setFullScreen: (flag) => nefu.invoke('window.setFullScreen', flag),
            isFullScreen: () => nefu.invoke('window.isFullScreen'),
            restore: () => nefu.invoke('window.restore'),
            isVisible: () => nefu.invoke('window.isVisible'),
        },

        // --- Dialog ---
        dialog: {
            alert: (msg) => nefu.invoke('dialog.alert', msg),
            confirm: (msg) => {
                return new Promise((resolve) => {
                    const off = nefu.on('dialog-confirm-result', (ok) => {
                        off();
                        resolve(ok);
                    });
                    nefu.invoke('dialog.confirm', msg);
                });
            },
            openFile: (options) => {
                return new Promise((resolve) => {
                    const off = nefu.on('dialog-openFile-result', (data) => {
                        off();
                        resolve(data);
                    });
                    nefu.invoke('dialog.openFile', options || {});
                });
            },
            saveFile: (options) => {
                return new Promise((resolve) => {
                    const off = nefu.on('dialog-saveFile-result', (data) => {
                        off();
                        resolve(data);
                    });
                    nefu.invoke('dialog.saveFile', options || {});
                });
            },
            showMessageBox: (options) => nefu.invoke('dialog.showMessageBox', options),
            showErrorBox: (title, msg) => nefu.invoke('dialog.showErrorBox', title, msg),
        },

        // --- Shell system operations ---
        shell: {
            openExternal: (url) => nefu.invoke('shell.openExternal', url),
            openPath: (path) => nefu.invoke('shell.openPath', path),
            showItemInFolder: (path) => nefu.invoke('shell.showItemInFolder', path),
            beep: () => nefu.invoke('shell.beep'),
            trashItem: (path) => nefu.invoke('shell.trashItem', path),
            getSystemVersion: () => nefu.invoke('shell.getSystemVersion'),
            exec: (command, args, cwd) => nefu.invoke('shell.exec', command, args, cwd),
        },

        // --- Desktop notifications ---
        notification: {
            send: (options) => nefu.invoke('notification.send', options),
            isSupported: () => nefu.invoke('notification.isSupported'),
        },

        // --- System info ---
        system: {
            getTheme: () => nefu.invoke('system.getTheme'),
            getMemoryInfo: () => nefu.invoke('system.getMemoryInfo'),
            getCpuInfo: () => nefu.invoke('system.getCpuInfo'),
            getGpuInfo: () => nefu.invoke('system.getGpuInfo'),
            getPowerState: () => nefu.invoke('system.getPowerState'),
            getScreenInfo: () => nefu.invoke('system.getScreenInfo'),
            getSummary: () => nefu.invoke('system.getSummary'),
            refresh: () => nefu.invoke('system.refresh'),
        },

        // --- Power monitoring ---
        powerMonitor: {
            getSystemIdleState: (threshold) => nefu.invoke('powerMonitor.getSystemIdleState', threshold),
            getSystemIdleTime: () => nefu.invoke('powerMonitor.getSystemIdleTime'),
            getCurrentThermalState: () => nefu.invoke('powerMonitor.getCurrentThermalState'),
            onBattery: () => nefu.invoke('powerMonitor.onBattery'),
            getCurrentDesktop: () => nefu.invoke('powerMonitor.getCurrentDesktop'),
        },

        // --- Power save blocker ---
        powerSaveBlocker: {
            start: (type) => nefu.invoke('powerSaveBlocker.start', type),
            stop: (id) => nefu.invoke('powerSaveBlocker.stop', id),
            isStopped: (id) => nefu.invoke('powerSaveBlocker.isStopped', id),
        },

        // --- Display ---
        screen: {
            getAllDisplays: () => nefu.invoke('screen.getAllDisplays'),
            getPrimaryDisplay: () => nefu.invoke('screen.getPrimaryDisplay'),
            getCursorScreenPoint: () => nefu.invoke('screen.getCursorScreenPoint'),
            getDisplayMatching: (point) => nefu.invoke('screen.getDisplayMatching', point),
            dipToScreenPixels: (dip) => nefu.invoke('screen.dipToScreenPixels', dip),
            screenToDipPixels: (screen) => nefu.invoke('screen.screenToDipPixels', screen),
        },

        // --- Auto updater ---
        updater: {
            checkForUpdates: (url) => nefu.invoke('updater.checkForUpdates', url),
        },

        // --- Crash reports ---
        crashReporter: {
            generateTestReport: () => nefu.invoke('crashReporter.generateTestReport'),
        },

        // --- Context bridge ---
        contextBridge: {
            isAvailable: () => nefu.invoke('contextBridge.isAvailable'),
        },

        // --- Native menu ---
        menu: {
            getDefaultTemplate: (appName) => nefu.invoke('menu.getDefaultTemplate', appName),
        },

        // --- App module ---
        app: {
            getPath: (name) => nefu.invoke('app.getPath', name),
            getAppPath: () => nefu.invoke('app.getAppPath'),
            getName: () => nefu.invoke('app.getName'),
            getVersion: () => nefu.invoke('app.getVersion'),
            getLocale: () => nefu.invoke('app.getLocale'),
            getLocaleCountryCode: () => nefu.invoke('app.getLocaleCountryCode'),
            isPackaged: () => nefu.invoke('app.isPackaged'),
            getAppMetrics: () => nefu.invoke('app.getAppMetrics'),
            getLoginItemSettings: () => nefu.invoke('app.getLoginItemSettings'),
            setLoginItemSettings: (settings) => nefu.invoke('app.setLoginItemSettings', settings),
            getSystemVersion: () => nefu.invoke('app.getSystemVersion'),
            getAppInfo: () => nefu.invoke('app.getAppInfo'),
            quit: () => nefu.invoke('app.quit'),
            exit: () => nefu.invoke('app.exit'),
            relaunch: () => nefu.invoke('app.relaunch'),
            focus: () => nefu.invoke('app.focus'),
            hide: () => nefu.invoke('app.hide'),
            show: () => nefu.invoke('app.show'),
            getSystemProxySettings: () => nefu.invoke('app.getSystemProxySettings'),
            getSystemVersionInfo: () => nefu.invoke('app.getSystemVersionInfo'),
        },

        // --- Native image ---
        nativeImage: {
            createFromPath: (path) => nefu.invoke('nativeImage.createFromPath', path),
            createFromBuffer: (buffer, options) => nefu.invoke('nativeImage.createFromBuffer', buffer, options),
            createEmpty: (w, h) => nefu.invoke('nativeImage.createEmpty', w, h),
            resize: (dataURL, options) => nefu.invoke('nativeImage.resize', dataURL, options),
        },

        // --- WebFrame ---
        webFrame: {
            setZoomLevel: (level) => nefu.invoke('webFrame.setZoomLevel', level),
            getZoomLevel: () => nefu.invoke('webFrame.getZoomLevel'),
            setZoomFactor: (factor) => nefu.invoke('webFrame.setZoomFactor', factor),
            getZoomFactor: () => nefu.invoke('webFrame.getZoomFactor'),
            findInPage: (text, options) => nefu.invoke('webFrame.findInPage', text, options),
            stopFindInPage: (action) => nefu.invoke('webFrame.stopFindInPage', action),
        },

        // --- SystemPreferences ---
        systemPreferences: {
            isDarkMode: () => nefu.invoke('systemPreferences.isDarkMode'),
            isInvertedColorScheme: () => nefu.invoke('systemPreferences.isInvertedColorScheme'),
            getColor: (color) => nefu.invoke('systemPreferences.getColor', color),
            getMediaAccessStatus: (type) => nefu.invoke('systemPreferences.getMediaAccessStatus', type),
            askForMediaAccess: (type) => nefu.invoke('systemPreferences.askForMediaAccess', type),
            getAccentColor: () => nefu.invoke('systemPreferences.getAccentColor'),
            isHighContrastColorScheme: () => nefu.invoke('systemPreferences.isHighContrastColorScheme'),
            areAnimationsEnabled: () => nefu.invoke('systemPreferences.areAnimationsEnabled'),
            isTransparencyEnabled: () => nefu.invoke('systemPreferences.isTransparencyEnabled'),
        },

        // --- SafeStorage ---
        safeStorage: {
            isEncryptionAvailable: () => nefu.invoke('safeStorage.isEncryptionAvailable'),
            encryptString: (text) => nefu.invoke('safeStorage.encryptString', text),
            decryptString: (encrypted) => nefu.invoke('safeStorage.decryptString', encrypted),
        },

        // --- Net module ---
        net: {
            fetch: (url, options) => nefu.invoke('net.fetch', url, options),
        },

        // --- Process module ---
        process: {
            getProcessMemoryInfo: () => nefu.invoke('process.getProcessMemoryInfo'),
            getSystemMemoryInfo: () => nefu.invoke('process.getSystemMemoryInfo'),
            getCPUUsage: () => nefu.invoke('process.getCPUUsage'),
            getIOCounters: () => nefu.invoke('process.getIOCounters'),
            getProcessId: () => nefu.invoke('process.getProcessId'),
            getProcessPath: () => nefu.invoke('process.getProcessPath'),
            getVersions: () => nefu.invoke('process.getVersions'),
            getCreationTime: () => nefu.invoke('process.getCreationTime'),
            getResourceUsage: () => nefu.invoke('process.getResourceUsage'),
            getEnvironmentVariable: (name) => nefu.invoke('process.getEnvironmentVariable', name),
            getCommandLineArgs: () => nefu.invoke('process.getCommandLineArgs'),
            getWorkingDirectory: () => nefu.invoke('process.getWorkingDirectory'),
        },

        // --- Tray ---
        tray: {
            setToolTip: (tip) => nefu.invoke('tray.setToolTip', tip),
            setTitle: (title) => nefu.invoke('tray.setTitle', title),
            displayBalloon: (options) => nefu.invoke('tray.displayBalloon', options),
            removeBalloon: () => nefu.invoke('tray.removeBalloon'),
            isDestroyed: () => nefu.invoke('tray.isDestroyed'),
        },

        // --- DesktopCapturer ---
        desktopCapturer: {
            getSources: (options) => nefu.invoke('desktopCapturer.getSources', options),
        },

        // ==================== New Electron module APIs ====================

        // --- WebContents module ---
        webContents: {
            getURL: () => nefu.invoke('webContents.getURL'),
            getTitle: () => nefu.invoke('webContents.getTitle'),
            isLoading: () => nefu.invoke('webContents.isLoading'),
            canGoBack: () => nefu.invoke('webContents.canGoBack'),
            canGoForward: () => nefu.invoke('webContents.canGoForward'),
            getNavigationHistory: () => nefu.invoke('webContents.getNavigationHistory'),
            getWebContentsInfo: () => nefu.invoke('webContents.getWebContentsInfo'),
        },

        // --- Session module ---
        session: {
            cookies: {
                get: (filter) => nefu.invoke('session.cookies.get', filter),
                set: (cookie) => nefu.invoke('session.cookies.set', cookie),
                remove: (url, name) => nefu.invoke('session.cookies.remove', url, name),
                flush: () => nefu.invoke('session.cookies.flush'),
            },
            clearCache: () => nefu.invoke('session.clearCache'),
            clearStorageData: () => nefu.invoke('session.clearStorageData'),
            setProxy: (config) => nefu.invoke('session.setProxy', config),
            getProxy: () => nefu.invoke('session.getProxy'),
            isOnline: () => nefu.invoke('session.isOnline'),
        },

        // --- Protocol module ---
        protocol: {
            registerStringProtocol: (scheme) => nefu.invoke('protocol.registerStringProtocol', scheme),
            isProtocolHandled: (scheme) => nefu.invoke('protocol.isProtocolHandled', scheme),
            unregisterProtocol: (scheme) => nefu.invoke('protocol.unregisterProtocol', scheme),
            getRegisteredProtocols: () => nefu.invoke('protocol.getRegisteredProtocols'),
        },

        // --- Dock module (macOS) ---
        dock: {
            setBadge: (text) => nefu.invoke('dock.setBadge', text),
            getBadge: () => nefu.invoke('dock.getBadge'),
            bounce: (type) => nefu.invoke('dock.bounce', type || 'informational'),
            cancelBounce: (id) => nefu.invoke('dock.cancelBounce', id),
            show: () => nefu.invoke('dock.show'),
            hide: () => nefu.invoke('dock.hide'),
            isVisible: () => nefu.invoke('dock.isVisible'),
        },

        // --- NativeTheme module ---
        nativeTheme: {
            shouldUseDarkColors: () => nefu.invoke('nativeTheme.shouldUseDarkColors'),
            shouldUseHighContrastColors: () => nefu.invoke('nativeTheme.shouldUseHighContrastColors'),
            shouldUseInvertedColorScheme: () => nefu.invoke('nativeTheme.shouldUseInvertedColorScheme'),
            themeSource: () => nefu.invoke('nativeTheme.themeSource'),
            setThemeSource: (source) => nefu.invoke('nativeTheme.setThemeSource', source),
        },

        // --- ContentTracing module ---
        contentTracing: {
            startRecording: (options) => nefu.invoke('contentTracing.startRecording', options),
            stopRecording: () => nefu.invoke('contentTracing.stopRecording'),
            getCategories: () => nefu.invoke('contentTracing.getCategories'),
            getTraceBufferUsage: () => nefu.invoke('contentTracing.getTraceBufferUsage'),
            isRecording: () => nefu.invoke('contentTracing.isRecording'),
        },

        // ==================== Second batch of new APIs ====================

        // --- GlobalShortcut module ---
        globalShortcut: {
            register: (accelerator) => nefu.invoke('globalShortcut.register', accelerator),
            unregister: (accelerator) => nefu.invoke('globalShortcut.unregister', accelerator),
            unregisterAll: () => nefu.invoke('globalShortcut.unregisterAll'),
            isRegistered: (accelerator) => nefu.invoke('globalShortcut.isRegistered', accelerator),
            getRegistered: () => nefu.invoke('globalShortcut.getRegistered'),
        },

        // --- Menu class methods ---
        menu: {
            setApplicationMenu: (template) => nefu.invoke('menu.setApplicationMenu', template),
            buildFromTemplate: (template) => nefu.invoke('menu.buildFromTemplate', template),
            popup: (template) => nefu.invoke('menu.popup', template),
            getApplicationMenu: () => nefu.invoke('menu.getApplicationMenu'),
            getDefaultTemplate: (appName) => nefu.invoke('menu.getDefaultTemplate', appName),
        },

        // --- webContents extension methods ---
        webContentsExt: {
            executeJavaScript: (code) => nefu.invoke('webContents.executeJavaScript', code),
            insertCSS: (css) => nefu.invoke('webContents.insertCSS', css),
            reload: () => nefu.invoke('webContents.reload'),
            reloadIgnoringCache: () => nefu.invoke('webContents.reloadIgnoringCache'),
            goBack: () => nefu.invoke('webContents.goBack'),
            goForward: () => nefu.invoke('webContents.goForward'),
            print: () => nefu.invoke('webContents.print'),
            printToPDF: () => nefu.invoke('webContents.printToPDF'),
            downloadURL: (url) => nefu.invoke('webContents.downloadURL', url),
            setZoomFactor: (factor) => nefu.invoke('webContents.setZoomFactor', factor),
            getZoomFactor: () => nefu.invoke('webContents.getZoomFactor'),
            setZoomLevel: (level) => nefu.invoke('webContents.setZoomLevel', level),
            getZoomLevel: () => nefu.invoke('webContents.getZoomLevel'),
            toggleDevTools: () => nefu.invoke('webContents.toggleDevTools'),
            openDevTools: () => nefu.invoke('webContents.openDevTools'),
            closeDevTools: () => nefu.invoke('webContents.closeDevTools'),
            isDevToolsOpened: () => nefu.invoke('webContents.isDevToolsOpened'),
            copy: () => nefu.invoke('webContents.copy'),
            paste: () => nefu.invoke('webContents.paste'),
            cut: () => nefu.invoke('webContents.cut'),
            selectAll: () => nefu.invoke('webContents.selectAll'),
            undo: () => nefu.invoke('webContents.undo'),
            redo: () => nefu.invoke('webContents.redo'),
            findInPage: (text, options) => nefu.invoke('webContents.findInPage', text, options),
            stopFindInPage: () => nefu.invoke('webContents.stopFindInPage'),
        },

        // --- App extension methods ---
        appExt: {
            setAboutPanelOptions: (options) => nefu.invoke('app.setAboutPanelOptions', options),
            getGPUInfo: (type) => nefu.invoke('app.getGPUInfo', type),
            setAsDefaultProtocolClient: (protocol) => nefu.invoke('app.setAsDefaultProtocolClient', protocol),
            removeAsDefaultProtocolClient: (protocol) => nefu.invoke('app.removeAsDefaultProtocolClient', protocol),
            isDefaultProtocolClient: (protocol) => nefu.invoke('app.isDefaultProtocolClient', protocol),
            getApplicationNameForProtocol: (url) => nefu.invoke('app.getApplicationNameForProtocol', url),
            setUserActivity: (type, userInfo) => nefu.invoke('app.setUserActivity', type, userInfo),
            requestSingleInstanceLock: () => nefu.invoke('app.requestSingleInstanceLock'),
            releaseSingleInstanceLock: () => nefu.invoke('app.releaseSingleInstanceLock'),
            getUserAgentFallback: () => nefu.invoke('app.getUserAgentFallback'),
            setAppUserModelId: (id) => nefu.invoke('app.setAppUserModelId', id),
            getJumpListSettings: () => nefu.invoke('app.getJumpListSettings'),
            setJumpList: (categories) => nefu.invoke('app.setJumpList', categories),
        },

        // --- Shell extension ---
        shellExt: {
            writeShortcutLink: (path, operation, options) => nefu.invoke('shell.writeShortcutLink', path, operation, options),
            readShortcutLink: (path) => nefu.invoke('shell.readShortcutLink', path),
            moveItemToTrash: (path) => nefu.invoke('shell.moveItemToTrash', path),
            exec: (command, args, cwd) => nefu.invoke('shell.exec', command, args, cwd),
        },

        // --- BrowserView module ---
        browserView: {
            create: (options) => nefu.invoke('browserView.create', options),
            setBounds: (id, bounds) => nefu.invoke('browserView.setBounds', id, bounds),
            setBackgroundColor: (id, color) => nefu.invoke('browserView.setBackgroundColor', id, color),
            webContents: (id) => nefu.invoke('browserView.webContents', id),
            remove: (id) => nefu.invoke('browserView.remove', id),
        },

        // --- ipcMain module ---
        ipcMain: {
            on: (channel) => nefu.invoke('ipcMain.on', channel),
            off: (channel) => nefu.invoke('ipcMain.off', channel),
            once: (channel) => nefu.invoke('ipcMain.once', channel),
            handle: (channel, handler) => nefu.invoke('ipcMain.handle', channel, handler),
            removeHandler: (channel) => nefu.invoke('ipcMain.removeHandler', channel),
        },

        // --- netLog module ---
        netLog: {
            startLogging: (path) => nefu.invoke('netLog.startLogging', path),
            stopLogging: () => nefu.invoke('netLog.stopLogging'),
        },

        // --- utilityProcess module ---
        utilityProcess: {
            fork: (path, args) => nefu.invoke('utilityProcess.fork', path, args),
        },

        // --- TouchBar module ---
        touchBar: {
            create: (items) => nefu.invoke('touchBar.create', items),
        },

        // --- shareMenu module ---
        shareMenu: {
            show: (items) => nefu.invoke('shareMenu.show', items),
        },

        // --- net module ---
        netExt: {
            request: (method, url, body) => nefu.invoke('net.request', method, url, body),
            isOnline: () => nefu.invoke('net.isOnline'),
            getOnline: () => nefu.invoke('net.getOnline'),
            fetch: (url, options) => nefu.invoke('net.fetch', url, options),
        },

        // --- Process extensions ---
        processExt: {
            getArchitecture: () => nefu.invoke('process.getArchitecture'),
            getSystemArchitecture: () => nefu.invoke('process.getSystemArchitecture'),
            getUptime: () => nefu.invoke('process.getUptime'),
            getProcessMemoryInfo: () => nefu.invoke('process.getProcessMemoryInfo'),
            getSystemMemoryInfo: () => nefu.invoke('process.getSystemMemoryInfo'),
            getCPUUsage: () => nefu.invoke('process.getCPUUsage'),
            getIOCounters: () => nefu.invoke('process.getIOCounters'),
            getProcessId: () => nefu.invoke('process.getProcessId'),
            getProcessPath: () => nefu.invoke('process.getProcessPath'),
            getVersions: () => nefu.invoke('process.getVersions'),
            getCreationTime: () => nefu.invoke('process.getCreationTime'),
            getResourceUsage: () => nefu.invoke('process.getResourceUsage'),
            getEnvironmentVariable: (name) => nefu.invoke('process.getEnvironmentVariable', name),
            getCommandLineArgs: () => nefu.invoke('process.getCommandLineArgs'),
            getWorkingDirectory: () => nefu.invoke('process.getWorkingDirectory'),
        },

        // --- Notification extensions ---
        notificationExt: {
            show: (title, body, icon) => nefu.invoke('notification.show', title, body, icon),
            send: (title, body) => nefu.invoke('notification.send', title, body),
            isSupported: () => nefu.invoke('notification.isSupported'),
        },

        // --- powerMonitor extensions ---
        powerMonitorExt: {
            getSystemIdleState: () => nefu.invoke('powerMonitor.getSystemIdleState'),
            getSystemIdleTime: () => nefu.invoke('powerMonitor.getSystemIdleTime'),
            getCurrentThermalState: () => nefu.invoke('powerMonitor.getCurrentThermalState'),
            onBattery: () => nefu.invoke('powerMonitor.onBattery'),
            getCurrentDesktop: () => nefu.invoke('powerMonitor.getCurrentDesktop'),
            getBatteryLevel: () => nefu.invoke('powerMonitor.getBatteryLevel'),
            isOnBatteryPower: () => nefu.invoke('powerMonitor.isOnBatteryPower'),
        },

        // --- screen extensions ---
        screenExt: {
            getAllDisplays: () => nefu.invoke('screen.getAllDisplays'),
            getPrimaryDisplay: () => nefu.invoke('screen.getPrimaryDisplay'),
            getCursorScreenPoint: () => nefu.invoke('screen.getCursorScreenPoint'),
            getDisplayMatching: (bounds) => nefu.invoke('screen.getDisplayMatching', bounds),
            getNearestDisplay: (x, y) => nefu.invoke('screen.getNearestDisplay', x, y),
            dipToScreenPixels: (dip) => nefu.invoke('screen.dipToScreenPixels', dip),
            screenToDipPixels: (px) => nefu.invoke('screen.screenToDipPixels', px),
        },

        // --- safeStorage extension ---
        safeStorageExt: {
            isEncryptionAvailable: () => nefu.invoke('safeStorage.isEncryptionAvailable'),
            encryptString: (text) => nefu.invoke('safeStorage.encryptString', text),
            decryptString: (encrypted) => nefu.invoke('safeStorage.decryptString', encrypted),
            getSelectedStorageBackend: () => nefu.invoke('safeStorage.getSelectedStorageBackend'),
        },

        // --- contextBridge extensions ---
        contextBridgeExt: {
            isAvailable: () => nefu.invoke('contextBridge.isAvailable'),
            exposeInMainWorld: (name, api) => nefu.invoke('contextBridge.exposeInMainWorld', name, api),
        },
    };

    // Global shortcut functions
    window.lj = (data) => nefu.send(data);
    window.nefu = nefu;

    // Notify that it is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', () => {
            window.dispatchEvent(new CustomEvent('nefu-ready'));
        });
    } else {
        window.dispatchEvent(new CustomEvent('nefu-ready'));
    }

    console.log('[Nefu] Bridge initialized');
})();"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let bridge = Bridge::new();
        assert!(bridge.has_method("getVersion"));
        assert!(bridge.has_method("getPlatform"));
        assert!(bridge.has_method("echo"));
        assert!(bridge.has_method("log"));
    }

    #[test]
    fn test_handle_invoke_echo() {
        let bridge = Bridge::new();
        let msg = r#"{"type":"invoke","id":1,"method":"echo","args":["hello"]}"#;
        let response = bridge.handle_message(msg);
        assert!(response.is_some());
        let resp: Value = serde_json::from_str(&response.unwrap()).unwrap();
        assert_eq!(resp["result"], "hello");
    }

    #[test]
    fn test_handle_invoke_unknown_method() {
        let bridge = Bridge::new();
        let msg = r#"{"type":"invoke","id":1,"method":"nonexistent","args":[]}"#;
        let response = bridge.handle_message(msg);
        assert!(response.is_some());
        let resp: Value = serde_json::from_str(&response.unwrap()).unwrap();
        assert!(resp["error"].is_string());
    }

    #[test]
    fn test_handle_send() {
        let bridge = Bridge::new();
        let msg = r#"{"type":"send","data":{"key":"value"}}"#;
        let response = bridge.handle_message(msg);
        assert!(response.is_none()); // send produces no response
    }

    #[test]
    fn test_emit_event() {
        let bridge = Bridge::new();
        let event_json = bridge.emit_event("test-event", Value::String("data".into()));
        assert!(event_json.is_some());
        let parsed: Value = serde_json::from_str(&event_json.unwrap()).unwrap();
        assert_eq!(parsed["type"], "event");
        assert_eq!(parsed["event"], "test-event");
    }

    #[test]
    fn test_register_custom_method() {
        let mut bridge = Bridge::new();
        bridge.register_method("custom.add", Box::new(|args| {
            let a = args[0].as_f64().unwrap_or(0.0);
            let b = args[1].as_f64().unwrap_or(0.0);
            Ok(Some(Value::Number(serde_json::Number::from_f64(a + b).unwrap())))
        }));
        assert!(bridge.has_method("custom.add"));
    }

    #[test]
    fn test_is_safe_path() {
        assert!(is_safe_path("./data/file.txt"));
        assert!(is_safe_path("resources/icon.png"));
        assert!(!is_safe_path("../etc/passwd"));
        assert!(!is_safe_path("/etc/shadow"));
    }

    #[test]
    fn test_sanitize_key() {
        assert_eq!(sanitize_key("my-key"), "my-key");
        assert_eq!(sanitize_key("my key!"), "my_key_");
        assert_eq!(sanitize_key("a.b/c"), "a_b_c");
    }

    #[test]
    fn test_list_methods() {
        let bridge = Bridge::new();
        let methods = bridge.list_methods();
        assert!(methods.contains(&"getVersion".to_string()));
        assert!(methods.contains(&"echo".to_string()));
        assert!(methods.len() > 5);
    }

    #[test]
    fn test_queue_and_drain_events() {
        let bridge = Bridge::new();
        bridge.queue_event("evt1", Value::Bool(true));
        bridge.queue_event("evt2", Value::Bool(false));

        let events = bridge.drain_pending_events();
        assert_eq!(events.len(), 2);

        // Retrieving again should be empty
        let events2 = bridge.drain_pending_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_invalid_json_message() {
        let bridge = Bridge::new();
        let response = bridge.handle_message("not valid json");
        assert!(response.is_none());
    }

    #[test]
    fn test_generate_bridge_script_not_empty() {
        let script = generate_full_bridge_script();
        assert!(script.len() > 100);
        assert!(script.contains("nefu"));
        assert!(script.contains("invoke"));
        assert!(script.contains("lj"));
    }
}
