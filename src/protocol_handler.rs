//! Protocol handling module
//!
//! Provides functionality similar to the Electron protocol module:
//! - registerFileProtocol - register a file protocol
//! - registerBufferProtocol - register a buffer protocol
//! - registerStringProtocol - register a string protocol
//! - registerHttpProtocol - register an HTTP protocol
//! - unregisterProtocol - unregister a protocol
//! - isProtocolHandled - check whether a protocol is registered
//! - interceptFileProtocol - intercept a file protocol
//! - interceptHttpProtocol - intercept an HTTP protocol
//! - uninterceptProtocol - stop intercepting a protocol

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Protocol handling type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    /// File protocol
    File,
    /// Buffer protocol
    Buffer,
    /// String protocol
    String,
    /// HTTP protocol
    Http,
}

/// Protocol handling response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolResponse {
    /// Status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response data (base64 encoded)
    pub data: Option<String>,
    /// Response text
    pub text: Option<String>,
    /// File path
    pub path: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
    /// Character encoding
    pub charset: Option<String>,
    /// Redirect URL
    pub redirect_url: Option<String>,
}

/// Protocol handling request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRequest {
    /// Request URL
    pub url: String,
    /// Request method
    pub method: String,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Option<String>,
    /// Upload data
    pub upload_data: Option<Vec<u8>>,
    /// Referrer
    pub referrer: Option<String>,
}

/// Protocol handler
pub type ProtocolHandler = Box<dyn Fn(ProtocolRequest) -> Result<ProtocolResponse> + Send + Sync>;

/// Protocol registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRegistration {
    /// Protocol name
    pub scheme: String,
    /// Handling type
    pub handler_type: ProtocolType,
    /// Whether it is a standard protocol
    pub is_standard: bool,
}

/// Protocol manager
///
/// Manages the registration of custom protocols and handlers
pub struct ProtocolManager {
    /// Registered protocol handlers
    handlers: Mutex<HashMap<String, ProtocolHandler>>,
    /// Registered protocol information
    registrations: Mutex<Vec<ProtocolRegistration>>,
    /// Intercepted protocols
    intercepted: Mutex<HashMap<String, ProtocolHandler>>,
}

impl ProtocolManager {
    /// Create a new protocol manager
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
            registrations: Mutex::new(Vec::new()),
            intercepted: Mutex::new(HashMap::new()),
        }
    }

    /// Register a string protocol handler
    ///
    /// # Parameters
    /// - `scheme`: Protocol name (e.g. "app", "custom")
    /// - `handler`: Handler function
    pub fn register_string_protocol<F>(&self, scheme: &str, handler: F) -> Result<bool>
    where
        F: Fn(ProtocolRequest) -> Result<ProtocolResponse> + Send + Sync + 'static,
    {
        let scheme = scheme.to_lowercase();
        if let Ok(mut handlers) = self.handlers.lock() {
            if handlers.contains_key(&scheme) {
                log::warn!("Protocol already registered: {}", scheme);
                return Ok(false);
            }
            handlers.insert(scheme.clone(), Box::new(handler));
        }
        if let Ok(mut regs) = self.registrations.lock() {
            regs.push(ProtocolRegistration {
                scheme: scheme.clone(),
                handler_type: ProtocolType::String,
                is_standard: true,
            });
        }
        log::info!("Protocol registered: {}://", scheme);
        Ok(true)
    }

    /// Register a file protocol handler
    pub fn register_file_protocol<F>(&self, scheme: &str, handler: F) -> Result<bool>
    where
        F: Fn(ProtocolRequest) -> Result<ProtocolResponse> + Send + Sync + 'static,
    {
        let scheme = scheme.to_lowercase();
        if let Ok(mut handlers) = self.handlers.lock() {
            if handlers.contains_key(&scheme) {
                log::warn!("Protocol already registered: {}", scheme);
                return Ok(false);
            }
            handlers.insert(scheme.clone(), Box::new(handler));
        }
        if let Ok(mut regs) = self.registrations.lock() {
            regs.push(ProtocolRegistration {
                scheme: scheme.clone(),
                handler_type: ProtocolType::File,
                is_standard: true,
            });
        }
        log::info!("File protocol registered: {}://", scheme);
        Ok(true)
    }

    /// Unregister a protocol
    pub fn unregister_protocol(&self, scheme: &str) -> Result<bool> {
        let scheme = scheme.to_lowercase();
        let removed = if let Ok(mut handlers) = self.handlers.lock() {
            handlers.remove(&scheme).is_some()
        } else {
            false
        };
        if let Ok(mut regs) = self.registrations.lock() {
            regs.retain(|r| r.scheme != scheme);
        }
        if removed {
            log::info!("Protocol unregistered: {}://", scheme);
        }
        Ok(removed)
    }

    /// Check whether a protocol is registered
    pub fn is_protocol_handled(&self, scheme: &str) -> bool {
        let scheme = scheme.to_lowercase();
        self.handlers.lock()
            .map(|h| h.contains_key(&scheme))
            .unwrap_or(false)
    }

    /// Get all registered protocols
    pub fn get_registered_protocols(&self) -> Vec<ProtocolRegistration> {
        self.registrations.lock()
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Handle a protocol request
    ///
    /// # Parameters
    /// - `url`: Request URL
    ///
    /// # Returns
    /// Handling response
    pub fn handle_request(&self, url: &str) -> Result<ProtocolResponse> {
        let scheme = url.split("://").next()
            .unwrap_or("")
            .to_lowercase();

        let handlers = self.handlers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(handler) = handlers.get(&scheme) {
            let request = ProtocolRequest {
                url: url.to_string(),
                method: "GET".to_string(),
                headers: HashMap::new(),
                body: None,
                upload_data: None,
                referrer: None,
            };
            handler(request)
        } else {
            Err(anyhow::anyhow!("Unregistered protocol: {}", scheme))
        }
    }
}

/// Global protocol manager instance
static PROTOCOL_INSTANCE: std::sync::OnceLock<ProtocolManager> = std::sync::OnceLock::new();

/// Get the global protocol manager
pub fn get_protocol_manager() -> &'static ProtocolManager {
    PROTOCOL_INSTANCE.get_or_init(ProtocolManager::new)
}
