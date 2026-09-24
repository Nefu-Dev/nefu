//! Nefu custom protocol module
//!
//! Implements the nefu:// custom URL protocol so the WebView can load files from
//! the in-memory resource pack without file system access.
//!
//! ## Protocol format
//! ```text
//! nefu://localhost/path/to/resource[?query][#fragment]
//! ```

use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::Path;

use crate::pack::ResourcePack;

/// Default protocol name
pub const DEFAULT_PROTOCOL: &str = "nefu";

/// Default host name
pub const DEFAULT_HOST: &str = "localhost";

/// Protocol request info
#[derive(Debug, Clone)]
pub struct ProtocolRequest {
    /// Full URI
    pub uri: String,
    /// Protocol name
    pub protocol: String,
    /// Host name
    pub host: String,
    /// Resource path
    pub path: String,
    /// Query parameters
    pub query: HashMap<String, String>,
    /// URL fragment
    pub fragment: Option<String>,
    /// HTTP method
    pub method: String,
}

impl ProtocolRequest {
    /// Parse a protocol request from a URI string
    ///
    /// # Arguments
    /// - `uri`: the full URI string
    /// - `protocol`: the expected protocol name
    ///
    /// # Returns
    /// The parsed request object
    pub fn parse(uri: &str, protocol: &str) -> Result<Self> {
        let prefix = format!("{}://", protocol);

        if !uri.starts_with(&prefix) {
            return Err(anyhow::anyhow!(
                "URI '{}' does not start with '{}://'",
                uri,
                protocol
            ));
        }

        let rest = &uri[prefix.len()..];

        // Split the host name and path
        let (host, path_and_query) = if let Some(slash_pos) = rest.find('/') {
            (&rest[..slash_pos], &rest[slash_pos..])
        } else {
            (rest, "/")
        };

        // Split the path, query string, and fragment
        let (path_part, fragment) = if let Some(hash_pos) = path_and_query.find('#') {
            (
                &path_and_query[..hash_pos],
                Some(path_and_query[hash_pos + 1..].to_string()),
            )
        } else {
            (path_and_query, None)
        };

        let (path, query_string) = if let Some(q_pos) = path_part.find('?') {
            (&path_part[..q_pos], &path_part[q_pos + 1..])
        } else {
            (path_part, "")
        };

        // Parse the query parameters
        let query = parse_query_string(query_string);

        Ok(Self {
            uri: uri.to_string(),
            protocol: protocol.to_string(),
            host: host.to_string(),
            path: normalize_path(path),
            query,
            fragment,
            method: "GET".to_string(),
        })
    }

    /// Get the normalized resource path
    ///
    /// Returns the entry file name if the path is empty or "/"
    pub fn resource_path(&self, entry: &str) -> String {
        if self.path.is_empty() || self.path == "/" {
            entry.to_string()
        } else {
            self.path.trim_start_matches('/').to_string()
        }
    }
}

/// Protocol response
#[derive(Debug, Clone)]
pub struct ProtocolResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Vec<u8>,
}

impl ProtocolResponse {
    /// Create a success response
    pub fn ok(body: Vec<u8>, content_type: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), content_type.to_string());
        headers.insert(
            "Access-Control-Allow-Origin".to_string(),
            "*".to_string(),
        );
        headers.insert(
            "Cache-Control".to_string(),
            "no-cache".to_string(),
        );

        Self {
            status: 200,
            headers,
            body,
        }
    }

    /// Create a 404 response
    pub fn not_found(path: &str) -> Self {
        let body = format!(
            r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>404</title></head>
<body style="font-family:sans-serif;text-align:center;padding:3rem">
<h1>404</h1><p>Resource not found: {}</p></body></html>"#,
            html_escape(path)
        );

        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );

        Self {
            status: 404,
            headers,
            body: body.into_bytes(),
        }
    }

    /// Create a 500 error response
    pub fn internal_error(message: &str) -> Self {
        let body = format!(
            r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>500</title></head>
<body style="font-family:sans-serif;text-align:center;padding:3rem;color:#c00">
<h1>500</h1><p>Internal error: {}</p></body></html>"#,
            html_escape(message)
        );

        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "text/html; charset=utf-8".to_string(),
        );

        Self {
            status: 500,
            headers,
            body: body.into_bytes(),
        }
    }

    /// Add a cache-control header
    pub fn with_cache_control(mut self, directive: &str) -> Self {
        self.headers
            .insert("Cache-Control".to_string(), directive.to_string());
        self
    }

    /// Add a custom response header
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }
}

/// Protocol handler
///
/// Manages request routing and resource resolution for the custom protocol
pub struct ProtocolHandler {
    /// Protocol name
    protocol: String,
    /// Entry file name
    entry: String,
    /// Additional MIME type overrides
    mime_overrides: HashMap<String, String>,
    /// Cache control policy
    cache_policy: CachePolicy,
}

/// Cache control policy
#[derive(Debug, Clone)]
pub enum CachePolicy {
    /// Do not cache (dev mode)
    NoCache,
    /// Short-lived cache (with validation)
    ShortLived,
    /// Long-lived cache (for immutable resources)
    Immutable,
    /// Custom directive
    Custom(String),
}

impl CachePolicy {
    /// Convert to a Cache-Control header value
    pub fn to_header_value(&self) -> &'static str {
        match self {
            CachePolicy::NoCache => "no-cache, no-store, must-revalidate",
            CachePolicy::ShortLived => "public, max-age=300",
            CachePolicy::Immutable => "public, max-age=31536000, immutable",
            CachePolicy::Custom(_) => "no-cache",
        }
    }
}

impl ProtocolHandler {
    /// Create a new protocol handler
    ///
    /// # Arguments
    /// - `protocol`: the protocol name
    /// - `entry`: the entry file name
    pub fn new(protocol: &str, entry: &str) -> Self {
        Self {
            protocol: protocol.to_string(),
            entry: entry.to_string(),
            mime_overrides: HashMap::new(),
            cache_policy: CachePolicy::NoCache,
        }
    }

    /// Set the cache policy
    pub fn with_cache_policy(mut self, policy: CachePolicy) -> Self {
        self.cache_policy = policy;
        self
    }

    /// Add a MIME type override
    pub fn with_mime_override(mut self, extension: &str, mime: &str) -> Self {
        self.mime_overrides
            .insert(extension.to_string(), mime.to_string());
        self
    }

    /// Handle a protocol request
    ///
    /// # Arguments
    /// - `uri`: the request URI
    /// - `resource_pack`: the resource pack
    ///
    /// # Returns
    /// The protocol response
    pub fn handle_request(
        &self,
        uri: &str,
        resource_pack: &mut ResourcePack,
    ) -> ProtocolResponse {
        debug!("handling protocol request: {}", uri);

        // Parse the request
        let request = match ProtocolRequest::parse(uri, &self.protocol) {
            Ok(req) => req,
            Err(e) => {
                warn!("failed to parse protocol request: {}", e);
                return ProtocolResponse::internal_error(&format!("invalid request: {}", e));
            }
        };

        // Get the resource path
        let resource_path = request.resource_path(&self.entry);
        debug!("resource path: {}", resource_path);

        // Look up the file in the resource pack
        match resource_pack.get(&resource_path) {
            Some(content) => {
                let mime = self.get_mime_type(&resource_path);
                let response = ProtocolResponse::ok(content.clone(), &mime)
                    .with_cache_control(self.cache_policy.to_header_value());

                debug!(
                    "response: {} ({} bytes, {})",
                    resource_path,
                    content.len(),
                    mime
                );
                response
            }
            None => {
                // Try index.html as a directory index
                let index_path = format!("{}/index.html", resource_path.trim_end_matches('/'));
                if let Some(content) = resource_pack.get(&index_path) {
                    let response = ProtocolResponse::ok(
                        content.clone(),
                        "text/html; charset=utf-8",
                    );
                    debug!("returning directory index: {}", index_path);
                    response
                } else {
                    warn!("resource not found: {}", resource_path);
                    ProtocolResponse::not_found(&resource_path)
                }
            }
        }
    }

    /// Get the MIME type of a file
    fn get_mime_type(&self, path: &str) -> String {
        // Check for custom overrides
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            if let Some(mime) = self.mime_overrides.get(ext) {
                return mime.clone();
            }
        }

        // Use the built-in mapping
        guess_content_type(path)
    }

    /// Get the protocol name
    pub fn protocol_name(&self) -> &str {
        &self.protocol
    }

    /// Build the full protocol URL
    ///
    /// # Arguments
    /// - `path`: the resource path
    pub fn build_url(&self, path: &str) -> String {
        format!("{}://{}/{}", self.protocol, DEFAULT_HOST, path.trim_start_matches('/'))
    }
}

/// Parse a query string into key-value pairs
fn parse_query_string(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if query.is_empty() {
        return params;
    }

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let Some(key) = parts.next() {
            let value = parts.next().unwrap_or("");
            if !key.is_empty() {
                params.insert(
                    url_decode_component(key),
                    url_decode_component(value),
                );
            }
        }
    }

    params
}

/// URL-decode a single component
fn url_decode_component(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();

    while let Some(b) = chars.next() {
        if b == b'%' {
            let h = chars.next().unwrap_or(b'0');
            let l = chars.next().unwrap_or(b'0');
            let hex = format!("{}{}", h as char, l as char);
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push(h as char);
                result.push(l as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }

    result
}

/// Normalize a path
fn normalize_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");

    // Remove leading slashes
    while normalized.starts_with('/') {
        normalized.remove(0);
    }

    // Remove trailing slashes (keep the root path)
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    // Resolve . and ..
    let components: Vec<&str> = normalized.split('/').collect();
    let mut resolved = Vec::new();

    for component in components {
        match component {
            "." | "" => continue,
            ".." => {
                resolved.pop();
            }
            _ => resolved.push(component),
        }
    }

    resolved.join("/")
}

/// Guess the Content-Type from the file extension
fn guess_content_type(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
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
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "ogg" => "video/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "pdf" => "application/pdf",
        "txt" | "text" => "text/plain; charset=utf-8",
        "md" | "markdown" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "wasm" => "application/wasm",
        "map" => "application/json",
        "yaml" | "yml" => "application/x-yaml",
        "toml" => "application/toml",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// HTML escape
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Register the custom protocol with the WebView
///
/// This function provides helper logic for protocol registration; the actual registration happens in the webview module.
///
/// # Arguments
/// - `protocol`: the protocol name
/// - `handler`: the protocol handler reference
pub fn prepare_protocol_registration(protocol: &str, handler: &ProtocolHandler) {
    info!("preparing to register custom protocol: {}://", protocol);
    debug!("entry file: {}", handler.build_url("index.html"));
    debug!("cache policy: {:?}", handler.cache_policy);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_protocol_request() {
        let req = ProtocolRequest::parse("nefu://localhost/index.html", "nefu").unwrap();
        assert_eq!(req.protocol, "nefu");
        assert_eq!(req.host, "localhost");
        assert_eq!(req.path, "index.html");
        assert!(req.query.is_empty());
        assert!(req.fragment.is_none());
    }

    #[test]
    fn test_parse_with_query_and_fragment() {
        let req = ProtocolRequest::parse(
            "nefu://localhost/page.html?key=value&foo=bar#section",
            "nefu",
        )
        .unwrap();
        assert_eq!(req.path, "page.html");
        assert_eq!(req.query.get("key").unwrap(), "value");
        assert_eq!(req.query.get("foo").unwrap(), "bar");
        assert_eq!(req.fragment.unwrap(), "section");
    }

    #[test]
    fn test_parse_invalid_protocol() {
        let result = ProtocolRequest::parse("http://localhost/index.html", "nefu");
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_path() {
        let req = ProtocolRequest::parse("nefu://localhost/", "nefu").unwrap();
        assert_eq!(req.resource_path("index.html"), "index.html");

        let req2 = ProtocolRequest::parse("nefu://localhost/css/style.css", "nefu").unwrap();
        assert_eq!(req2.resource_path("index.html"), "css/style.css");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/index.html"), "index.html");
        assert_eq!(normalize_path("a/b/../c"), "a/c");
        assert_eq!(normalize_path("./foo/./bar"), "foo/bar");
        assert_eq!(normalize_path(""), "");
    }

    #[test]
    fn test_parse_query_string() {
        let params = parse_query_string("a=1&b=2&c=hello%20world");
        assert_eq!(params.get("a").unwrap(), "1");
        assert_eq!(params.get("b").unwrap(), "2");
        assert_eq!(params.get("c").unwrap(), "hello world");
    }

    #[test]
    fn test_parse_empty_query() {
        let params = parse_query_string("");
        assert!(params.is_empty());
    }

    #[test]
    fn test_guess_content_type() {
        assert_eq!(guess_content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(guess_content_type("style.css"), "text/css; charset=utf-8");
        assert_eq!(
            guess_content_type("app.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(guess_content_type("logo.png"), "image/png");
        assert_eq!(guess_content_type("data.unknown"), "application/octet-stream");
    }

    #[test]
    fn test_protocol_handler_build_url() {
        let handler = ProtocolHandler::new("nefu", "index.html");
        assert_eq!(
            handler.build_url("css/style.css"),
            "nefu://localhost/css/style.css"
        );
        assert_eq!(
            handler.build_url("/index.html"),
            "nefu://localhost/index.html"
        );
    }

    #[test]
    fn test_protocol_response_ok() {
        let resp = ProtocolResponse::ok(b"hello".to_vec(), "text/plain");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"hello");
        assert_eq!(
            resp.headers.get("Content-Type").unwrap(),
            "text/plain"
        );
    }

    #[test]
    fn test_protocol_response_not_found() {
        let resp = ProtocolResponse::not_found("missing.html");
        assert_eq!(resp.status, 404);
        assert!(String::from_utf8_lossy(&resp.body).contains("missing.html"));
    }

    #[test]
    fn test_cache_policy_values() {
        assert_eq!(
            CachePolicy::NoCache.to_header_value(),
            "no-cache, no-store, must-revalidate"
        );
        assert!(CachePolicy::ShortLived.to_header_value().contains("max-age"));
        assert!(CachePolicy::Immutable.to_header_value().contains("immutable"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }
}
