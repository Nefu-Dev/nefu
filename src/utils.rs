//! Nefu utility functions module
//!
//! Provides common helper functionality for the project:
//! - File operations (read, write, copy, progress display)
//! - SHA-256 hash calculation
//! - Path handling and normalization
//! - Colored terminal output
//! - File size formatting
//! - Project directory detection
//! - Backup file management

use anyhow::{Context, Result};
use log::{debug, info};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

// ==================== File operations ====================

/// Safely read file content as a string
///
/// # Arguments
/// - `path`: File path
///
/// # Returns
/// File content string
pub fn read_file_string(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// Safely read a file as a byte array
///
/// # Arguments
/// - `path`: File path
///
/// # Returns
/// File bytes
pub fn read_file_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// Safely write a file
///
/// Automatically creates the parent directory.
///
/// # Arguments
/// - `path`: Target file path
/// - `content`: Content to write
pub fn write_file(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;

    debug!("Written file: {} ({} bytes)", path.display(), content.len());
    Ok(())
}

/// Copy a file with progress display
///
/// # Arguments
/// - `src`: Source file path
/// - `dst`: Target file path
/// - `show_progress`: Whether to show a progress bar
pub fn copy_file_with_progress(src: &Path, dst: &Path, show_progress: bool) -> Result<u64> {
    // Ensure the target directory exists
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let metadata = std::fs::metadata(src)
        .with_context(|| format!("Failed to get source file info: {}", src.display()))?;

    let total_size = metadata.len();

    if !show_progress || total_size < 1024 * 1024 {
        // Copy small files directly
        return std::fs::copy(src, dst)
            .with_context(|| format!("Failed to copy file: {} -> {}", src.display(), dst.display()));
    }

    // Copy large files with progress
    let mut reader = std::fs::File::open(src)
        .with_context(|| format!("Failed to open source file: {}", src.display()))?;

    let mut writer = std::fs::File::create(dst)
        .with_context(|| format!("Failed to create target file: {}", dst.display()))?;

    let mut buffer = vec![0u8; 8192];
    let mut copied: u64 = 0;
    let start = Instant::now();

    loop {
        let bytes_read = reader.read(&mut buffer)
            .with_context(|| format!("Failed to read file: {}", src.display()))?;

        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read])
            .with_context(|| format!("Failed to write file: {}", dst.display()))?;

        copied += bytes_read as u64;

        // Update progress
        let percent = (copied as f64 / total_size as f64 * 100.0) as usize;
        print!("\r  Copying: [");
        let filled = percent / 2;
        for i in 0..50 {
            if i < filled {
                print!("█");
            } else {
                print!("░");
            }
        }
        print!("] {}% ({}/{})", percent, format_size(copied), format_size(total_size));
        io::stdout().flush().ok();
    }

    println!(); // newline

    let elapsed = start.elapsed();
    let speed = if elapsed.as_secs_f64() > 0.0 {
        copied as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    info!(
        "Copy complete: {} -> {} ({}, {:.1}s, {}/s)",
        src.file_name().unwrap_or_default().to_string_lossy(),
        dst.file_name().unwrap_or_default().to_string_lossy(),
        format_size(copied),
        elapsed.as_secs_f64(),
        format_size(speed as u64)
    );

    Ok(copied)
}

/// Recursively copy a directory
///
/// # Arguments
/// - `src`: Source directory
/// - `dst`: Target directory
/// - `exclude_patterns`: List of exclude patterns
pub fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    exclude_patterns: &[String],
) -> Result<usize> {
    use walkdir::WalkDir;

    let mut count = 0;

    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel_path = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        // Check exclusion rules
        let excluded = exclude_patterns.iter().any(|pattern| {
            simple_glob_match(pattern, &rel_str)
        });

        if excluded {
            continue;
        }

        let target = dst.join(rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).ok();
        } else if entry.file_type().is_file() {
            copy_file_with_progress(entry.path(), &target, false)?;
            count += 1;
        }
    }

    info!("Copied {} files", count);
    Ok(count)
}

// ==================== Hash calculation ====================

/// Compute the SHA-256 hash of a file
///
/// # Arguments
/// - `path`: File path
///
/// # Returns
/// Hex-encoded hash string
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Compute the SHA-256 hash of byte data
///
/// # Arguments
/// - `data`: Byte data
///
/// # Returns
/// Hash byte array
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Verify the SHA-256 checksum of a file
///
/// # Arguments
/// - `path`: File path
/// - `expected`: Expected hex hash value
///
/// # Returns
/// true if it matches
pub fn verify_sha256(path: &Path, expected: &str) -> Result<bool> {
    let computed = sha256_file(path)?;
    Ok(computed.eq_ignore_ascii_case(expected))
}

// ==================== Path handling ====================

/// Normalize path separators to forward slashes
///
/// # Arguments
/// - `path`: Raw path string
pub fn normalize_path_sep(path: &str) -> String {
    path.replace('\\', "/")
}

/// Safely join paths, preventing directory traversal
///
/// # Arguments
/// - `base`: Base directory
/// - `relative`: Relative path
///
/// # Returns
/// Safe full path, or None (if a traversal attack is detected)
pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let normalized = normalize_path_sep(relative);

    // Check for directory traversal
    for component in normalized.split('/') {
        if component == ".." {
            return None;
        }
    }

    let full_path = base.join(&normalized);

    // Ensure the result path stays under the base directory
    if full_path.starts_with(base) {
        Some(full_path)
    } else {
        None
    }
}

/// Get the relative path of a file
///
/// # Arguments
/// - `path`: Absolute path
/// - `base`: Base directory
pub fn relative_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Ensure a directory exists
///
/// # Arguments
/// - `path`: Directory path
pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    }
    Ok(())
}

// ==================== Project directory detection ====================

/// Find the Nefu project root directory
///
/// Searches upward from the current directory for one containing main.nefu or nefu.toml.
///
/// # Returns
/// Project root directory path
pub fn find_project_dir() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;
    let config_files = ["main.nefu", "nefu.toml"];

    let mut search = current.clone();
    for _ in 0..10 {
        for name in &config_files {
            if search.join(name).exists() {
                return Some(search);
            }
        }

        if let Some(parent) = search.parent() {
            search = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Fall back to the current directory
    Some(current)
}

/// Check whether the given directory is a Nefu project
///
/// # Arguments
/// - `dir`: Directory to check
pub fn is_nefu_project(dir: &Path) -> bool {
    dir.join("main.nefu").exists() || dir.join("nefu.toml").exists()
}

// ==================== Terminal output ====================

/// ANSI color codes
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
}

/// Print a colored success message
pub fn print_success(message: &str) {
    println!("{}✅ {}{}", colors::GREEN, message, colors::RESET);
}

/// Print a colored error message
pub fn print_error(message: &str) {
    eprintln!("{}❌ {}{}", colors::RED, message, colors::RESET);
}

/// Print a colored warning message
pub fn print_warning(message: &str) {
    println!("{}⚠️  {}{}", colors::YELLOW, message, colors::RESET);
}

/// Print a colored info message
pub fn print_info(message: &str) {
    println!("{}ℹ️  {}{}", colors::CYAN, message, colors::RESET);
}

/// Print a build step
pub fn print_step(step: usize, total: usize, message: &str) {
    println!(
        "{}[{}/{}]{} {}{}{}",
        colors::BOLD, step, total, colors::RESET,
        colors::BLUE, message, colors::RESET
    );
}

/// Print a separator line
pub fn print_separator() {
    println!("{}", "─".repeat(60));
}

/// Print a title banner
pub fn print_banner(title: &str) {
    let width = 60;
    let padding = (width - title.len() - 4) / 2;
    println!();
    println!("{}╔{}╗{}", colors::BOLD, "═".repeat(width - 2), colors::RESET);
    println!(
        "{}║{}{}{}║{}",
        colors::BOLD,
        " ".repeat(padding.max(0)),
        title,
        " ".repeat((width - 4 - title.len()).max(0)),
        colors::RESET
    );
    println!("{}╚{}╝{}", colors::BOLD, "═".repeat(width - 2), colors::RESET);
    println!();
}

// ==================== Size formatting ====================

/// Format a byte count as a human-readable size string
///
/// # Arguments
/// - `bytes`: Byte count
///
/// # Examples
/// ```
/// assert_eq!(format_size(0), "0 B");
/// assert_eq!(format_size(1024), "1.0 KB");
/// assert_eq!(format_size(1048576), "1.0 MB");
/// ```
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Parse a human-readable size string into bytes
///
/// # Arguments
/// - `size_str`: Size string (e.g. "1.5MB", "100KB")
///
/// # Returns
/// Byte count
pub fn parse_size(size_str: &str) -> Result<u64> {
    let trimmed = size_str.trim().to_uppercase();

    let (number_part, unit) = if let Some(pos) = trimmed.find(|c: char| c.is_alphabetic()) {
        (&trimmed[..pos], &trimmed[pos..])
    } else {
        (trimmed.as_str(), "B")
    };

    let number: f64 = number_part.trim()
        .parse()
        .with_context(|| format!("Invalid number: {}", number_part))?;

    // Reject NaN / negative / infinity
    if !number.is_finite() || number < 0.0 {
        return Err(anyhow::anyhow!("Size must be a non-negative finite number: {}", number_part));
    }

    let multiplier: f64 = match unit.trim() {
        "B" | "" => 1.0,
        "K" | "KB" => 1024.0,
        "M" | "MB" => 1024.0 * 1024.0,
        "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return Err(anyhow::anyhow!("Unknown size unit: {}", unit)),
    };

    let value = number * multiplier;

    // Use an upper bound check to prevent overflow truncation
    if value > u64::MAX as f64 {
        return Err(anyhow::anyhow!("Size exceeds the range representable by u64: {}", size_str));
    }

    Ok(value as u64)
}

// ==================== Backup management ====================

/// Create a file backup
///
/// Appends a `.bak` suffix and timestamp to the file name.
///
/// # Arguments
/// - `path`: Path of the file to back up
///
/// # Returns
/// Path of the backup file
pub fn create_backup(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
    }

    let timestamp = chrono_timestamp();
    let backup_name = format!(
        "{}.{}.bak",
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        timestamp
    );

    let backup_path = path.parent()
        .unwrap_or(Path::new("."))
        .join(&backup_name);

    std::fs::copy(path, &backup_path)
        .with_context(|| format!("Failed to create backup: {}", path.display()))?;

    info!("Backup created: {}", backup_path.display());
    Ok(backup_path)
}

/// Generate a simple timestamp string
fn chrono_timestamp() -> String {
    use std::time::SystemTime;

    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    format!("{}", duration.as_secs())
}

/// Clean up old backup files
///
/// Keeps the most recent N backups and deletes the rest.
///
/// # Arguments
/// - `dir`: Directory containing the backups
/// - `prefix`: File name prefix
/// - `keep`: Number to keep
pub fn cleanup_backups(dir: &Path, prefix: &str, keep: usize) -> Result<usize> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map_or(false, |n| n.to_string_lossy().starts_with(prefix))
                && p.extension().map_or(false, |e| e == "bak")
        })
        .collect();

    // Sort by modification time (newest first)
    backups.sort_by(|a, b| {
        let time_a = std::fs::metadata(a).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let time_b = std::fs::metadata(b).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    let mut removed = 0;
    for old_backup in backups.iter().skip(keep) {
        if std::fs::remove_file(old_backup).is_ok() {
            debug!("Deleted old backup: {}", old_backup.display());
            removed += 1;
        }
    }

    if removed > 0 {
        info!("Cleaned up {} old backup files", removed);
    }

    Ok(removed)
}

// ==================== Template rendering ====================

/// Simple template variable substitution
///
/// Supports `{{variable}}` placeholders.
///
/// # Arguments
/// - `template`: Template string
/// - `vars`: Mapping of variable names to values
pub fn render_template(template: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

// ==================== Miscellaneous utilities ====================

/// Simple glob pattern matching
///
/// Supports `*` (not crossing `/`) and `**` (crossing directories), using recursive backtracking for correctness.
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_recursive(&p, &t, 0, 0)
}

/// Core recursive glob matching implementation
fn glob_match_recursive(p: &[char], t: &[char], pi: usize, ti: usize) -> bool {
    // Handle ** (can cross directories)
    if p.get(pi) == Some(&'*') && p.get(pi + 1) == Some(&'*') {
        let mut pj = pi;
        while p.get(pj) == Some(&'*') {
            pj += 1;
        }
        if pj >= p.len() {
            return true;
        }
        for k in ti..=t.len() {
            if glob_match_recursive(p, t, pj, k) {
                return true;
            }
        }
        return false;
    }

    if pi >= p.len() {
        return ti >= t.len();
    }

    match p[pi] {
        '*' => {
            for k in ti..=t.len() {
                if k > ti && t[k - 1] == '/' {
                    break;
                }
                if glob_match_recursive(p, t, pi + 1, k) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if ti < t.len() && t[ti] != '/' {
                glob_match_recursive(p, t, pi + 1, ti + 1)
            } else {
                false
            }
        }
        pc => {
            if ti < t.len() && t[ti] == pc {
                glob_match_recursive(p, t, pi + 1, ti + 1)
            } else {
                false
            }
        }
    }
}

/// Generate a unique temporary file path
///
/// # Arguments
/// - `prefix`: File name prefix
/// - `extension`: File extension
pub fn temp_file_path(prefix: &str, extension: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4().to_string();
    let short_id = &id[..8];
    std::env::temp_dir().join(format!("{}_{}.{}", prefix, short_id, extension))
}

/// Check whether a command is available in PATH
///
/// # Arguments
/// - `command`: Command name
pub fn command_exists(command: &str) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(false, |s| s.success())
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("which")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(false, |s| s.success())
    }
}

/// Safe wrapper to get the current working directory
pub fn current_dir_safe() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Dependency type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepType {
    /// WebView2 Runtime (required on Windows)
    WebView2,
    /// NSIS installer tool (required for --installer)
    Nsis,
    /// Android SDK (required for build-android)
    AndroidSdk,
    /// Java JDK (required for Android builds)
    Java,
    /// Rcedit (Windows exe icon tool)
    Rcedit,
}

/// Check for and automatically download missing dependencies
///
/// Detects whether the dependency is installed and downloads it automatically if missing.
///
/// # Arguments
/// - `dep_type`: Dependency type to check
///
/// # Returns
/// `true` if the dependency is available (installed or downloaded), `false` otherwise
pub fn check_and_download_dep(dep_type: DepType) -> bool {
    match dep_type {
        DepType::WebView2 => check_webview2(),
        DepType::Nsis => check_or_download_nsis(),
        DepType::AndroidSdk => check_android_sdk(),
        DepType::Java => check_java(),
        DepType::Rcedit => check_or_download_rcedit(),
    }
}

/// Check whether the WebView2 Runtime is installed
///
/// WebView2 is required for wry to run on Windows.
/// It ships with Windows 11; Windows 10 may require installation.
fn check_webview2() -> bool {
    #[cfg(windows)]
    {
        // Check the registry for WebView2 installation info
        let check_cmd = std::process::Command::new("reg")
            .args([
                "query",
                "HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
                "/v",
                "pv",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(false, |s| s.success());

        if check_cmd {
            log::info!("WebView2 Runtime is installed");
            return true;
        }

        log::warn!("WebView2 Runtime is not installed, attempting automatic download and install...");
        match download_file(
            "https://go.microsoft.com/fwlink/p/?LinkId=2124703",
            &std::env::temp_dir().join("MicrosoftEdgeWebview2Setup.exe"),
        ) {
            Ok(path) => {
                log::info!("WebView2 installer downloaded: {}", path.display());
                log::info!("Installing WebView2 Runtime silently...");
                let status = std::process::Command::new(&path)
                    .args(["/silent", "/install"])
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        log::info!("WebView2 Runtime installed successfully!");
                        true
                    }
                    _ => {
                        log::warn!("WebView2 Runtime automatic install failed, please install manually");
                        log::warn!("Download: https://developer.microsoft.com/microsoft-edge/webview2/");
                        false
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to download WebView2: {}", e);
                false
            }
        }
    }

    #[cfg(not(windows))]
    {
        true // WebView2 Runtime is not needed on non-Windows platforms
    }
}

/// Check for or automatically download NSIS (Nullsoft Scriptable Install System)
///
/// Used to generate Windows installers (--installer option)
fn check_or_download_nsis() -> bool {
    if command_exists("makensis") {
        log::info!("NSIS (makensis) is installed");
        return true;
    }

    #[cfg(windows)]
    {
        log::warn!("NSIS is not installed, attempting automatic download...");
        let nsis_url = "https://sourceforge.net/projects/nsis/files/latest/download";
        let temp_path = std::env::temp_dir().join("nsis-setup.exe");

        match download_file(nsis_url, &temp_path) {
            Ok(path) => {
                log::info!("NSIS installer downloaded: {}", path.display());
                log::info!("Please run the installer manually: {}", path.display());
                log::info!("After installation, ensure makensis is in PATH");
                // Open the download directory
                let _ = std::process::Command::new("explorer")
                    .arg("/select,")
                    .arg(&path)
                    .spawn();
                false
            }
            Err(e) => {
                log::warn!("Failed to download NSIS: {}", e);
                log::warn!("Please download and install manually: https://nsis.sourceforge.io/Download");
                false
            }
        }
    }

    #[cfg(not(windows))]
    {
        log::warn!("NSIS only supports Windows, please use WINE or install manually");
        false
    }
}

/// Check for and automatically download rcedit (used to set exe icons)
///
/// rcedit is a lightweight PE resource editor developed by the electron team,
/// used to modify the icon, version info and other resources of Windows executables.
/// After download it is cached in the nefu dependencies directory.
fn check_or_download_rcedit() -> bool {
    #[cfg(windows)]
    {
        let deps_dir = nefu_deps_dir();
        let rcedit_path = deps_dir.join("rcedit-x64.exe");

        if rcedit_path.exists() {
            log::debug!("rcedit cached: {}", rcedit_path.display());
            return true;
        }

        log::info!("Downloading rcedit (icon tool)...");
        let url = "https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe";
        match download_file(url, &rcedit_path) {
            Ok(path) => {
                log::info!("rcedit downloaded to: {}", path.display());
                true
            }
            Err(e) => {
                log::warn!("Failed to download rcedit: {}, icon setup will be skipped", e);
                log::warn!("Can be downloaded manually: https://github.com/electron/rcedit/releases");
                false
            }
        }
    }

    #[cfg(not(windows))]
    {
        // rcedit is not needed on non-Windows platforms
        true
    }
}

/// Get the rcedit executable path
#[cfg(windows)]
pub fn rcedit_path() -> Option<PathBuf> {
    let deps_dir = nefu_deps_dir();
    let path = deps_dir.join("rcedit-x64.exe");
    if path.exists() { Some(path) } else { None }
}

/// Get the Nefu local cache directory (stores the auto-downloaded JDK/Android SDK/Gradle)
///
/// Windows: %LOCALAPPDATA%\nefu\deps
/// macOS/Linux: ~/.nefu/deps
pub fn nefu_deps_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_next::home_dir().unwrap_or_else(|| PathBuf::from(".")))
    } else {
        dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("."))
    };
    let dir = if cfg!(windows) {
        base.join("nefu").join("deps")
    } else {
        base.join(".nefu").join("deps")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Check whether the Java JDK is available; if missing, download Adoptium JDK 17 automatically
fn check_java() -> bool {
    // 1. Check JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_path = std::path::Path::new(&java_home);
        if java_path.join("bin").join("java.exe").exists()
            || java_path.join("bin").join("java").exists()
        {
            log::info!("Java found (JAVA_HOME): {}", java_home);
            return true;
        }
    }

    // 2. Check the java command
    if command_exists("java") {
        log::info!("java command is available (PATH)");
        return true;
    }

    // 3. Check the locally cached JDK
    let cached_jdk = find_cached_jdk();
    if cached_jdk.is_some() {
        log::info!("Java found (local cache)");
        return true;
    }

    // 4. Download the JDK automatically
    log::warn!("Java is not installed; downloading Adoptium JDK 17 automatically...");
    match download_and_install_jdk() {
        Ok(path) => {
            log::info!("JDK installed successfully: {}", path.display());
            true
        }
        Err(e) => {
            log::error!("JDK auto-download failed: {}", e);
            log::error!("please install JDK 17+ manually: https://adoptium.net/");
            false
        }
    }
}

/// Find an installed JDK in the local cache directory
fn find_cached_jdk() -> Option<PathBuf> {
    let jdk_dir = nefu_deps_dir().join("jdk");
    if !jdk_dir.exists() {
        return None;
    }
    // Find jdk-* subdirectories
    if let Ok(entries) = std::fs::read_dir(&jdk_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir()
                && (p.join("bin").join("java.exe").exists()
                    || p.join("bin").join("java").exists())
            {
                return Some(p);
            }
        }
    }
    None
}

/// Download and install Adoptium JDK 17 to the local cache
fn download_and_install_jdk() -> Result<PathBuf> {
    let deps_dir = nefu_deps_dir();
    let jdk_dir = deps_dir.join("jdk");
    std::fs::create_dir_all(&jdk_dir)?;

    let archive_ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let archive_path = deps_dir.join(format!("jdk-17.{}", archive_ext));

    // Download (multi-mirror fallback; prefer Tsinghua/Tencent mirrors for domestic networks)
    let urls = jdk_download_urls();
    download_file_fallback(&urls, &archive_path)?;

    // Extract
    log::info!("extracting JDK...");
    extract_archive(&archive_path, &jdk_dir)?;

    // Find the extracted JDK directory
    let jdk_home = find_cached_jdk()
        .ok_or_else(|| anyhow::anyhow!("no java executable found after JDK extraction"))?;

    log::info!("JDK installed: {}", jdk_home.display());
    Ok(jdk_home)
}

/// Get the Java executable path (JAVA_HOME first, then PATH, finally local cache)
pub fn get_java_executable() -> Option<PathBuf> {
    // JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(&java_home).join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if p.exists() {
            return Some(p);
        }
    }
    // PATH
    if command_exists("java") {
        return Some(PathBuf::from("java"));
    }
    // Local cache
    if let Some(jdk_home) = find_cached_jdk() {
        let p = jdk_home.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Check whether the Android SDK is available; if missing, download command-line tools and install required components
fn check_android_sdk() -> bool {
    // 1. Check environment variables
    let android_home = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .ok();

    if let Some(ref path) = android_home {
        let sdk_path = std::path::Path::new(path);
        if sdk_path.join("platforms").exists() && sdk_path.join("platform-tools").exists() {
            log::info!("Android SDK found: {}", path);
            return true;
        }
    }

    // 2. Check local cache
    let cached_sdk = nefu_deps_dir().join("android-sdk");
    if cached_sdk.join("platforms").exists() && cached_sdk.join("platform-tools").exists() {
        log::info!("Android SDK found (local cache): {}", cached_sdk.display());
        return true;
    }

    // 3. Download and install automatically
    log::warn!("Android SDK is not installed; downloading command-line tools automatically...");
    match download_and_install_android_sdk() {
        Ok(path) => {
            log::info!("Android SDK installed successfully: {}", path.display());
            true
        }
        Err(e) => {
            log::error!("Android SDK auto-download failed: {}", e);
            log::error!("please install Android Studio manually: https://developer.android.com/studio");
            false
        }
    }
}

/// Get the Android SDK path (environment variable first, then local cache)
pub fn get_android_sdk_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT")) {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }
    let cached = nefu_deps_dir().join("android-sdk");
    if cached.exists() {
        return Some(cached);
    }
    None
}

/// Download and install Android SDK command-line tools + required components
fn download_and_install_android_sdk() -> Result<PathBuf> {
    let deps_dir = nefu_deps_dir();
    let sdk_dir = deps_dir.join("android-sdk");
    std::fs::create_dir_all(&sdk_dir)?;

    // Download command-line tools (multi-mirror fallback)
    let archive_path = deps_dir.join("commandlinetools.zip");
    let urls = android_cmdtools_urls();
    download_file_fallback(&urls, &archive_path)?;

    // Extract to a temporary directory
    let temp_extract = deps_dir.join("_cmdtools_extract");
    let _ = std::fs::remove_dir_all(&temp_extract);
    extract_archive(&archive_path, &temp_extract)?;

    // After extraction command-line tools is a cmdline-tools/ directory that must be placed in sdk/cmdline-tools/latest/
    let cmdline_src = temp_extract.join("cmdline-tools");
    let latest_dir = sdk_dir.join("cmdline-tools").join("latest");
    std::fs::create_dir_all(&latest_dir)?;

    if cmdline_src.exists() {
        // Copy the cmdline-tools contents into latest/
        copy_dir_recursively(&cmdline_src, &latest_dir)?;
    }

    // Clean up the temporary directory
    let _ = std::fs::remove_dir_all(&temp_extract);

    // Use sdkmanager to install the required components
    let sdkmanager = latest_dir.join("bin").join(if cfg!(windows) { "sdkmanager.bat" } else { "sdkmanager" });

    if sdkmanager.exists() {
        log::info!("using sdkmanager to install Android SDK components...");

        // Set the ANDROID_HOME environment variable for sdkmanager
        let sdk_path_str = sdk_dir.to_string_lossy().to_string();

        // Accept licenses
        let _ = run_command_with_env(
            &sdkmanager,
            &["--licenses"],
            &[("ANDROID_HOME", &sdk_path_str), ("ANDROID_SDK_ROOT", &sdk_path_str)],
            Some("y\ny\ny\ny\ny\ny\ny\ny\n"),
        );

        // Install the required components
        let packages = [
            "platform-tools",
            "platforms;android-34",
            "build-tools;34.0.0",
        ];

        for pkg in &packages {
            log::info!("installing component: {}", pkg);
            let status = run_command_with_env(
                &sdkmanager,
                &[pkg],
                &[("ANDROID_HOME", &sdk_path_str), ("ANDROID_SDK_ROOT", &sdk_path_str)],
                Some("y\n"),
            );
            if !status {
                log::warn!("component {} may have failed to install, continuing...", pkg);
            }
        }

        log::info!("Android SDK components installed");
    } else {
        log::warn!("sdkmanager not found; please install Android SDK components manually");
    }

    Ok(sdk_dir)
}

/// Download and install Gradle to the local cache (returns the gradle executable path)
pub fn ensure_gradle() -> Result<PathBuf> {
    let deps_dir = nefu_deps_dir();
    let gradle_dir = deps_dir.join("gradle");

    // Check whether it already exists
    if let Some(gradle_bin) = find_gradle_binary(&gradle_dir) {
        log::info!("Gradle found: {}", gradle_bin.display());
        return Ok(gradle_bin);
    }

    // Download Gradle (multi-mirror fallback; prefer Tencent/Huawei mirrors domestically)
    log::warn!("Gradle is not installed; downloading Gradle 8.4 automatically...");
    std::fs::create_dir_all(&gradle_dir)?;

    let archive_path = deps_dir.join("gradle-8.4-bin.zip");
    let urls = gradle_download_urls();
    download_file_fallback(&urls, &archive_path)?;

    log::info!("extracting Gradle...");
    extract_archive(&archive_path, &gradle_dir)?;

    find_gradle_binary(&gradle_dir)
        .ok_or_else(|| anyhow::anyhow!("no gradle executable found after Gradle extraction"))
}

/// Find the gradle executable in a directory
fn find_gradle_binary(base_dir: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let bin = p.join("bin").join(if cfg!(windows) { "gradle.bat" } else { "gradle" });
                if bin.exists() {
                    return Some(bin);
                }
            }
        }
    }
    None
}

/// Extract a zip or tar.gz archive to the target directory
fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let path_str = archive_path.to_string_lossy().to_lowercase();

    if path_str.ends_with(".zip") {
        extract_zip(archive_path, target_dir)
    } else if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        extract_tar_gz(archive_path, target_dir)
    } else {
        Err(anyhow::anyhow!("unsupported archive format: {}", archive_path.display()))
    }
}

/// Extract a ZIP file
fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path)
        .with_context(|| format!("failed to open ZIP: {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("ZIP parse failed: {}", archive_path.display()))?;

    for i in 0..archive.len() {
        let mut file_in_zip = archive.by_index(i)?;
        let outpath = target_dir.join(file_in_zip.name());

        if file_in_zip.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file_in_zip, &mut outfile)?;
        }
    }
    Ok(())
}

/// Extract a tar.gz file (uses the system tar command because the Rust dependencies do not include the tar crate)
fn extract_tar_gz(archive_path: &Path, target_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(target_dir)?;
    let status = std::process::Command::new("tar")
        .args(["-xzf", &archive_path.to_string_lossy(), "-C", &target_dir.to_string_lossy()])
        .status()
        .context("Failed to extract tar")?;
    if !status.success() {
        return Err(anyhow::anyhow!("tar extraction failed, status code: {:?}", status.code()));
    }
    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursively(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(src).unwrap_or(path);
        let target = dst.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target)?;
        }
    }
    Ok(())
}

/// Run a command with environment variables set, optionally feeding stdin
fn run_command_with_env(
    cmd: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
    stdin_input: Option<&str>,
) -> bool {
    let mut command = std::process::Command::new(cmd);
    command.args(args);
    for (k, v) in envs {
        command.env(k, v);
    }
    command.stdout(std::process::Stdio::inherit());
    command.stderr(std::process::Stdio::inherit());

    if let Some(input) = stdin_input {
        command.stdin(std::process::Stdio::piped());
        match command.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(input.as_bytes());
                }
                child.wait().map(|s| s.success()).unwrap_or(false)
            }
            Err(e) => {
                log::error!("failed to run command: {}", e);
                false
            }
        }
    } else {
        command.status().map(|s| s.success()).unwrap_or(false)
    }
}

/// Download a file to a local path (with caching: skip re-download if the file exists and is non-empty)
///
/// # Arguments
/// - `url`: Download URL
/// - `target_path`: Save path
///
/// # Returns
/// Path of the downloaded file (returns the original path if already cached)
pub fn download_file(url: &str, target_path: &Path) -> Result<PathBuf> {
    // Cache check: skip downloading when the file exists and is larger than 0, to avoid re-downloading
    if target_path.exists() {
        if let Ok(meta) = std::fs::metadata(target_path) {
            if meta.len() > 0 {
                log::info!("file already exists, skipping download: {} ({} bytes)", target_path.display(), meta.len());
                return Ok(target_path.to_path_buf());
            }
        }
        // The file exists but is empty; delete it and re-download
        let _ = std::fs::remove_file(target_path);
    }

    log::info!("downloading: {}", url);

    // Ensure the parent directory exists
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    // Download over HTTP using ureq
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

    let total_size = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    // Stream the file to disk to avoid large files using too much memory
    let mut file = std::fs::File::create(target_path)
        .with_context(|| format!("failed to create file: {}", target_path.display()))?;

    let mut reader = resp.into_reader();
    let mut downloaded: u64 = 0;
    let mut last_log_pct: i32 = -1;

    let mut buf = [0u8; 65536];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| anyhow::anyhow!("failed to read response data: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .with_context(|| format!("failed to write file: {}", target_path.display()))?;
        downloaded += n as u64;

        if total_size > 0 {
            let pct = ((downloaded as f64 / total_size as f64) * 100.0) as i32;
            if pct != last_log_pct && pct % 10 == 0 {
                last_log_pct = pct;
                log::info!("download progress: {}% ({}/{})", pct, downloaded, total_size);
            }
        }
    }

    file.flush()?;
    drop(file);

    log::info!("download complete: {} ({} bytes)", target_path.display(), downloaded);
    Ok(target_path.to_path_buf())
}

/// Single-URL download with timeout (used for fallback; a shorter timeout allows faster mirror switching)
fn download_file_with_timeout(url: &str, target_path: &Path, timeout_secs: u64) -> Result<PathBuf> {
    // Cache check
    if target_path.exists() {
        if let Ok(meta) = std::fs::metadata(target_path) {
            if meta.len() > 0 {
                return Ok(target_path.to_path_buf());
            }
        }
        let _ = std::fs::remove_file(target_path);
    }

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .call()
        .map_err(|e| anyhow::anyhow!("HTTP request failed [{}]: {}", url, e))?;

    let total_size = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let mut file = std::fs::File::create(target_path)?;
    let mut reader = resp.into_reader();
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 65536];

    loop {
        let n = reader.read(&mut buf)
            .map_err(|e| anyhow::anyhow!("read failed: {}", e))?;
        if n == 0 { break; }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
    }

    file.flush()?;
    drop(file);
    log::info!("download complete [{}]: {} bytes", url, downloaded);
    Ok(target_path.to_path_buf())
}

/// Multi-URL fallback download: try each URL in order and return on the first success
///
/// Used to automatically switch to domestic mirrors when the official source is unstable on domestic networks.
/// The first few mirrors use a shorter timeout (60s) for fast failover; the last uses a long timeout.
///
/// # Arguments
/// - `urls`: URL list ordered by priority (first has priority)
/// - `target_path`: Save path
///
/// # Returns
/// Path of the successfully downloaded file
pub fn download_file_fallback(urls: &[&str], target_path: &Path) -> Result<PathBuf> {
    if urls.is_empty() {
        return Err(anyhow::anyhow!("no available download URL"));
    }

    // Cache check (only needed once)
    if target_path.exists() {
        if let Ok(meta) = std::fs::metadata(target_path) {
            if meta.len() > 0 {
                log::info!("file already exists, skipping download: {}", target_path.display());
                return Ok(target_path.to_path_buf());
            }
        }
        let _ = std::fs::remove_file(target_path);
    }

    let total = urls.len();
    let mut last_error: Option<anyhow::Error> = None;

    for (i, url) in urls.iter().enumerate() {
        // The first N-1 use a 60s timeout to fail fast; the last uses a 300s long timeout
        let timeout = if i == total - 1 { 300 } else { 60 };
        log::info!("attempting download ({}/{}): {} (timeout: {}s)", i + 1, total, url, timeout);

        match download_file_with_timeout(url, target_path, timeout) {
            Ok(path) => return Ok(path),
            Err(e) => {
                log::warn!("download failed ({}): {}", url, e);
                last_error = Some(e);
                // Delete a possibly incomplete file
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("all download URLs failed")))
}

// ==================== Domestic mirror list ====================

/// Gradle 8.4 download mirror list (official + domestic mirrors)
fn gradle_download_urls() -> Vec<&'static str> {
    vec![
        // Domestic mirrors take priority (user is in China)
        "https://mirrors.cloud.tencent.com/gradle/gradle-8.4-bin.zip",
        "https://mirrors.huaweicloud.com/gradle/gradle-8.4-bin.zip",
        "https://mirrors.aliyun.com/macports/distfiles/gradle/gradle-8.4-bin.zip",
        // Official source fallback
        "https://services.gradle.org/distributions/gradle-8.4-bin.zip",
    ]
}

/// Adoptium JDK 17 download mirror list
fn jdk_download_urls() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            // Tsinghua University mirror (domestic)
            "https://mirrors.tuna.tsinghua.edu.cn/Adoptium/17/jdk/x64/windows/OpenJDK17U-jdk_x64_windows_hotspot_17.0.12_7.zip",
            // Tencent Cloud mirror
            "https://mirrors.cloud.tencent.com/Adoptium/17/jdk/x64/windows/OpenJDK17U-jdk_x64_windows_hotspot_17.0.12_7.zip",
            // Official API
            "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "https://mirrors.tuna.tsinghua.edu.cn/Adoptium/17/jdk/aarch64/mac/OpenJDK17U-jdk_aarch64_mac_hotspot_17.0.12_7.tar.gz",
            "https://api.adoptium.net/v3/binary/latest/17/ga/mac/aarch64/jdk/hotspot/normal/eclipse",
        ]
    } else {
        vec![
            "https://mirrors.tuna.tsinghua.edu.cn/Adoptium/17/jdk/x64/linux/OpenJDK17U-jdk_x64_linux_hotspot_17.0.12_7.tar.gz",
            "https://mirrors.cloud.tencent.com/Adoptium/17/jdk/x64/linux/OpenJDK17U-jdk_x64_linux_hotspot_17.0.12_7.tar.gz",
            "https://api.adoptium.net/v3/binary/latest/17/ga/linux/x64/jdk/hotspot/normal/eclipse",
        ]
    }
}

/// Android SDK command-line tools download mirror list
fn android_cmdtools_urls() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            // Official source (dl.google.com is accessible domestically in China)
            "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip",
            // Tencent Cloud mirror
            "https://mirrors.cloud.tencent.com/AndroidSDK/commandlinetools-win-11076708_latest.zip",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "https://dl.google.com/android/repository/commandlinetools-mac-11076708_latest.zip",
            "https://mirrors.cloud.tencent.com/AndroidSDK/commandlinetools-mac-11076708_latest.zip",
        ]
    } else {
        vec![
            "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip",
            "https://mirrors.cloud.tencent.com/AndroidSDK/commandlinetools-linux-11076708_latest.zip",
        ]
    }
}

/// Check all required dependencies and hint at what to install if missing
///
/// Called before commands such as 'nefu start' and 'nefu build'
pub fn check_required_deps(target: Option<&str>) -> bool {
    let mut all_ok = true;

    #[cfg(windows)]
    {
        log::info!("checking WebView2 Runtime...");
        if !check_webview2() {
            all_ok = false;
        }
    }

    if let Some(t) = target {
        match t {
            "exe" | "app" | "bin" => {
                // Basic build targets do not need additional dependencies
            }
            _ => {}
        }
    }

    all_ok
}

/// Timer helper struct
pub struct Timer {
    start: Instant,
    label: String,
}

impl Timer {
    /// Create and start the timer
    pub fn new(label: &str) -> Self {
        Self {
            start: Instant::now(),
            label: label.to_string(),
        }
    }

    /// Get the number of elapsed milliseconds
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    /// Print the elapsed time and return
    pub fn finish(self) -> u128 {
        let ms = self.elapsed_ms();
        info!("{}: {}ms", self.label, ms);
        ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("1MB").unwrap(), 1048576);
        assert_eq!(parse_size("1.5GB").unwrap(), 1610612736);
    }

    #[test]
    fn test_parse_size_invalid() {
        assert!(parse_size("abc").is_err());
        assert!(parse_size("10XB").is_err());
    }

    #[test]
    fn test_normalize_path_sep() {
        assert_eq!(normalize_path_sep("a\\b\\c"), "a/b/c");
        assert_eq!(normalize_path_sep("a/b/c"), "a/b/c");
    }

    #[test]
    fn test_safe_join_valid() {
        let base = PathBuf::from("/project");
        assert!(safe_join(&base, "src/main.rs").is_some());
        assert!(safe_join(&base, "css/style.css").is_some());
    }

    #[test]
    fn test_safe_join_traversal() {
        let base = PathBuf::from("/project");
        assert!(safe_join(&base, "../etc/passwd").is_none());
        assert!(safe_join(&base, "foo/../../bar").is_none());
    }

    #[test]
    fn test_sha256_bytes() {
        let hash = sha256_bytes(b"hello");
        assert_eq!(hash.len(), 32);
        // SHA-256 of "hello"
        assert_eq!(
            hash.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_render_template() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "Nefu".to_string());
        vars.insert("version".to_string(), "1.0".to_string());

        let result = render_template("Hello {{name}} v{{version}}!", &vars);
        assert_eq!(result, "Hello Nefu v1.0!");
    }

    #[test]
    fn test_simple_glob_match() {
        assert!(simple_glob_match("*.html", "index.html"));
        assert!(simple_glob_match("dist/**", "dist/output/app.exe"));
        assert!(!simple_glob_match("*.css", "index.html"));
    }

    #[test]
    fn test_temp_file_path() {
        let path = temp_file_path("nefu", "tmp");
        assert!(path.to_string_lossy().contains("nefu_"));
        assert!(path.to_string_lossy().ends_with(".tmp"));
    }

    #[test]
    fn test_current_dir_safe() {
        let dir = current_dir_safe();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ms = timer.finish();
        assert!(ms >= 10);
    }

    #[test]
    fn test_colors_constants() {
        assert!(colors::RESET.contains("0m"));
        assert!(colors::RED.contains("31m"));
        assert!(colors::GREEN.contains("32m"));
    }

    #[test]
    fn test_is_nefu_project() {
        // The current test directory is unlikely to contain main.nefu
        let tmp = std::env::temp_dir();
        assert!(!is_nefu_project(&tmp));
    }

    #[test]
    fn test_relative_path() {
        let base = Path::new("/project");
        let full = Path::new("/project/src/main.rs");
        assert_eq!(relative_path(full, base), "src/main.rs");
    }
}
