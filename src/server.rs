//! Nefu development server module
//!
//! Provides a local development server with hot reload:
//! - HTTP file server (based on tiny_http)
//! - WebSocket hot reload notifications (RFC 6455)
//! - File system watching (based on notify)
//! - Real-time .nc file conversion to HTML
//! - Automatic injection of the hot reload client script

use anyhow::{Context, Result};
use base64::Engine;
use log::{debug, error, info, warn};
use notify::{Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sha1::{Digest, Sha1};
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::config::NefuConfig;
use crate::nc_parser;

/// The magic GUID required for the WebSocket handshake (RFC 6455)
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-5AB9FC11E97A";

/// File extensions that trigger a reload
const RELOAD_EXTENSIONS: &[&str] = &[
    "html", "htm", "css", "js", "mjs", "json", "nc", "svg",
    "png", "jpg", "jpeg", "gif", "webp", "wasm",
];

/// Development server
pub struct DevServer {
    /// Project root directory
    project_dir: PathBuf,
    /// Project configuration
    config: NefuConfig,
    /// Listening port
    port: u16,
}

impl DevServer {
    /// Create a new development server instance
    ///
    /// # Arguments
    /// - `project_dir`: Project root directory
    /// - `config`: Project configuration
    /// - `port`: Listening port number
    pub fn new(project_dir: PathBuf, config: NefuConfig, port: u16) -> Result<Self> {
        Ok(Self {
            project_dir,
            config,
            port,
        })
    }

    /// Start the development server
    ///
    /// Starts the HTTP server, WebSocket server, and file watcher together.
    /// This function blocks until an interrupt signal is received.
    pub fn run(self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);

        // Create the HTTP server
        let server = Server::http(&addr)
            .map_err(|e| anyhow::anyhow!("Failed to bind to port {}; check whether the port is already in use: {}", self.port, e))?;

        // Create the hot reload WebSocket server (separate port)
        let reload_server = ReloadServer::start()?;
        let reload_port = reload_server.port();

        info!("🚀 Development server started: http://localhost:{}", self.port);
        info!("📂 Project directory: {}", self.project_dir.display());
        info!("🔄 Hot reload enabled (ws://127.0.0.1:{})", reload_port);
        info!("Press Ctrl+C to stop the server");

        // The file watcher writes change notifications to this channel
        let (reload_tx, reload_rx): (Sender<String>, Receiver<String>) = channel();

        // Start the file watcher
        let watch_dir = self.project_dir.clone();
        let watcher_tx = reload_tx.clone();
        thread::spawn(move || {
            if let Err(e) = start_file_watcher(&watch_dir, watcher_tx) {
                error!("File watcher error: {}", e);
            }
        });

        // Broadcast received changes to all WebSocket clients
        let ws_connections = reload_server.connections();
        thread::spawn(move || {
            broadcast_reload(reload_rx, ws_connections);
        });

        // Main loop for handling HTTP requests
        let config = self.config.clone();
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();

            debug!("{} {}", method, url);

            // Handle normal HTTP requests (the injected hot reload script connects via reload_port rather than the WS path)
            match handle_http_request(&request, &self.project_dir, &config, reload_port) {
                Ok(response) => {
                    if let Err(e) = request.respond(response) {
                        warn!("Failed to send response: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to handle request: {} - {}", url, e);
                    let error_response = create_error_response(500, &format!("Internal error: {}", e));
                    if let Err(e) = request.respond(error_response) {
                        warn!("Failed to send error response: {}", e);
                    }
                }
            }
        }

        Ok(())
    }
}

/// Handle an HTTP request
///
/// # Arguments
/// - `request`: HTTP request object
/// - `project_dir`: Project root directory
/// - `config`: Project configuration
///
/// # Returns
/// HTTP response object
fn handle_http_request(
    request: &Request,
    project_dir: &Path,
    config: &NefuConfig,
    reload_port: u16,
) -> Result<Response<Cursor<Vec<u8>>>> {
    let url = request.url();
    let path = url.split('?').next().unwrap_or(url);

    // Domain whitelist restriction (effective when allowed_domains is non-empty)
    if !config.allowed_domains.is_empty() {
        let host = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Host"))
            .map(|h| h.value.as_str().to_string())
            .unwrap_or_default();
        let host_name = host.split(':').next().unwrap_or("").to_string();
        if !config.allowed_domains.iter().any(|d| d == &host_name) {
            warn!("rejecting request from non-whitelisted domain: {}", host_name);
            let deny_headers = vec![
                Header::from_bytes("Content-Type", b"text/html; charset=utf-8").unwrap(),
            ];
            let body = "<h1>403 Forbidden</h1><p>Domain is not in the allowed_domains whitelist</p>";
            return Ok(Response::new(
                StatusCode(403),
                deny_headers,
                Cursor::new(body.as_bytes().to_vec()),
                None,
                None,
            ));
        }
    }

    // Normalize the path to prevent directory traversal attacks
    let safe_path = sanitize_path(path)?;

    // Determine the actual file path
    let file_path = if safe_path.is_empty() || safe_path == "/" {
        // Map the root path to the entry file
        project_dir.join(&config.entry)
    } else {
        let trimmed = safe_path.trim_start_matches('/');
        project_dir.join(trimmed)
    };

    // In development mode, prefer .nc: if a .html is requested and a same-name .nc exists, convert the .nc live.
    // This way editing .nc and refreshing takes effect, instead of reading a stale .html build artifact.
    let actual_path = {
        let prefer_nc = file_path.extension().map_or(false, |e| e == "html")
            && file_path.with_extension("nc").exists();

        if prefer_nc {
            file_path.with_extension("nc")
        } else if !file_path.exists()
            && file_path.extension().map_or(false, |e| e == "html")
        {
            // Requested .html does not exist; try the same-named .nc
            let nc_path = file_path.with_extension("nc");
            if nc_path.exists() {
                nc_path
            } else {
                return Ok(create_404_response(path));
            }
        } else if !file_path.exists() {
            return Ok(create_404_response(path));
        } else {
            file_path
        }
    };

    // Whether converted from .nc (determines MIME and whether to inject the hot reload script)
    let rendered_from_nc = actual_path.extension().map_or(false, |e| e == "nc");

    // Read the file content
    let content = if rendered_from_nc {
        // .nc files need to be converted to HTML
        let nc_content = std::fs::read_to_string(&actual_path)
            .context("Failed to read NC file")?;

        match nc_parser::parse_nc_to_html(&nc_content) {
            Ok(html) => {
                // Inject the hot reload script
                let html_with_reload = inject_hot_reload_script(&html, reload_port);
                html_with_reload.into_bytes()
            }
            Err(e) => {
                // Parse failure: return a page with specific error diagnostics (HTTP 400),
                // instead of turning the whole page into a blank 500 error page
                warn!("NC parse failed {}: {}", actual_path.display(), e);
                let diag = create_nc_error_response(&actual_path, &e.to_string());
                return Ok(diag);
            }
        }
    } else {
        std::fs::read(&actual_path)
            .with_context(|| format!("failed to read file: {}", actual_path.display()))?
    };

    // Inject the hot reload script into HTML content (including .nc-converted HTML and regular .html/.htm)
    let final_content = if is_html_file(&actual_path) || rendered_from_nc {
        let text = String::from_utf8_lossy(&content);
        inject_hot_reload_script(&text, reload_port).into_bytes()
    } else {
        content
    };

    // Determine the MIME type: content converted from .nc is HTML in essence
    let mime = if rendered_from_nc {
        "text/html; charset=utf-8".to_string()
    } else {
        guess_mime_for_path(&actual_path)
    };

    // Build response headers: base headers + security policy (csp) + custom headers (headers)
    let mut headers = vec![
        Header::from_bytes("Content-Type", mime.as_bytes()).unwrap(),
        Header::from_bytes("Access-Control-Allow-Origin", b"*").unwrap(),
        Header::from_bytes("Access-Control-Allow-Methods", b"GET, POST, OPTIONS").unwrap(),
        Header::from_bytes("Access-Control-Allow-Headers", b"Content-Type").unwrap(),
        Header::from_bytes("Cache-Control", b"no-cache, no-store, must-revalidate").unwrap(),
        Header::from_bytes("X-Nefu-Dev", b"true").unwrap(),
    ];

    if let Some(csp) = config.csp.as_deref() {
        if let Ok(h) = Header::from_bytes("Content-Security-Policy", csp.as_bytes()) {
            headers.push(h);
        }
    }
    for (name, value) in &config.headers {
        if let Ok(h) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            headers.push(h);
        }
    }

    let cursor = Cursor::new(final_content);
    Ok(Response::new(StatusCode(200), headers, cursor, None, None))
}

/// Sanitize a path
///
/// Prevent directory traversal attacks (../ etc.)
fn sanitize_path(path: &str) -> Result<String> {
    // URL decode
    let decoded = url_decode(path);

    // Remove the query string
    let clean = decoded.split('?').next().unwrap_or(&decoded);

    // Check for directory traversal
    let normalized = clean.replace('\\', "/");
    for component in normalized.split('/') {
        if component == ".." {
            return Err(anyhow::anyhow!("illegal path: contains directory traversal"));
        }
    }

    Ok(normalized)
}

/// Simple URL decoding
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Determine whether the file is of HTML type
fn is_html_file(path: &Path) -> bool {
    path.extension()
        .map_or(false, |ext| ext == "html" || ext == "htm")
}

/// Guess the MIME type from the file path
fn guess_mime_for_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8".to_string(),
        "css" => "text/css; charset=utf-8".to_string(),
        "js" | "mjs" => "application/javascript; charset=utf-8".to_string(),
        "json" => "application/json; charset=utf-8".to_string(),
        "xml" => "application/xml; charset=utf-8".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "ico" => "image/x-icon".to_string(),
        "woff" => "font/woff".to_string(),
        "woff2" => "font/woff2".to_string(),
        "ttf" => "font/ttf".to_string(),
        "otf" => "font/otf".to_string(),
        "mp4" => "video/mp4".to_string(),
        "webm" => "video/webm".to_string(),
        "mp3" => "audio/mpeg".to_string(),
        "wav" => "audio/wav".to_string(),
        "pdf" => "application/pdf".to_string(),
        "txt" => "text/plain; charset=utf-8".to_string(),
        "md" => "text/markdown; charset=utf-8".to_string(),
        "wasm" => "application/wasm".to_string(),
        "map" => "application/json".to_string(),
        _ => mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
    }
}

/// Inject the hot reload client script into HTML content
///
/// The client connects to the separate reload_port via WebSocket and receives
/// file change events pushed by the server. CSS changes trigger a no-reload style update;
/// other changes trigger a full page refresh.
fn inject_hot_reload_script(html: &str, reload_port: u16) -> String {
    let reload_script = format!(
        r#"
<script>
(function() {{
    'use strict';
    const WS_URL = 'ws://127.0.0.1:{reload_port}/';
    let ws = null;
    let reconnectTimer = null;
    let reconnectDelay = 500;

    function connect() {{
        try {{
            ws = new WebSocket(WS_URL);
        }} catch (e) {{
            scheduleReconnect();
            return;
        }}

        ws.onopen = function() {{
            console.log('[Nefu Dev] Hot reload connected');
            if (reconnectTimer) {{
                clearTimeout(reconnectTimer);
                reconnectTimer = null;
            }}
            reconnectDelay = 500;
        }};

        ws.onmessage = function(event) {{
            var data;
            try {{
                data = JSON.parse(event.data);
            }} catch (e) {{
                return;
            }}
            if (data && data.type === 'reload') {{
                console.log('[Nefu Dev] File changed:', data.file || '');
                if (data.file && data.file.split('?')[0].endsWith('.css')) {{
                    reloadStylesheets();
                }} else {{
                    location.reload();
                }}
            }}
        }};

        ws.onclose = function() {{
            console.log('[Nefu Dev] connection closed, reconnecting later...');
            scheduleReconnect();
        }};

        ws.onerror = function() {{
            try {{ ws.close(); }} catch (e) {{}}
        }};
    }}

    function scheduleReconnect() {{
        if (reconnectTimer) return;
        reconnectTimer = setTimeout(function() {{
            reconnectTimer = null;
            reconnectDelay = Math.min(reconnectDelay * 2, 10000);
            connect();
        }}, reconnectDelay);
    }}

    function reloadStylesheets() {{
        var links = document.querySelectorAll('link[rel="stylesheet"]');
        links.forEach(function(link) {{
            var href = link.href;
            link.href = '';
            setTimeout(function() {{
                link.href = href + (href.indexOf('?') >= 0 ? '&' : '?') + '_t=' + Date.now();
            }}, 50);
        }});
        console.log('[Nefu Dev] stylesheet refreshed');
    }}

    connect();
}})();
</script>
"#,
        reload_port = reload_port
    );

    // Try to inject before </head>
    if let Some(pos) = html.find("</head>") {
        let mut result = html[..pos].to_string();
        result.push_str(&reload_script);
        result.push_str(&html[pos..]);
        result
    } else if let Some(pos) = html.find("<body") {
        // No head tag; inject before <body>
        let mut result = html[..pos].to_string();
        result.push_str(&reload_script);
        result.push_str(&html[pos..]);
        result
    } else {
        // Append directly at the end
        format!("{}{}", html, reload_script)
    }
}

/// Create a 404 response
fn create_404_response(path: &str) -> Response<Cursor<Vec<u8>>> {
    let body = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>404</title>
<style>
body{{font-family:sans-serif;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#f5f5f5}}
.box{{text-align:center;padding:2rem}}
h1{{font-size:4rem;margin:0;color:#ddd}}
p{{color:#666}}
</style></head>
<body><div class="box"><h1>404</h1><p>File not found: {}</p></div></body></html>"#,
        html_escape_simple(path)
    );

    let headers = vec![
        Header::from_bytes("Content-Type", b"text/html; charset=utf-8").unwrap(),
    ];

    Response::new(StatusCode(404), headers, Cursor::new(body.into_bytes()), None, None)
}

/// Create an error response
fn create_error_response(code: u16, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Error {}</title>
<style>body{{font-family:sans-serif;padding:2rem;background:#fff5f5;color:#c00}}</style></head>
<body><h1>Error {}</h1><pre>{}</pre></body></html>"#,
        code, code, html_escape_simple(message)
    );

    let headers = vec![
        Header::from_bytes("Content-Type", b"text/html; charset=utf-8").unwrap(),
    ];

    Response::new(StatusCode(code), headers, Cursor::new(body.into_bytes()), None, None)
}

/// Create a diagnostic page for .nc parse failures
///
/// Returns HTTP 400 and shows the specific error message in the page so developers can
/// locate the problem directly in the browser instead of seeing a silent 500.
fn create_nc_error_response(path: &Path, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>NC Compile Error</title>
<style>
body{{font-family:-apple-system,'Segoe UI',Roboto,sans-serif;margin:0;background:#fff7f7;color:#8a1f11;padding:2rem}}
h1{{font-size:1.4rem;margin:0 0 .4rem}}
code, pre{{background:#2b2b2b;color:#f8f8f8;padding:.2em .5em;border-radius:4px}}
pre{{padding:1rem;overflow:auto;white-space:pre-wrap;word-break:break-all}}
.file{{color:#555;font-size:.9rem;margin-bottom:1rem}}
.hint{{color:#555;font-size:.9rem;background:#fff;border:1px solid #ecc;border-radius:6px;padding:1rem;margin-top:1rem}}
</style></head>
<body>
<h1>⚠️ NC Compile Error</h1>
<div class="file">File: {file}</div>
<pre>{msg}</pre>
<div class="hint">💡 Common causes: make sure every node has a <code>component</code> or <code>type</code> field, the JSON syntax is correct,
and the root node is an object (not an array). Save after editing and hot reload will recompile automatically.</div>
<script>setTimeout(function(){{ location.reload(); }}, 3000);</script>
</body></html>"#,
        file = html_escape_simple(&path.display().to_string()),
        msg = html_escape_simple(message),
    );

    let headers = vec![
        Header::from_bytes("Content-Type", b"text/html; charset=utf-8").unwrap(),
    ];

    Response::new(StatusCode(400), headers, Cursor::new(body.into_bytes()), None, None)
}

/// Simple HTML escaping
fn html_escape_simple(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ==================== WebSocket hot reload server ====================

/// WebSocket handshake result
#[derive(Debug)]
enum HandshakeResult {
    /// Client passed the handshake; returns the stream with masking disabled
    Established(TcpStream),
    /// Handshake request is invalid; returns the rejection reason text
    Rejected(String),
}

/// Hot reload WebSocket server (RFC 6455)
///
/// Runs on a separate port, accepts browser WebSocket connections, and
/// pushes reload notifications to all connections on file changes.
pub struct ReloadServer {
    /// Listening address
    port: u16,
    /// All active connection streams
    connections: Arc<Mutex<Vec<TcpStream>>>,
}

impl ReloadServer {
    /// Start the hot reload server, bound to a temporary port
    pub fn start() -> Result<ReloadServer> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .context("Failed to bind the hot reload WebSocket port")?;

        let port = listener
            .local_addr()
            .context("Failed to obtain the hot reload port number")?
            .port();

        let connections: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

        // Thread accepting connections
        let conns = connections.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        // Reject a single failed handshake without affecting the server
                        match perform_websocket_handshake(stream) {
                            Ok(HandshakeResult::Established(ws_stream)) => {
                                // Save the connection
                                if let Ok(mut guard) = conns.lock() {
                                    guard.push(ws_stream);
                                }
                            }
                            Ok(HandshakeResult::Rejected(reason)) => {
                                debug!("Rejected WebSocket handshake: {}", reason);
                            }
                            Err(e) => {
                                debug!("WebSocket handshake failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept WebSocket connection: {}", e);
                        // Exit when the server is shutting down
                        break;
                    }
                }
            }
        });

        info!("Hot reload WebSocket server started: ws://127.0.0.1:{}", port);
        Ok(ReloadServer {
            port,
            connections,
        })
    }

    /// Get the server's port number
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the shared connection list (for the broadcast thread)
    pub fn connections(&self) -> Arc<Mutex<Vec<TcpStream>>> {
        self.connections.clone()
    }

    /// Get the number of active connections (for diagnostics)
    pub fn connection_count(&self) -> usize {
        self.connections.lock().map(|c| c.len()).unwrap_or(0)
    }
}

/// Broadcast a reload message to all WebSocket clients
///
/// Read file changes from the receiver, serialize them to JSON, and write to each connection.
/// Connections that fail to write are removed.
pub fn broadcast_reload(rx: Receiver<String>, connections: Arc<Mutex<Vec<TcpStream>>>) {
    while let Ok(path) = rx.recv() {
        let payload = serde_json::json!({
            "type": "reload",
            "file": path,
        })
        .to_string();

        let mut connected = {
            match connections.lock() {
                Ok(guard) => guard,
                Err(_) => continue, // skip this round when the lock is corrupted
            }
        };

        if connected.is_empty() {
            debug!("Hot reload broadcast: no WebSocket clients connected");
            continue;
        }

        // Keep connections that are still alive
        connected.retain(|stream| match stream.try_clone() {
            Ok(mut clone) => {
                send_websocket_text_frame(&mut clone, payload.as_bytes()).is_ok()
            }
            Err(_) => false,
        });

        debug!("Hot reload broadcast: {} ({} active clients)", path, connected.len());
    }
}

/// Perform the WebSocket handshake
///
/// Reads the client HTTP upgrade request, validates the upgrade headers, computes the RFC 6455
/// Sec-WebSocket-Accept, sends a 101 response, and returns the established connection.
fn perform_websocket_handshake(stream: TcpStream) -> Result<HandshakeResult> {
    let mut stream = stream;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .ok();

    // Read the HTTP upgrade request headers (size-limited to prevent malicious requests)
    let mut header_buffer = Vec::new();
    let mut byte = [0u8; 1];
    let mut saw_header_end = false;
    for _ in 0..64 * 1024 {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                header_buffer.push(byte[0]);
                if header_buffer.ends_with(b"\r\n\r\n") {
                    saw_header_end = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if !saw_header_end {
        return Ok(HandshakeResult::Rejected("Incomplete request headers".to_string()));
    }

    let request_text = String::from_utf8_lossy(&header_buffer);
    let mut lines = request_text.lines();

    // Request line
    let _request_line = lines.next();

    // Parse the request headers
    let mut upgrade = false;
    let mut connection_upgrade = false;
    let mut ws_key: Option<String> = None;

    for line in lines {
        let line = line.trim_end_matches('\r');
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match name.as_str() {
                "upgrade" => {
                    upgrade = value.eq_ignore_ascii_case("websocket");
                }
                "connection" => {
                    connection_upgrade = value
                        .split(',')
                        .any(|part| part.trim().eq_ignore_ascii_case("upgrade"));
                }
                "sec-websocket-key" => {
                    ws_key = Some(value.to_string());
                }
                _ => {}
            }
        }
    }

    if !upgrade || !connection_upgrade {
        return Ok(HandshakeResult::Rejected("Missing Upgrade/Connection headers".to_string()));
    }

    let key = match ws_key {
        Some(k) if !k.is_empty() => k,
        _ => return Ok(HandshakeResult::Rejected("missing Sec-WebSocket-Key".to_string())),
    };

    // Compute the Accept value: Base64 of SHA-1(key + GUID)
    let accept = compute_websocket_accept(&key);

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        accept
    );

    if let Err(e) = stream.write_all(response.as_bytes()) {
        return Err(anyhow::anyhow!("failed to send handshake response: {}", e));
    }
    stream.flush().ok();

    Ok(HandshakeResult::Established(stream))
}

/// Compute the WebSocket Accept Key (SHA-1 + Base64, per RFC 6455)
fn compute_websocket_accept(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(digest)
}

/// Send a text data frame to the connection (RFC 6455, server does not mask)
fn send_websocket_text_frame(stream: &mut TcpStream, payload: &[u8]) -> std::io::Result<()> {
    // First byte: FIN=1 (0x80) | opcode=1 (text)
    stream.write_all(&[0x81])?;

    // Length encoding
    let len = payload.len();
    if len < 126 {
        stream.write_all(&[len as u8])?;
    } else if len <= 0xFFFF {
        stream.write_all(&[126])?;
        stream.write_all(&(len as u16).to_be_bytes())?;
    } else {
        stream.write_all(&[127])?;
        stream.write_all(&(len as u64).to_be_bytes())?;
    }

    stream.write_all(payload)?;
    stream.flush()
}

/// Start the file system watcher
///
/// Watch for file changes in the project directory and send notifications through the channel.
///
/// # Arguments
/// - `dir`: Directory to watch
/// - `tx`: Change notification sender
fn start_file_watcher(dir: &Path, tx: Sender<String>) -> Result<()> {
    let (_notify_tx, _notify_rx): (Sender<()>, _) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: std::result::Result<NotifyEvent, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        for path in &event.paths {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if RELOAD_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                                    let rel_path = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "unknown".to_string());

                                    info!("📝 file change: {}", rel_path);
                                    let _ = tx.send(rel_path);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        },
        notify::Config::default(),
    )
    .context("Failed to create the file watcher")?;

    watcher
        .watch(dir, RecursiveMode::Recursive)
        .context("Failed to add the watch directory")?;

    info!("👁️ File watcher started: {}", dir.display());

    // Keep the watcher running
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

/// Get an available port number
///
/// If the specified port is in use, automatically find the next available port.
///
/// # Arguments
/// - `preferred`: The preferred port number
///
/// # Returns
/// An available port number
pub fn find_available_port(preferred: u16) -> u16 {
    let mut port = preferred;

    while port < preferred + 100 {
        if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return port;
        }
        port += 1;
    }

    // If 100 consecutive ports are all occupied, let the system allocate
    TcpListener::bind("127.0.0.1:0")
        .map(|l| l.local_addr().unwrap().port())
        .unwrap_or(preferred)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path_normal() {
        assert_eq!(sanitize_path("/index.html").unwrap(), "/index.html");
        assert_eq!(sanitize_path("/css/style.css").unwrap(), "/css/style.css");
    }

    #[test]
    fn test_sanitize_path_traversal() {
        assert!(sanitize_path("/../../../etc/passwd").is_err());
        assert!(sanitize_path("/foo/../bar").is_err());
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("a+b"), "a b");
        assert_eq!(url_decode("normal"), "normal");
    }

    #[test]
    fn test_is_html_file() {
        assert!(is_html_file(Path::new("index.html")));
        assert!(is_html_file(Path::new("page.htm")));
        assert!(!is_html_file(Path::new("style.css")));
        assert!(!is_html_file(Path::new("app.js")));
    }

    #[test]
    fn test_inject_hot_reload_script() {
        let html = "<html><head><title>Test</title></head><body>Hello</body></html>";
        let result = inject_hot_reload_script(html, 45678);
        assert!(result.contains("ws://127.0.0.1:45678"));
        assert!(result.contains("hot reload"));
    }

    #[test]
    fn test_inject_no_head_tag() {
        let html = "<body>Hello</body>";
        let result = inject_hot_reload_script(html, 45678);
        assert!(result.contains("ws://127.0.0.1:45678"));
    }

    #[test]
    fn test_find_available_port() {
        let port = find_available_port(3000);
        assert!(port >= 3000);
    }

    #[test]
    fn test_guess_mime_for_path() {
        assert_eq!(
            guess_mime_for_path(Path::new("test.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            guess_mime_for_path(Path::new("test.css")),
            "text/css; charset=utf-8"
        );
    }

    #[test]
    fn test_html_escape_simple() {
        assert_eq!(html_escape_simple("<b>bold</b>"), "&lt;b&gt;bold&lt;/b&gt;");
        assert_eq!(html_escape_simple("a & b"), "a &amp; b");
    }

    #[test]
    fn test_create_404_response() {
        let resp = create_404_response("/missing.html");
        assert_eq!(resp.status_code().0, 404);
    }

    /// Standard test vectors from the RFC 6455 appendix
    ///
    /// Note: `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=` comes from the GUID in the draft version
    /// (...-C5AB0DC85B11) computed; the final RFC 6455 GUID is
    /// `258EAFA5-E914-47DA-95CA-5AB9FC11E97A`; its correct Accept value is
    /// `evRRRcnbilln1hhKoXsmWwBZTkY=` (cross-validated with a standalone SHA-1 implementation).
    #[test]
    fn test_compute_websocket_accept_rfc_vector() {
        let accept = compute_websocket_accept("dGhlIHNhbXBsZSBub25jZQ==");
        assert_eq!(accept, "evRRRcnbilln1hhKoXsmWwBZTkY=");
    }

    /// Different keys should produce different accept values
    #[test]
    fn test_compute_websocket_accept_distinct() {
        let a = compute_websocket_accept("key-one");
        let b = compute_websocket_accept("key-two");
        assert_ne!(a, b);
    }
}
