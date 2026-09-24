//! Nefu WebView management module
//!
//! Responsible for creating and managing desktop windows and WebView instances.
//! Uses tao for window management and wry for WebView functionality.
//! Supports custom protocols, window state persistence, system tray, and more.

use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use wry::WebViewBuilder;

use crate::bridge::{Bridge, WindowCommand};
use crate::config::NefuConfig;
use crate::pack::ResourcePack;
use crate::rendering::{RenderConfig, RenderingMode, generate_render_optimization_script};

/// Window state persistence data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// Window X coordinate
    pub x: i32,
    /// Window Y coordinate
    pub y: i32,
    /// Window width
    pub width: u32,
    /// Window height
    pub height: u32,
    /// Whether the window is maximized
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: -1, // -1 means use the system default position
            y: -1,
            width: 1024,
            height: 768,
            maximized: false,
        }
    }
}

impl WindowState {
    /// Load the window state from a file
    ///
    /// # Arguments
    /// - `path`: Path to the state file
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save the window state to a file
    ///
    /// # Arguments
    /// - `path`: Target file path
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create the window state directory")?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize the window state")?;

        std::fs::write(path, content)
            .context("Failed to write the window state file")?;

        Ok(())
    }

    /// Get the window state file path
    ///
    /// Stored under .nefu/window_state.json in the user data directory
    pub fn state_file_path(config: &NefuConfig) -> PathBuf {
        let data_dir = dirs_or_default();
        data_dir
            .join(".nefu")
            .join(&config.output)
            .join("window_state.json")
    }
}

/// Get the user data directory, falling back to the current directory on failure
fn dirs_or_default() -> PathBuf {
    // Try to get the user data directory
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local/share");
        }
    }

    // Fall back to the current directory
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// WebView application manager
///
/// Encapsulates the full lifecycle management of windows and WebView
pub struct WebViewApp {
    /// Project configuration
    config: NefuConfig,
    /// Resource pack (packaged mode) or None (dev mode)
    resource_pack: Option<Arc<Mutex<ResourcePack>>>,
    /// Dev server URL (dev mode)
    dev_url: Option<String>,
    /// Window state file path
    state_path: PathBuf,
}

impl WebViewApp {
    /// Create a new WebView application instance (packaged mode)
    ///
    /// # Arguments
    /// - `config`: Project configuration
    /// - `resource_pack`: Unpacked resource pack
    pub fn new_packaged(config: NefuConfig, resource_pack: ResourcePack) -> Self {
        let state_path = WindowState::state_file_path(&config);
        Self {
            config,
            resource_pack: Some(Arc::new(Mutex::new(resource_pack))),
            dev_url: None,
            state_path,
        }
    }

    /// Create a new WebView application instance (dev mode)
    ///
    /// # Arguments
    /// - `config`: Project configuration
    /// - `dev_url`: Dev server URL
    pub fn new_dev(config: NefuConfig, dev_url: String) -> Self {
        let state_path = WindowState::state_file_path(&config);
        Self {
            config,
            resource_pack: None,
            dev_url: Some(dev_url),
            state_path,
        }
    }

    /// Start the application main loop
    ///
    /// Create the window, initialize the WebView, and enter the event loop.
    /// This function blocks until the window is closed.
    pub fn run(self) -> Result<()> {
        info!("starting WebView application...");

        // Load the saved window state
        let saved_state = if self.config.persist_window_state {
            WindowState::load(&self.state_path)
        } else {
            None
        };

        // Build the event loop
        let event_loop = EventLoopBuilder::new().build();

        // Build the window
        let mut window_builder = WindowBuilder::new()
            .with_title(self.config.get_title())
            .with_inner_size(LogicalSize::new(
                self.config.window_width as f64,
                self.config.window_height as f64,
            ))
            .with_resizable(self.config.resizable)
            .with_decorations(self.config.decorations)
            .with_fullscreen(if self.config.fullscreen {
                Some(tao::window::Fullscreen::Borderless(None))
            } else {
                None
            })
            .with_always_on_top(self.config.always_on_top);

        // Window icon (applied when present and decodable)
        if let Some(icon) = load_window_icon(&self.config) {
            window_builder = window_builder.with_window_icon(Some(icon));
        }

        // Drag-and-drop file support (Windows platform extension)
        #[cfg(windows)]
        {
            use tao::platform::windows::WindowBuilderExtWindows;
            window_builder = window_builder.with_drag_and_drop(self.config.drag_drop);
        }

        // Apply the saved window position and size
        if let Some(ref state) = saved_state {
            if state.x >= 0 && state.y >= 0 {
                window_builder = window_builder.with_position(PhysicalPosition::new(
                    state.x,
                    state.y,
                ));
            }
            if !self.config.fullscreen {
                window_builder = window_builder.with_inner_size(LogicalSize::new(
                    state.width as f64,
                    state.height as f64,
                ));
            }
        }

        // Set the minimum/maximum size
        if let (Some(min_w), Some(min_h)) = (self.config.min_width, self.config.min_height) {
            window_builder = window_builder.with_min_inner_size(LogicalSize::new(
                min_w as f64,
                min_h as f64,
            ));
        }

        if let (Some(max_w), Some(max_h)) = (self.config.max_width, self.config.max_height) {
            window_builder = window_builder.with_max_inner_size(LogicalSize::new(
                max_w as f64,
                max_h as f64,
            ));
        }

        let window = window_builder.build(&event_loop)
            .context("Failed to create the window")?;

        // If it was maximized before, restore the maximized state
        if let Some(ref state) = saved_state {
            if state.maximized {
                window.set_maximized(true);
            }
        }

        info!("window created: {}x{}", self.config.window_width, self.config.window_height);

        // ==================== JS-Rust bridge ====================
        // The bridge manager holds all built-in methods; IPC messages are queued first,
        // then processed in the main event loop and results are pushed back to the page.
        let bridge = Arc::new(Bridge::new());

        // Window control command channel (JS -> main loop)
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<WindowCommand>();
        bridge.set_window_command_sender(cmd_tx);

        // JS IPC message queue
        let ipc_queue: Arc<Mutex<std::collections::VecDeque<String>>> =
            Arc::new(Mutex::new(std::collections::VecDeque::new()));

        // Build the WebView
        let webview_builder = WebViewBuilder::new(&window)
            .with_user_agent(&self.config.user_agent);

        // Set the URL or custom protocol according to the mode
        let webview_builder = if let Some(ref url) = self.dev_url {
            info!("dev mode: loading {}", url);
            webview_builder.with_url(url)
        } else {
            // Packaged mode: use the custom protocol
            let protocol_name = self.config.protocol.clone();
            let entry = self.config.entry.clone();
            let pack = self.resource_pack.clone();
            let proto_config = self.config.clone();

            info!("packaged mode: using {}:// protocol", protocol_name);

            let protocol_for_handler = protocol_name.clone();

            // Register the custom protocol handler
            let builder = webview_builder.with_asynchronous_custom_protocol(
                protocol_name.clone(),
                move |request, responder| {
                    let uri = request.uri().to_string();
                    log::debug!("protocol request: {}", uri);

                    // Parse the request path
                    let path = extract_path_from_uri(&uri, &protocol_for_handler);

                    // Get the file from the resource pack
                    if let Some(ref pack) = pack {
                        if let Ok(mut guard) = pack.lock() {
                            if let Some(content) = guard.get(&path) {
                                let mime = guess_mime_type(&path);

                                // Build the response headers: MIME + CORS + security policy
                                let mut resp = wry::http::Response::builder()
                                    .header("Content-Type", mime)
                                    .header("Access-Control-Allow-Origin", "*");

                                if let Some(csp) = proto_config.csp.as_deref() {
                                    resp = resp.header("Content-Security-Policy", csp);
                                }
                                for (name, value) in &proto_config.headers {
                                    resp = resp.header(name.as_str(), value.as_str());
                                }

                                let response = resp.body(content.clone()).unwrap();
                                responder.respond(response);
                                return;
                            }
                        }
                    }

                    // File not found
                    let not_found = wry::http::Response::builder()
                        .status(404)
                        .header("Content-Type", "text/html; charset=utf-8")
                        .body(generate_404_page(&path).into_bytes())
                        .unwrap();
                    responder.respond(not_found);
                },
            );

            // Set the initial URL
            let initial_url = format!("{}://localhost/{}", protocol_name, entry);
            builder.with_url(&initial_url)
        };

        // Register the IPC handler: queue JS messages and process them uniformly in the main loop
        let queue = ipc_queue.clone();
        let webview_builder = webview_builder.with_ipc_handler(
            move |request: wry::http::Request<String>| {
                let body = request.body().clone();
                if let Ok(mut q) = queue.lock() {
                    q.push_back(body);
                }
            },
        );

        // Inject the initialization script: bridge + user preload + render optimization + (optional) disable context menu
        let mut init_scripts = Vec::new();
        init_scripts.push(generate_bridge_script());
        
        // Add the render optimization script
        if self.config.render_optimization {
            let render_config = RenderConfig {
                rendering_mode: match self.config.rendering_mode.as_str() {
                    "high-performance" => RenderingMode::HighPerformance,
                    "power-saving" => RenderingMode::PowerSaving,
                    _ => RenderingMode::Auto,
                },
                ..RenderConfig::default()
            };
            init_scripts.push(generate_render_optimization_script(&render_config));
        }
        
        if let Some(preload) = self.config.preload.as_ref() {
            let preload_path = self.config.preload_path(std::path::Path::new("."));
            if let Some(path) = preload_path {
                match std::fs::read_to_string(&path) {
                    Ok(content) if !content.trim().is_empty() => {
                        init_scripts.push(format!(
                            "\n// === User preload script ({}) ===\n{}",
                            preload, content
                        ));
                    }
                    _ => warn!("preload script unreadable: {}", preload),
                }
            }
        }
        if !self.config.context_menu {
            init_scripts.push(disable_context_menu_script());
        }
        let webview_builder =
            webview_builder.with_initialization_script(&init_scripts.join("\n"));

        // Enable developer tools in debug mode
        #[cfg(debug_assertions)]
        let webview_builder = webview_builder.with_devtools(true);

        let webview = webview_builder.build()
            .context("Failed to create the WebView")?;

        info!("WebView initialized");

        // System tray (enabled when tray_icon is explicitly configured)
        // Named `_tray`: the TrayIcon must live until the app exits; held by this scope
        let _tray = setup_tray(&self.config);

        // Save the config reference for event handling
        let persist_state = self.config.persist_window_state;
        let state_path = self.state_path.clone();
        let bridge = bridge.clone();
        let ipc_queue = ipc_queue.clone();
        let cmd_rx = cmd_rx;

        // Enter the event loop
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::NewEvents(StartCause::Init) => {
                    info!("application event loop started");
                }

                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    info!("received window close request");

                    // Save the window state
                    if persist_state {
                        save_current_window_state(&window, &state_path);
                    }

                    *control_flow = ControlFlow::Exit;
                }

                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    log::trace!("window size changed: {}x{}", size.width, size.height);
                }

                Event::WindowEvent {
                    event: WindowEvent::Moved(position),
                    ..
                } => {
                    log::trace!("window position changed: ({}, {})", position.x, position.y);
                }

                Event::MainEventsCleared => {
                    // ---- 1. Process JS IPC messages (max 50 per frame, to avoid flooding the main loop) ----
                    const MAX_IPC_PER_FRAME: usize = 50;
                    let mut processed = 0usize;
                    let mut has_pending = false;
                    loop {
                        let next = ipc_queue
                            .lock()
                            .ok()
                            .and_then(|mut q| q.pop_front());
                        match next {
                            Some(message) => {
                                match bridge.handle_message(&message) {
                                    Some(resp) => push_to_webview(&webview, &resp),
                                    None => {
                                        // Processing failed but the error was logged; continue with the next message
                                    }
                                }
                                processed += 1;
                                if processed >= MAX_IPC_PER_FRAME {
                                    // Check whether there are remaining messages
                                    if ipc_queue.lock().ok().map(|q| !q.is_empty()).unwrap_or(false) {
                                        has_pending = true;
                                        log::trace!(
                                            "IPC queue still has messages, continue processing next frame (processed {}/{})",
                                            processed,
                                            ipc_queue.lock().ok().map(|q| q.len()).unwrap_or(0) + processed
                                        );
                                    }
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    // If the queue still has messages, request the next frame to continue (avoid flooding the main loop)
                    if has_pending {
                        window.request_redraw();
                    }

                    // ---- 2. Execute window control commands from JS (produced during IPC processing above) ----
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            WindowCommand::Minimize => {
                                log::debug!("window command: minimize");
                                window.set_minimized(true);
                            }
                            WindowCommand::Maximize => {
                                log::debug!("window command: maximize");
                                window.set_maximized(!window.is_maximized());
                            }
                            WindowCommand::Restore => {
                                log::debug!("window command: restore");
                                window.set_minimized(false);
                                window.set_maximized(false);
                            }
                            WindowCommand::Close => {
                                log::debug!("window command: close");
                                if persist_state {
                                    save_current_window_state(&window, &state_path);
                                }
                                *control_flow = ControlFlow::Exit;
                                return;
                            }
                        }
                    }

                    // ---- 3. Push pending Rust -> JS events ----
                    for event_msg in bridge.drain_pending_events() {
                        if let Ok(json) = serde_json::to_string(&event_msg) {
                            push_to_webview(&webview, &json);
                        }
                    }

                    // ---- 4. Handle system tray events ----
                    poll_tray_events(&window, &mut |action| match action {
                        TrayAction::Show => {
                            window.set_minimized(false);
                            window.set_focus();
                        }
                        TrayAction::Quit => {
                            if persist_state {
                                save_current_window_state(&window, &state_path);
                            }
                            *control_flow = ControlFlow::Exit;
                        }
                    });
                }

                _ => {}
            }

            // After RedrawEventsCleared, set Wait so the event loop enters a waiting state
            // Note: once Exit is set in CloseRequested, tao exits immediately after the check,
            // so RedrawEventsCleared is not dispatched again and Exit is not overridden
            if matches!(event, Event::RedrawEventsCleared) {
                *control_flow = ControlFlow::Wait;
            }
        });
    }
}

/// Push one IPC JSON message back to the page JS
///
/// Delivered via `window.nefu._handleResponse(json)`: callbacks results or event notifications.
fn push_to_webview(webview: &wry::WebView, json: &str) {
    let encoded = serde_json::to_string(json).unwrap_or_else(|_| "\"\"".to_string());
    let js = format!("window.nefu && window.nefu._handleResponse({});", encoded);
    if let Err(e) = webview.evaluate_script(&js) {
        log::debug!("failed to push JS message: {}", e);
    }
}

/// Load the window icon from a file (tao Icon, RGBA decode)
fn load_window_icon(config: &NefuConfig) -> Option<tao::window::Icon> {
    let path = config.icon.as_ref()?;
    let path = std::path::Path::new(path);
    if !path.exists() {
        return None;
    }
    match image::open(path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            match tao::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                Ok(icon) => {
                    info!("loaded window icon: {}", path.display());
                    Some(icon)
                }
                Err(e) => {
                    warn!("invalid window icon format: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            warn!("failed to load window icon {}: {}", path.display(), e);
            None
        }
    }
}

/// Generate the tray icon (prefer the config file; fall back to a dark square)
fn load_tray_icon(config: &NefuConfig) -> tray_icon::Icon {
    let candidate = config.tray_icon.as_ref().or(config.icon.as_ref());
    if let Some(p) = candidate {
        if let Ok(img) = image::open(p) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba.into_raw(), w, h) {
                return icon;
            }
        }
    }
    // Fallback: 32x32 dark square
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..size * size {
        rgba.extend_from_slice(&[0x1F, 0x1F, 0x1F, 0xFF]);
    }
    tray_icon::Icon::from_rgba(rgba, size, size)
        .unwrap_or_else(|_| tray_icon::Icon::from_rgba(vec![0, 0, 0, 0], 1, 1).expect("failed to create the tray icon"))
}

/// System tray action
enum TrayAction {
    /// Show/focus the main window
    Show,
    /// Quit the application
    Quit,
}

/// Create the system tray icon (enabled when tray_icon is explicitly configured)
///
/// The context menu includes "Show window / Quit"; a left click restores the window.
fn setup_tray(config: &NefuConfig) -> Option<tray_icon::TrayIcon> {
    if config.tray_icon.is_none() {
        return None;
    }

    use tray_icon::menu::{Menu, MenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let show_item = MenuItem::with_id("show", "Show window", true, None);
    let quit_item = MenuItem::with_id("quit", "Quit", true, None);
    if menu.append_items(&[&show_item, &quit_item]).is_err() {
        warn!("failed to create the tray menu");
        return None;
    }

    let icon = load_tray_icon(config);

    match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(config.get_title())
        .with_icon(icon)
        .build()
    {
        Ok(tray) => {
            info!("system tray enabled: {}", config.get_title());
            Some(tray)
        }
        Err(e) => {
            warn!("failed to enable the system tray: {}", e);
            None
        }
    }
}

/// Poll tray and menu events, and call the callback to perform actions
fn poll_tray_events(_window: &Window, on_action: &mut dyn FnMut(TrayAction)) {
    // Tray icon click event
    while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
        if let tray_icon::TrayIconEvent::Click {
            button,
            button_state,
            ..
        } = event
        {
            if button == tray_icon::MouseButton::Left
                && button_state == tray_icon::MouseButtonState::Up
            {
                on_action(TrayAction::Show);
            }
        }
    }

    // Tray context menu event
    while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
        match event.id.0.as_str() {
            "show" => on_action(TrayAction::Show),
            "quit" => on_action(TrayAction::Quit),
            _ => {}
        }
    }
}

/// Generate the initialization script that disables the context menu
fn disable_context_menu_script() -> String {
    r#"(function() {
    'use strict';
    document.addEventListener('contextmenu', function(e) {
        e.preventDefault();
    }, true);
})();"#
    .to_string()
}

/// Extract the resource path from a URI
///
/// # Arguments
/// - `uri`: Full URI string
/// - `protocol`: Protocol name
///
/// # Returns
/// Extracted resource relative path
fn extract_path_from_uri(uri: &str, protocol: &str) -> String {
    let prefix = format!("{}://", protocol);

    if let Some(rest) = uri.strip_prefix(&prefix) {
        // Remove the localhost prefix
        let path = if let Some(after_host) = rest.strip_prefix("localhost") {
            after_host.trim_start_matches('/')
        } else {
            rest.trim_start_matches('/')
        };

        // Remove the query string and fragment
        let path = path.split('?').next().unwrap_or(path);
        let path = path.split('#').next().unwrap_or(path);

        if path.is_empty() {
            "index.html".to_string()
        } else {
            path.to_string()
        }
    } else {
        // Non-custom-protocol URI; try to extract the path directly
        uri.trim_start_matches('/').to_string()
    }
}

/// Guess the MIME type of a file
///
/// # Arguments
/// - `path`: File path
///
/// # Returns
/// MIME type string
fn guess_mime_type(path: &str) -> &'static str {
    // Quick lookup of common types
    match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "wasm" => "application/wasm",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Generate the 404 error page
///
/// # Arguments
/// - `path`: Requested resource path
fn generate_404_page(path: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>404 - Resource not found</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: #f8f9fa;
            color: #495057;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
        }}
        h1 {{
            font-size: 6rem;
            margin: 0;
            color: #dee2e6;
            line-height: 1;
        }}
        h2 {{
            font-size: 1.5rem;
            margin: 1rem 0;
        }}
        p {{
            color: #6c757d;
        }}
        code {{
            background: #e9ecef;
            padding: 0.2em 0.5em;
            border-radius: 4px;
            font-size: 0.9em;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>404</h1>
        <h2>Resource not found</h2>
        <p>The requested resource <code>{}</code> does not exist</p>
    </div>
</body>
</html>"#,
        html_escape(path)
    )
}

/// HTML-escape special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Generate the JS-Rust bridge script
///
/// It provides the following global APIs:
/// - `lj(data)` - Send data to Rust
/// - `nefu.invoke(method, args)` - Call a Rust method
/// - `nefu.on(event, callback)` - Listen for Rust events
/// - `nefu.send(data)` - Send data to Rust
/// - `nefu.shell.*` - System shell operations
/// - `nefu.notification.*` - Desktop notifications
/// - `nefu.system.*` - System info
/// - `nefu.powerMonitor.*` - Power monitoring
/// - `nefu.updater.*` - Auto updates
/// - `nefu.crashReporter.*` - Crash reports
/// - `nefu.contextBridge.*` - Context bridge
/// - `nefu.menu.*` - Native menu
fn generate_bridge_script() -> String {
    r#"(function() {
    'use strict';

    // Nefu bridge object
    const nefu = {
        // Event listener registry
        _listeners: {},

        // Pending callbacks
        _pendingCallbacks: {},

        // Call counter
        _callId: 0,

        /**
         * Call a Rust-side method
         * @param {string} method - Method name
         * @param {*} args - Args
         * @returns {Promise<*>} Return value
         */
        invoke: function(method, ...args) {
            const id = ++this._callId;
            return new Promise((resolve, reject) => {
                this._pendingCallbacks[id] = { resolve, reject };
                const message = JSON.stringify({
                    type: 'invoke',
                    id: id,
                    method: method,
                    args: args
                });
                postMessage(message);
            });
        },

        /**
         * Send data to the Rust side
         * @param {*} data - Data to send
         */
        send: function(data) {
            const message = JSON.stringify({
                type: 'send',
                data: data
            });
            postMessage(message);
        },

        /**
         * Register an event listener
         * @param {string} event - Event name
         * @param {Function} callback - Callback function
         * @returns {Function} Function that cancels listening
         */
        on: function(event, callback) {
            if (!this._listeners[event]) {
                this._listeners[event] = [];
            }
            this._listeners[event].push(callback);

            // Return the unsubscribe function
            return () => {
                const idx = this._listeners[event].indexOf(callback);
                if (idx !== -1) {
                    this._listeners[event].splice(idx, 1);
                }
            };
        },

        /**
         * Trigger an event (called by the Rust side)
         * @param {string} event - Event name
         * @param {*} data - Event data
         */
        emit: function(event, data) {
            const listeners = this._listeners[event] || [];
            listeners.forEach(cb => {
                try {
                    cb(data);
                } catch (e) {
                    console.error(`[Nefu] event handling error (${event}):`, e);
                }
            });
        },

        /**
         * Handle a response from the Rust side
         * @param {string} responseJson - Response in JSON format
         */
        _handleResponse: function(responseJson) {
            try {
                const response = JSON.parse(responseJson);
                if (response.type === 'callback' && response.id) {
                    const pending = this._pendingCallbacks[response.id];
                    if (pending) {
                        delete this._pendingCallbacks[response.id];
                        if (response.error) {
                            pending.reject(new Error(response.error));
                        } else {
                            pending.resolve(response.result);
                        }
                    }
                } else if (response.type === 'event') {
                    this.emit(response.event, response.data);
                }
            } catch (e) {
                console.error('[Nefu] response handling error:', e);
            }
        },

        /**
         * Get application info
         * @returns {Object} Application metadata
         */
        getInfo: function() {
            return {
                version: '1.0.0',
                platform: navigator.platform,
                userAgent: navigator.userAgent
            };
        },

        // ==================== Electron-level API namespace ====================

        // System shell operations (similar to the Electron shell module)
        shell: {
            openExternal: (url) => nefu.invoke('shell.openExternal', url),
            openPath: (path) => nefu.invoke('shell.openPath', path),
            showItemInFolder: (path) => nefu.invoke('shell.showItemInFolder', path),
            beep: () => nefu.invoke('shell.beep'),
            trashItem: (path) => nefu.invoke('shell.trashItem', path),
            getSystemVersion: () => nefu.invoke('shell.getSystemVersion'),
        },

        // Desktop notifications (similar to the Electron Notification API)
        notification: {
            send: (options) => nefu.invoke('notification.send', options),
            isSupported: () => nefu.invoke('notification.isSupported'),
        },

        // System info (similar to Electron systemPreferences + screen)
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

        // Power monitoring (similar to Electron powerMonitor)
        powerMonitor: {
            getSystemIdleState: (threshold) => nefu.invoke('powerMonitor.getSystemIdleState', threshold),
        },

        // Auto updates (similar to Electron autoUpdater)
        updater: {
            checkForUpdates: (url) => nefu.invoke('updater.checkForUpdates', url),
        },

        // Crash reports (similar to Electron crashReporter)
        crashReporter: {
            generateTestReport: () => nefu.invoke('crashReporter.generateTestReport'),
        },

        // Context bridge (similar to Electron contextBridge)
        contextBridge: {
            isAvailable: () => nefu.invoke('contextBridge.isAvailable'),
        },

        // Native menu (similar to Electron Menu)
        menu: {
            getDefaultTemplate: (appName) => nefu.invoke('menu.getDefaultTemplate', appName),
        },

        // Window control (similar to Electron BrowserWindow)
        window: {
            minimize: () => nefu.invoke('window.minimize'),
            maximize: () => nefu.invoke('window.maximize'),
            restore: () => nefu.invoke('window.restore'),
            close: () => nefu.invoke('window.close'),
            isMaximized: () => nefu.invoke('window.isMaximized'),
            isMinimized: () => nefu.invoke('window.isMinimized'),
            isVisible: () => nefu.invoke('window.isVisible'),
        },

        // Screenshots (similar to Electron desktopCapturer)
        captureScreenshot: () => nefu.invoke('captureScreenshot'),

        // Printing
        print: () => nefu.invoke('print'),
        printToPDF: () => nefu.invoke('printToPDF'),
    };

    // Send a message to Rust (wry injects window.ipc.postMessage after registering the ipc_handler)
    function postMessage(message) {
        if (window.ipc && window.ipc.postMessage) {
            window.ipc.postMessage(message);
        } else {
            console.warn('[Nefu] IPC channel not ready', message);
        }
    }

    // lj() shortcut function - send data to Rust
    window.lj = function(data) {
        nefu.send(data);
    };

    // Mount to global
    window.nefu = nefu;

    // Trigger a custom event to notify the page that the bridge is ready
    document.addEventListener('DOMContentLoaded', function() {
        window.dispatchEvent(new CustomEvent('nefu-ready'));
    });

    console.log('[Nefu] bridge initialized, Electron-compatible APIs loaded');
})();"#.to_string()
}

/// Save the current window state to a file
fn save_current_window_state(window: &Window, state_path: &Path) {
    let inner_size = window.inner_size();
    let position = window.outer_position().unwrap_or_default();

    let state = WindowState {
        x: position.x,
        y: position.y,
        width: inner_size.width,
        height: inner_size.height,
        maximized: window.is_maximized(),
    };

    if let Err(e) = state.save(state_path) {
        warn!("failed to save window state: {}", e);
    } else {
        info!("window state saved");
    }
}

/// Check whether a GUI application can be started
///
/// In some headless environments a window may not be creatable
pub fn can_create_gui() -> bool {
    // Basic check: ensure it is not a display-less environment such as an SSH session
    #[cfg(target_os = "linux")]
    {
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_path_from_uri() {
        assert_eq!(
            extract_path_from_uri("nefu://localhost/index.html", "nefu"),
            "index.html"
        );
        assert_eq!(
            extract_path_from_uri("nefu://localhost/css/style.css", "nefu"),
            "css/style.css"
        );
        assert_eq!(
            extract_path_from_uri("nefu://localhost/", "nefu"),
            "index.html"
        );
        assert_eq!(
            extract_path_from_uri("nefu://localhost/page.html?q=1#top", "nefu"),
            "page.html"
        );
    }

    #[test]
    fn test_guess_mime_type() {
        assert_eq!(guess_mime_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(guess_mime_type("style.css"), "text/css; charset=utf-8");
        assert_eq!(
            guess_mime_type("app.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(guess_mime_type("logo.png"), "image/png");
        assert_eq!(guess_mime_type("unknown.xyz"), "application/octet-stream");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"hello\""), "&quot;hello&quot;");
    }

    #[test]
    fn test_window_state_default() {
        let state = WindowState::default();
        assert_eq!(state.width, 1024);
        assert_eq!(state.height, 768);
        assert!(!state.maximized);
    }

    #[test]
    fn test_generate_bridge_script_not_empty() {
        let script = generate_bridge_script();
        assert!(script.contains("nefu"));
        assert!(script.contains("invoke"));
        assert!(script.contains("emit"));
        assert!(script.contains("lj"));
    }

    #[test]
    fn test_generate_404_page() {
        let page = generate_404_page("missing.html");
        assert!(page.contains("404"));
        assert!(page.contains("missing.html"));
        assert!(page.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_dirs_or_default() {
        let dir = dirs_or_default();
        assert!(!dir.as_os_str().is_empty());
    }
}
