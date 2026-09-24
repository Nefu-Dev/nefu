//! Nefu project configuration module
//!
//! Responsible for parsing and managing the main.nefu configuration file (TOML format).
//! Contains default values, validation logic, and file search functionality for all project settings.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Nefu project configuration struct
///
/// Corresponds to all fields in the main.nefu configuration file;
/// unspecified fields use sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NefuConfig {
    /// Entry HTML file path (relative to the project root)
    #[serde(default = "default_entry")]
    pub entry: String,

    /// Output executable file name (without extension)
    #[serde(default = "default_output")]
    pub output: String,

    /// Application description
    #[serde(default = "default_description")]
    pub description: String,

    /// Application version number
    #[serde(default = "default_version")]
    pub version: String,

    /// Author information
    #[serde(default)]
    pub author: String,

    /// Whether debug mode is enabled
    /// Debug mode shows developer tools and console output
    #[serde(default)]
    pub debug: bool,

    /// Preload script path (executed before the page loads)
    #[serde(default)]
    pub preload: Option<String>,

    /// Window width (pixels)
    #[serde(default = "default_window_width")]
    pub window_width: u32,

    /// Window height (pixels)
    #[serde(default = "default_window_height")]
    pub window_height: u32,

    /// Minimum window width
    #[serde(default)]
    pub min_width: Option<u32>,

    /// Minimum window height
    #[serde(default)]
    pub min_height: Option<u32>,

    /// Maximum window width
    #[serde(default)]
    pub max_width: Option<u32>,

    /// Maximum window height
    #[serde(default)]
    pub max_height: Option<u32>,

    /// Whether to start fullscreen
    #[serde(default)]
    pub fullscreen: bool,

    /// Whether the window is resizable
    #[serde(default = "default_true")]
    pub resizable: bool,

    /// Whether to show window decorations (title bar, borders, etc.)
    #[serde(default = "default_true")]
    pub decorations: bool,

    /// Whether the window is always on top
    #[serde(default)]
    pub always_on_top: bool,

    /// Custom User-Agent string
    #[serde(default = "default_user_agent")]
    pub user_agent: String,

    /// Application icon path
    #[serde(default)]
    pub icon: Option<String>,

    /// Window title (defaults to the output name)
    #[serde(default)]
    pub title: Option<String>,

    /// Window initial transparency (0.0 - 1.0)
    #[serde(default)]
    pub transparent: Option<bool>,

    /// Whether to show an icon in the system tray
    #[serde(default)]
    pub tray_icon: Option<String>,

    /// Allowed domain list (for the security policy)
    #[serde(default)]
    pub allowed_domains: Vec<String>,

    /// Additional HTTP response headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// File/directory patterns to exclude when building
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,

    /// Environment variable mapping
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    /// CSP (Content Security Policy) settings
    #[serde(default)]
    pub csp: Option<String>,

    /// Whether the context menu is enabled
    #[serde(default = "default_true")]
    pub context_menu: bool,

    /// Whether file drag-and-drop is allowed
    #[serde(default)]
    pub drag_drop: bool,

    /// Custom protocol name (defaults to nefu)
    #[serde(default = "default_protocol")]
    pub protocol: String,

    /// Automatic update check URL
    #[serde(default)]
    pub update_url: Option<String>,

    /// Single instance lock (prevents multiple instances)
    #[serde(default)]
    pub single_instance: bool,

    /// Window state persistence
    #[serde(default = "default_true")]
    pub persist_window_state: bool,

    /// Rendering mode: auto, high-performance, power-saving
    #[serde(default = "default_rendering_mode")]
    pub rendering_mode: String,
    
    /// Whether render optimization is enabled (auto-injects performance optimization scripts)
    #[serde(default = "default_true")]
    pub render_optimization: bool,

    /// Custom script commands (executable via nefu run <name>)
    #[serde(default)]
    pub scripts: std::collections::HashMap<String, String>,
}

// ==================== Default value functions ====================

fn default_entry() -> String {
    "index.html".to_string()
}

fn default_output() -> String {
    "myapp".to_string()
}

fn default_description() -> String {
    "Nefu App".to_string()
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_window_width() -> u32 {
    1024
}

fn default_window_height() -> u32 {
    768
}

fn default_true() -> bool {
    true
}

fn default_rendering_mode() -> String {
    "auto".to_string()
}

fn default_user_agent() -> String {
    format!("nefu/{}", env!("CARGO_PKG_VERSION"))
}

fn default_protocol() -> String {
    "nefu".to_string()
}

fn default_exclude() -> Vec<String> {
    vec![
        "dist/**".to_string(),
        ".nefu/**".to_string(),
        "node_modules/**".to_string(),
        ".git/**".to_string(),
        // Compilation/build artifact directories and files (Cargo, generic build caches, etc.), to avoid bloating the package
        "target/**".to_string(),
        "build/**".to_string(),
        "deps/**".to_string(),
        ".fingerprint/**".to_string(),
        ".cargo/**".to_string(),
        ".cargo-build-lock".to_string(),
        ".cargo-lock".to_string(),
        ".cargo-artifact-lock".to_string(),
        "*.pdb".to_string(),
        "*.o".to_string(),
        "*.obj".to_string(),
        "*.exe".to_string(),
        // Log and system files
        "*.log".to_string(),
        ".DS_Store".to_string(),
        "Thumbs.db".to_string(),
        "__pycache__/**".to_string(),
        ".idea/**".to_string(),
        ".vscode/**".to_string(),
    ]
}

impl Default for NefuConfig {
    fn default() -> Self {
        Self {
            entry: default_entry(),
            output: default_output(),
            description: default_description(),
            version: default_version(),
            author: String::new(),
            debug: false,
            preload: None,
            window_width: default_window_width(),
            window_height: default_window_height(),
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            fullscreen: false,
            resizable: true,
            decorations: true,
            always_on_top: false,
            user_agent: default_user_agent(),
            icon: None,
            title: None,
            transparent: None,
            tray_icon: None,
            allowed_domains: Vec::new(),
            headers: std::collections::HashMap::new(),
            exclude: default_exclude(),
            env: std::collections::HashMap::new(),
            csp: None,
            context_menu: true,
            drag_drop: false,
            protocol: default_protocol(),
            update_url: None,
            single_instance: false,
            persist_window_state: true,
            rendering_mode: default_rendering_mode(),
            render_optimization: true,
            scripts: std::collections::HashMap::new(),
        }
    }
}

impl NefuConfig {
    /// Load configuration from a file
    ///
    /// # Parameters
    /// - `path`: path to the main.nefu configuration file
    ///
    /// # Returns
    /// The parsed configuration object, or a parse error
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "configuration file does not exist: {}",
                path.display()
            ));
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read configuration file: {}", path.display()))?;

        Self::load_from_str(&content)
            .with_context(|| format!("failed to parse configuration file: {}", path.display()))
    }

    /// Parse configuration from byte content
    ///
    /// Used to load the configuration file directly from a resource bundle.
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self> {
        let content = std::str::from_utf8(bytes)
            .context("configuration file is not valid UTF-8 text")?;
        Self::load_from_str(content)
    }

    /// Parse and validate configuration from string content
    ///
    /// Supports both configuration formats:
    /// 1. Top-level flat fields: `entry = "index.html"`, `window_width = 1024`, ...
    /// 2. Sectioned (manual/legacy): `[app]` / `[build]` / `[dev]` / `[window]` ...
    ///    Uses TOML table structure with different field names.
    pub fn load_from_str(content: &str) -> Result<Self> {
        let table: toml::Table = toml::from_str(content)
            .context("configuration file is not valid TOML format")?;

        // Detect whether the sectioned structure is used (presence of [app]/[build]/[dev]/[window] tables)
        let sectioned = table.contains_key("app")
            || table.contains_key("build")
            || table.contains_key("dev")
            || table.contains_key("window");

        let config = if sectioned {
            Self::from_sectioned(&table)?
        } else {
            toml::from_str(content).context("failed to parse configuration fields")?
        };

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// Parse the sectioned configuration format ([app]/[build]/[dev]/[window]...) into NefuConfig
    fn from_sectioned(table: &toml::Table) -> Result<Self> {
        let get_table = |name: &str| {
            table.get(name).and_then(|v| v.as_table())
        };

        // Flatten all table fields into a flat TOML table, then deserialize
        let mut flat = toml::map::Map::new();
        let mut put = |key: &str, value: toml::Value| {
            flat.insert(key.to_string(), value);
        };

        // [app]
        if let Some(app) = get_table("app") {
            if let Some(v) = app.get("name").and_then(|v| v.as_str()) {
                put("output", toml::Value::String(v.to_string()));
            }
            if let Some(v) = app.get("version").and_then(|v| v.as_str()) {
                put("version", toml::Value::String(v.to_string()));
            }
            if let Some(v) = app.get("width").and_then(|v| v.as_integer()) {
                put("window_width", toml::Value::Integer(v));
            }
            if let Some(v) = app.get("height").and_then(|v| v.as_integer()) {
                put("window_height", toml::Value::Integer(v));
            }
            if let Some(v) = app.get("resizable").and_then(|v| v.as_bool()) {
                put("resizable", toml::Value::Boolean(v));
            }
            if let Some(v) = app.get("fullscreen").and_then(|v| v.as_bool()) {
                put("fullscreen", toml::Value::Boolean(v));
            }
            if let Some(v) = app.get("icon").and_then(|v| v.as_str()) {
                put("icon", toml::Value::String(v.to_string()));
            }
        }

        // [build]
        if let Some(build) = get_table("build") {
            if let Some(v) = build.get("entry").and_then(|v| v.as_str()) {
                put("entry", toml::Value::String(v.to_string()));
            }
            if let Some(v) = build.get("preload").and_then(|v| v.as_str()) {
                put("preload", toml::Value::String(v.to_string()));
            }
            // output may be a full path (e.g. "dist/app.exe"); take its file name stem
            if let Some(v) = build.get("output").and_then(|v| v.as_str()) {
                let stem = Path::new(v)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| v.to_string());
                put("output", toml::Value::String(stem));
            }
            if let Some(v) = build.get("compress").and_then(|v| v.as_bool()) {
                if !v {
                    log::warn!("[build] compress=false: use --no-compress when packaging");
                }
            }
            if let Some(v) = build.get("encrypt").and_then(|v| v.as_bool()) {
                if !v {
                    log::warn!("[build] encrypt=false: use --no-encrypt when packaging");
                }
            }
        }

        // [window]
        if let Some(w) = get_table("window") {
            if let Some(v) = w.get("title").and_then(|v| v.as_str()) {
                put("title", toml::Value::String(v.to_string()));
            }
            if let Some(v) = w.get("min_width").and_then(|v| v.as_integer()) {
                put("min_width", toml::Value::Integer(v));
            }
            if let Some(v) = w.get("min_height").and_then(|v| v.as_integer()) {
                put("min_height", toml::Value::Integer(v));
            }
            if let Some(v) = w.get("max_width").and_then(|v| v.as_integer()) {
                put("max_width", toml::Value::Integer(v));
            }
            if let Some(v) = w.get("max_height").and_then(|v| v.as_integer()) {
                put("max_height", toml::Value::Integer(v));
            }
            if let Some(v) = w.get("frameless").and_then(|v| v.as_bool()) {
                put("decorations", toml::Value::Boolean(!v));
            }
            if let Some(v) = w.get("always_on_top").and_then(|v| v.as_bool()) {
                put("always_on_top", toml::Value::Boolean(v));
            }
        }

        let flat_str = toml::to_string(&toml::Value::Table(flat))
            .context("failed to generate flat configuration")?;

        let config: NefuConfig = toml::from_str(&flat_str)
            .context("failed to parse sectioned configuration fields")?;
        Ok(config)
    }

    /// Validate the configuration
    ///
    /// Checks that all field values are within reasonable ranges
    pub fn validate(&self) -> Result<()> {
        // Validate that the entry file is not empty
        if self.entry.trim().is_empty() {
            return Err(anyhow::anyhow!("entry must not be empty"));
        }

        // Validate that the output name only contains legal characters
        if !self
            .output
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow::anyhow!(
                "output name may only contain letters, digits, hyphens, and underscores"
            ));
        }

        // Validate window dimensions
        if self.window_width < 200 || self.window_width > 7680 {
            return Err(anyhow::anyhow!(
                "window_width must be between 200-7680, current value: {}",
                self.window_width
            ));
        }

        if self.window_height < 150 || self.window_height > 4320 {
            return Err(anyhow::anyhow!(
                "window_height must be between 150-4320, current value: {}",
                self.window_height
            ));
        }

        // Validate min/max size constraints
        if let (Some(min_w), Some(max_w)) = (self.min_width, self.max_width) {
            if min_w > max_w {
                return Err(anyhow::anyhow!(
                    "min_width ({}) must not be greater than max_width ({})",
                    min_w,
                    max_w
                ));
            }
        }

        if let (Some(min_h), Some(max_h)) = (self.min_height, self.max_height) {
            if min_h > max_h {
                return Err(anyhow::anyhow!(
                    "min_height ({}) must not be greater than max_height ({})",
                    min_h,
                    max_h
                ));
            }
        }

        // Validate the version number format
        if !is_valid_semver(&self.version) {
            log::warn!(
                "version '{}' does not follow semantic versioning (x.y.z); a standard format is recommended",
                self.version
            );
        }

        // Validate the protocol name
        if self.protocol.trim().is_empty() {
            return Err(anyhow::anyhow!("protocol must not be empty"));
        }

        if !self
            .protocol
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-')
        {
            return Err(anyhow::anyhow!(
                "protocol name may only contain letters, digits, and hyphens"
            ));
        }

        Ok(())
    }

    /// Get the window title
    ///
    /// Uses the output name if no custom title is set
    pub fn get_title(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| self.output.clone())
    }

    /// Get the full entry file path
    ///
    /// # Parameters
    /// - `project_dir`: project root directory
    pub fn entry_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.entry)
    }

    /// Get the full path of the icon file
    ///
    /// # Parameters
    /// - `project_dir`: project root directory
    pub fn icon_path(&self, project_dir: &Path) -> Option<PathBuf> {
        self.icon.as_ref().map(|icon| project_dir.join(icon))
    }

    /// Get the full path of the preload script
    ///
    /// # Parameters
    /// - `project_dir`: project root directory
    pub fn preload_path(&self, project_dir: &Path) -> Option<PathBuf> {
        self.preload.as_ref().map(|p| project_dir.join(p))
    }

    /// Serialize the configuration to a TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("failed to serialize configuration to TOML")
    }

    /// Save the configuration to a file
    ///
    /// # Parameters
    /// - `path`: target file path
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content)
            .with_context(|| format!("failed to write configuration file: {}", path.display()))?;
        Ok(())
    }

    /// Check whether a file path should be excluded
    ///
    /// # Parameters
    /// - `file_path`: the file path to check (relative to the project root)
    pub fn is_excluded(&self, file_path: &str) -> bool {
        for pattern in &self.exclude {
            if glob_match(pattern, file_path) {
                return true;
            }
        }
        false
    }

    /// Merge environment variables into the current process
    pub fn apply_env_vars(&self) {
        for (key, value) in &self.env {
            std::env::set_var(key, value);
            log::debug!("setting environment variable: {}={}", key, value);
        }
    }
}

/// glob pattern matching
///
/// Supports the following wildcards:
/// - `*`: matches any number of characters (not including the path separator `/`)
/// - `**`: matches any number of characters (including the path separator `/`, across directories)
/// - `?`: matches a single character (not including the path separator `/`)
///
/// Uses a recursive backtracking algorithm to guarantee correctness.
///
/// # Parameters
/// - `pattern`: the glob pattern string
/// - `text`: the text to match
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    match_glob(&p, &t, 0, 0)
}

/// Recursive core implementation of glob matching
fn match_glob(p: &[char], t: &[char], pi: usize, ti: usize) -> bool {
    // Handle ** (matches across directories)
    if p.get(pi) == Some(&'*') && p.get(pi + 1) == Some(&'*') {
        // Skip consecutive *
        let mut pj = pi;
        while p.get(pj) == Some(&'*') {
            pj += 1;
        }
        // If ** is at the end, match all remaining
        if pj >= p.len() {
            return true;
        }
        // ** must match at least to a position where the remaining pattern can hold (across directories)
        for k in ti..=t.len() {
            if match_glob(p, t, pj, k) {
                return true;
            }
        }
        return false;
    }

    // Pattern exhausted
    if pi >= p.len() {
        return ti >= t.len();
    }

    match p[pi] {
        '*' => {
            // Match any number of non-/ characters
            for k in ti..=t.len() {
                // `*` cannot cross the path separator
                if k > ti && t[k - 1] == '/' {
                    break;
                }
                if match_glob(p, t, pi + 1, k) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if ti < t.len() && t[ti] != '/' {
                match_glob(p, t, pi + 1, ti + 1)
            } else {
                false
            }
        }
        pc => {
            if ti < t.len() && t[ti] == pc {
                match_glob(p, t, pi + 1, ti + 1)
            } else {
                false
            }
        }
    }
}

/// Validate the semantic version number format
///
/// # Parameters
/// - `version`: the version number string
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

/// Search for a configuration file in the project directory
///
/// Searches in the following priority order:
/// 1. main.nefu in the current directory
/// 2. nefu.toml in the current directory
/// 3. Configuration files in parent directories (up to 5 levels)
///
/// # Returns
/// The path of the found configuration file, or None if not found
pub fn find_config_file() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let config_names = ["main.nefu", "nefu.toml"];

    // Search in the current directory and parent directories
    let mut search_dir = current_dir.clone();
    for _ in 0..5 {
        for name in &config_names {
            let config_path = search_dir.join(name);
            if config_path.exists() {
                return Some(config_path);
            }
        }

        // Try the parent directory
        if let Some(parent) = search_dir.parent() {
            search_dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    None
}

/// Create default configuration file content
///
/// # Parameters
/// - `project_name`: project name
///
/// # Returns
/// A formatted TOML configuration string
pub fn create_default_config(project_name: &str) -> String {
    format!(
        r#"# Nefu project configuration file
# Docs: https://nefu.dev/docs/config

# Entry HTML file path
entry = "index.html"

# Output executable file name (without extension)
output = "{}"

# Application description
description = "{} App"

# Application version number
version = "1.0.0"

# Author information
author = ""

# Debug mode
debug = false

# Window settings
window_width = 1024
window_height = 768
fullscreen = false
resizable = true

# Custom User-Agent
user_agent = "nefu/1.0"
"#,
        project_name, project_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NefuConfig::default();
        assert_eq!(config.entry, "index.html");
        assert_eq!(config.output, "myapp");
        assert_eq!(config.window_width, 1024);
        assert_eq!(config.window_height, 768);
        assert!(config.resizable);
        assert!(!config.debug);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = NefuConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_width() {
        let mut config = NefuConfig::default();
        config.window_width = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_output() {
        let mut config = NefuConfig::default();
        config.output = "my app!".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_is_valid_semver() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.1.0"));
        assert!(is_valid_semver("10.20.30"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("abc"));
        assert!(!is_valid_semver("1.0.0.0"));
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.html", "index.html"));
        assert!(glob_match("dist/**", "dist/output/app.exe"));
        assert!(glob_match("**/*.log", "logs/debug.log"));
        assert!(!glob_match("*.css", "index.html"));
    }

    #[test]
    fn test_glob_match_multi_star() {
        // Multiple * wildcards should match correctly (this case failed before the fix)
        assert!(glob_match("a*b*c", "axxxbzzc"));
        assert!(glob_match("a*b*c", "abc"));
        assert!(!glob_match("a*b*c", "axxb"));
        // * cannot cross the path separator, ** can
        assert!(!glob_match("a/*/c", "a/b/d/c"));
        assert!(glob_match("a/**/c", "a/b/d/c"));
        // Single-character wildcard ?
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(!glob_match("file?.txt", "file10.txt"));
    }

    #[test]
    fn test_get_title() {
        let mut config = NefuConfig::default();
        assert_eq!(config.get_title(), "myapp");

        config.title = Some("My App".to_string());
        assert_eq!(config.get_title(), "My App");
    }

    #[test]
    fn test_is_excluded() {
        let config = NefuConfig::default();
        assert!(config.is_excluded("dist/output.exe"));
        assert!(config.is_excluded(".nefu/cache"));
        assert!(config.is_excluded("node_modules/pkg/index.js"));
        assert!(!config.is_excluded("src/main.rs"));
        assert!(!config.is_excluded("index.html"));
    }

    #[test]
    fn test_config_serialization() {
        let config = NefuConfig::default();
        let toml_str = config.to_toml().unwrap();
        assert!(toml_str.contains("entry"));
        assert!(toml_str.contains("output"));
    }

    #[test]
    fn test_load_from_bytes() {
        let toml = b"entry = \"app/index.html\"\noutput = \"demo\"\nwindow_width = 1280\nwindow_height = 800\n";
        let config = NefuConfig::load_from_bytes(toml).unwrap();
        assert_eq!(config.entry, "app/index.html");
        assert_eq!(config.output, "demo");
        assert_eq!(config.window_width, 1280);
        assert_eq!(config.window_height, 800);
    }

    #[test]
    fn test_load_from_bytes_invalid() {
        let bad = b"not valid toml [[[";
        assert!(NefuConfig::load_from_bytes(bad).is_err());

        // Invalid window dimensions should cause validation to fail
        let bad_width = b"entry = \"index.html\"\nwindow_width = 5\n";
        assert!(NefuConfig::load_from_bytes(bad_width).is_err());
    }

    #[test]
    fn test_load_from_str_roundtrip() {
        let config = NefuConfig::default();
        let toml_str = config.to_toml().unwrap();
        let reloaded = NefuConfig::load_from_str(&toml_str).unwrap();
        assert_eq!(reloaded.entry, config.entry);
        assert_eq!(reloaded.output, config.output);
        assert_eq!(reloaded.window_width, config.window_width);
    }

    #[test]
    fn test_load_sectioned_format() {
        // Manual/legacy sectioned configuration format
        let toml_str = r#"
[app]
name = "My App"
version = "2.1.0"
width = 1280
height = 800
resizable = true
fullscreen = false

[build]
entry = "pages/app.html"
preload = "preload.js"
output = "dist/deploy.exe"

[window]
title = "Main Window"
min_width = 900
min_height = 600
frameless = true
"#;
        let config = NefuConfig::load_from_str(toml_str).unwrap();
        assert_eq!(config.output, "deploy");
        assert_eq!(config.version, "2.1.0");
        assert_eq!(config.entry, "pages/app.html");
        assert_eq!(config.preload.as_deref(), Some("preload.js"));
        assert_eq!(config.window_width, 1280);
        assert_eq!(config.window_height, 800);
        assert_eq!(config.min_width, Some(900));
        assert_eq!(config.min_height, Some(600));
        assert!(!config.decorations); // frameless = true -> decorations = false
        assert_eq!(config.get_title(), "Main Window");
    }

    #[test]
    fn test_flat_format_still_works() {
        // The top-level flat field format should not be affected by sectioned detection
        let toml_str = r#"
entry = "index.html"
output = "myapp"
window_width = 1024
window_height = 768
"#;
        let config = NefuConfig::load_from_str(toml_str).unwrap();
        assert_eq!(config.entry, "index.html");
        assert_eq!(config.output, "myapp");
        assert_eq!(config.window_width, 1024);
        assert_eq!(config.window_height, 768);
    }

    #[test]
    fn test_create_default_config() {
        let content = create_default_config("testapp");
        assert!(content.contains("testapp"));
        assert!(content.contains("entry"));
    }

    #[test]
    fn test_min_max_validation() {
        let mut config = NefuConfig::default();
        config.min_width = Some(800);
        config.max_width = Some(400);
        assert!(config.validate().is_err());
    }
}
