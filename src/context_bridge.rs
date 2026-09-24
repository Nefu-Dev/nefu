//! Secure context bridge module
//!
//! Provides functionality similar to the Electron contextBridge:
//! - contextIsolation isolation
//! - contextBridge.exposeInMainWorld()
//! - Secure IPC communication channel
//! - Whitelisted API exposure

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Secure bridge configuration
#[derive(Debug, Clone)]
pub struct ContextBridgeConfig {
    /// Whether context isolation is enabled
    pub context_isolation: bool,
    /// List of API names allowed to be exposed
    pub allowed_apis: Vec<String>,
    /// Whether strict mode is enabled (rejects all APIs not on the whitelist)
    pub strict_mode: bool,
}

impl Default for ContextBridgeConfig {
    fn default() -> Self {
        Self {
            context_isolation: true,
            allowed_apis: vec![
                "nefu".to_string(),
                "lj".to_string(),
            ],
            strict_mode: false,
        }
    }
}

/// Secure context bridge
///
/// Manages secure communication between JS and Rust,
/// ensuring that only whitelisted APIs can be accessed by the frontend.
pub struct ContextBridge {
    /// Configuration
    config: ContextBridgeConfig,
    /// Registered API handlers
    handlers: HashMap<String, Box<dyn Fn(Vec<Value>) -> Result<Option<Value>> + Send + Sync>>,
}

impl ContextBridge {
    /// Create a new context bridge
    ///
    /// # Parameters
    /// - `config`: Bridge configuration
    pub fn new(config: ContextBridgeConfig) -> Self {
        Self {
            config,
            handlers: HashMap::new(),
        }
    }

    /// Expose an API to the main world
    ///
    /// # Parameters
    /// - `name`: API name
    /// - `handler`: Handler function
    ///
    /// # Security
    /// If strict_mode is enabled, checks whether name is in allowed_apis
    pub fn expose_in_main_world<F>(&mut self, name: &str, handler: F) -> Result<bool>
    where
        F: Fn(Vec<Value>) -> Result<Option<Value>> + Send + Sync + 'static,
    {
        // Security check
        if self.config.strict_mode && !self.config.allowed_apis.contains(&name.to_string()) {
            return Err(anyhow::anyhow!(
                "API '{}' is not on the whitelist, refusing to expose",
                name
            ));
        }

        if self.handlers.contains_key(name) {
            log::warn!("API already exists, will be overwritten: {}", name);
        }

        self.handlers.insert(name.to_string(), Box::new(handler));
        log::info!("Secure bridge: exposing API '{}'", name);
        Ok(true)
    }

    /// Handle a message from the isolated context
    ///
    /// # Parameters
    /// - `channel`: Communication channel
    /// - `args`: Argument list
    ///
    /// # Returns
    /// Processing result
    pub fn handle_message(&self, channel: &str, args: Vec<Value>) -> Result<Option<Value>> {
        if let Some(handler) = self.handlers.get(channel) {
            handler(args)
        } else {
            Err(anyhow::anyhow!("API not found: {}", channel))
        }
    }

    /// Generate the contextBridge initialization script
    ///
    /// Injected before the page loads to provide secure API access
    pub fn generate_bridge_script(&self) -> String {
        let api_names: Vec<String> = self.handlers.keys().cloned().collect();
        let apis_json = serde_json::to_string(&api_names).unwrap_or_default();

        format!(
            r#"(function() {{
    'use strict';

    // Nefu Context Bridge
    // Provides a secure IPC communication channel
    const contextBridge = {{
        _apis: {},

        /**
         * Invoke a backend API
         * @param {{string}} channel - API name
         * @param {{*}} args - arguments
         * @returns {{Promise<*>}} the return value
         */
        invoke: function(channel, ...args) {{
            return new Promise((resolve, reject) => {{
                try {{
                    const msg = JSON.stringify({{
                        type: 'context_bridge_invoke',
                        channel: channel,
                        args: args
                    }});
                    if (window.ipc && window.ipc.postMessage) {{
                        window.ipc.postMessage(msg);
                    }} else {{
                        reject(new Error('IPC channel is not ready'));
                    }}
                    // Listen for the response
                    const handler = function(event) {{
                        if (event.detail && event.detail.channel === channel) {{
                            window.removeEventListener('nefu-bridge-response', handler);
                            if (event.detail.error) {{
                                reject(new Error(event.detail.error));
                            }} else {{
                                resolve(event.detail.result);
                            }}
                        }}
                    }};
                    window.addEventListener('nefu-bridge-response', handler);
                }} catch (e) {{
                    reject(e);
                }}
            }});
        }},

        /**
         * Check whether an API is available
         * @param {{string}} channel - API name
         * @returns {{boolean}}
         */
        hasAPI: function(channel) {{
            return this._apis.indexOf(channel) !== -1;
        }},

        /**
         * Get the list of all available APIs
         * @returns {{string[]}}
         */
        availableAPIs: function() {{
            return [...this._apis];
        }}
    }};

    // Mount to the global scope (do not overwrite an existing nefu object)
    if (!window.contextBridge) {{
        window.contextBridge = contextBridge;
    }}

    console.log('[Nefu] Context Bridge initialized');
}})();"#,
            apis_json
        )
    }
}

/// Verify whether a message origin is secure
///
/// # Parameters
/// - `request_url`: Request origin URL
/// - `allowed_domains`: Allowed domain whitelist
///
/// # Returns
/// Whether it is secure
pub fn validate_message_origin(request_url: &str, allowed_domains: &[String]) -> bool {
    if allowed_domains.is_empty() {
        return true; // An empty list means all origins are allowed
    }

    // Try to extract the domain from the URL
    let domain = extract_domain(request_url);
    match domain {
        Some(d) => allowed_domains.iter().any(|allowed| d == allowed.as_str()),
        None => false,
    }
}

/// Extract the domain from a URL
fn extract_domain(url: &str) -> Option<String> {
    // Handle the nefu:// protocol and other custom protocols
    let url = if url.contains("://") {
        url
    } else {
        return None;
    };

    let after_protocol = url.split("://").nth(1)?;
    let domain = after_protocol.split('/').next()?;
    let domain = domain.split(':').next()?; // Remove the port number

    Some(domain.to_string())
}

/// Generate a secure IPC script
///
/// Validates message format and whitelist
pub fn generate_safe_ipc_script(allowed_apis: &[String]) -> String {
    let apis_json = serde_json::to_string(allowed_apis).unwrap_or_default();

    format!(
        r#"(function() {{
    'use strict';

    // Secure IPC communication
    const ALLOWED_APIS = {};
    const originalPostMessage = window.ipc && window.ipc.postMessage;

    window.__nefu_safe_ipc = {{
        /**
         * Securely send an IPC message
         * Validates whether the API is on the whitelist
         */
        postMessage: function(message) {{
            try {{
                const parsed = JSON.parse(message);
                if (parsed.type === 'invoke' || parsed.type === 'context_bridge_invoke') {{
                    const method = parsed.channel || parsed.method;
                    if (ALLOWED_APIS.indexOf(method) !== -1) {{
                        if (originalPostMessage) {{
                            originalPostMessage(message);
                        }}
                    }} else {{
                        console.warn('[Nefu] Security restriction: API "' + method + '" is not allowed');
                    }}
                }} else {{
                    if (originalPostMessage) {{
                        originalPostMessage(message);
                    }}
                }}
            }} catch (e) {{
                console.error('[Nefu] Secure IPC error:', e);
            }}
        }}
    }};

    // Replace the original postMessage with the secure version
    if (window.ipc && window.ipc.postMessage) {{
        window.ipc.postMessage = function(msg) {{
            window.__nefu_safe_ipc.postMessage(msg);
        }};
    }}

    console.log('[Nefu] Secure IPC initialized, whitelist:', ALLOWED_APIS);
}})();"#,
        apis_json
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_bridge_config_default() {
        let config = ContextBridgeConfig::default();
        assert!(config.context_isolation);
        assert!(config.allowed_apis.contains(&"nefu".to_string()));
    }

    #[test]
    fn test_expose_in_main_world() {
        let mut bridge = ContextBridge::new(ContextBridgeConfig::default());
        let result = bridge.expose_in_main_world("test_api", |_args| {
            Ok(Some(Value::String("ok".to_string())))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_strict_mode() {
        let config = ContextBridgeConfig {
            strict_mode: true,
            ..ContextBridgeConfig::default()
        };
        let mut bridge = ContextBridge::new(config);
        let result = bridge.expose_in_main_world("forbidden_api", |_args| {
            Ok(Some(Value::Null))
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("nefu://localhost/index.html"),
            Some("localhost".to_string())
        );
        assert_eq!(
            extract_domain("https://example.com:8080/path"),
            Some("example.com".to_string())
        );
        assert!(extract_domain("invalid-url").is_none());
    }

    #[test]
    fn test_validate_message_origin() {
        assert!(validate_message_origin("nefu://localhost/index.html", &[]));
        assert!(validate_message_origin(
            "nefu://localhost/index.html",
            &["localhost".to_string()]
        ));
        assert!(!validate_message_origin(
            "nefu://evil.com/index.html",
            &["localhost".to_string()]
        ));
    }

    #[test]
    fn test_generate_safe_ipc_script() {
        let apis = vec!["nefu".to_string(), "lj".to_string()];
        let script = generate_safe_ipc_script(&apis);
        assert!(script.contains("nefu"));
        assert!(script.contains("lj"));
        assert!(script.contains("__nefu_safe_ipc"));
    }
}
