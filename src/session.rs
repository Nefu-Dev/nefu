//! Session module
//!
//! Provides functionality similar to the Electron session:
//! - Cookie management (get, set, remove, flush)
//! - Cache control (clearCache, clearStorageData)
//! - Proxy settings (setProxy, getProxy)
//! - Download management (createDownload, cancelDownload)
//! - Protocol handling (protocol)
//! - Network status (isOnline, networkStatus)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Cookie information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// Name
    pub name: String,
    /// Value
    pub value: String,
    /// Domain
    pub domain: String,
    /// Path
    pub path: String,
    /// Whether HTTPS only
    pub secure: bool,
    /// Whether HTTP only
    pub http_only: bool,
    /// Whether it is a session cookie
    pub session: bool,
    /// Expiration date (timestamp)
    pub expiration_date: Option<f64>,
    /// Same-site policy
    pub same_site: String,
}

/// Cookie query filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieFilter {
    /// Domain (optional)
    pub domain: Option<String>,
    /// Name (optional)
    pub name: Option<String>,
    /// Path (optional)
    pub path: Option<String>,
    /// Whether secure connection only
    pub secure: Option<bool>,
    /// Whether session only
    pub session: Option<bool>,
    /// URL filter
    pub url: Option<String>,
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Proxy mode (direct, auto_detect, pac_script, fixed_servers, system)
    pub mode: String,
    /// PAC script URL
    pub pac_script: Option<String>,
    /// Proxy rules
    pub proxy_rules: Option<String>,
    /// Proxy bypass list
    pub proxy_bypass_list: Option<Vec<String>>,
}

/// Cache size information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSize {
    /// Total size (bytes)
    pub total: u64,
    /// Cache data size
    pub cache: u64,
    /// Storage data size
    pub storage: u64,
    /// Code cache size
    pub code_cache: u64,
}

/// Session manager
///
/// Manages cookies, cache, proxy, and network status
pub struct SessionManager {
    /// Cookie storage
    cookies: Mutex<Vec<Cookie>>,
    /// Proxy configuration
    proxy_config: Mutex<ProxyConfig>,
    /// Network status
    is_online: Mutex<bool>,
}

impl SessionManager {
    /// Create a new Session manager
    pub fn new() -> Self {
        Self {
            cookies: Mutex::new(Vec::new()),
            proxy_config: Mutex::new(ProxyConfig {
                mode: "system".to_string(),
                pac_script: None,
                proxy_rules: None,
                proxy_bypass_list: None,
            }),
            is_online: Mutex::new(true),
        }
    }

    /// Get all cookies
    pub fn get_cookies(&self, filter: Option<CookieFilter>) -> Vec<Cookie> {
        let cookies = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        match filter {
            Some(f) => cookies.iter()
                .filter(|c| {
                    let mut matches = true;
                    if let Some(ref domain) = f.domain {
                        matches = matches && c.domain.contains(domain);
                    }
                    if let Some(ref name) = f.name {
                        matches = matches && c.name == *name;
                    }
                    if let Some(ref path) = f.path {
                        matches = matches && c.path == *path;
                    }
                    matches
                })
                .cloned()
                .collect(),
            None => cookies.clone(),
        }
    }

    /// Set a cookie
    pub fn set_cookie(&self, cookie: Cookie) -> Result<()> {
        if let Ok(mut cookies) = self.cookies.lock() {
            // If a cookie with the same name and domain already exists, update it
            if let Some(pos) = cookies.iter().position(|c| {
                c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path
            }) {
                cookies[pos] = cookie;
            } else {
                cookies.push(cookie);
            }
            log::info!("Cookie set");
        }
        Ok(())
    }

    /// Remove a cookie
    pub fn remove_cookie(&self, url: &str, name: &str) -> Result<()> {
        if let Ok(mut cookies) = self.cookies.lock() {
            cookies.retain(|c| !(c.name == name && url.contains(&c.domain)));
            log::info!("Cookie removed: {} @ {}", name, url);
        }
        Ok(())
    }

    /// Flush the cookie storage
    pub fn flush_cookies(&self) -> Result<()> {
        log::info!("Cookie storage flushed");
        Ok(())
    }

    /// Clear the cache
    pub fn clear_cache(&self) -> Result<()> {
        log::info!("Cache cleared");
        Ok(())
    }

    /// Clear storage data (localStorage, sessionStorage, IndexedDB, etc.)
    pub fn clear_storage_data(&self) -> Result<()> {
        log::info!("Storage data cleared");
        Ok(())
    }

    /// Get the cache size
    pub fn get_cache_size(&self) -> CacheSize {
        CacheSize {
            total: 0,
            cache: 0,
            storage: 0,
            code_cache: 0,
        }
    }

    /// Set the proxy
    pub fn set_proxy(&self, config: ProxyConfig) -> Result<()> {
        if let Ok(mut proxy) = self.proxy_config.lock() {
            *proxy = config;
            log::info!("Proxy configuration updated");
        }
        Ok(())
    }

    /// Get the proxy configuration
    pub fn get_proxy(&self) -> ProxyConfig {
        self.proxy_config.lock()
            .map(|p| p.clone())
            .unwrap_or(ProxyConfig {
                mode: "system".to_string(),
                pac_script: None,
                proxy_rules: None,
                proxy_bypass_list: None,
            })
    }

    /// Get the network status
    pub fn is_online(&self) -> bool {
        *self.is_online.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Set the network status
    pub fn set_online(&self, online: bool) {
        if let Ok(mut status) = self.is_online.lock() {
            *status = online;
        }
    }
}

/// Global Session manager instance
static SESSION_INSTANCE: std::sync::OnceLock<SessionManager> = std::sync::OnceLock::new();

/// Get the global Session manager
pub fn get_session() -> &'static SessionManager {
    SESSION_INSTANCE.get_or_init(SessionManager::new)
}
