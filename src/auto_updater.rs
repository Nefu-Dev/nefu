//! Auto-update module
//!
//! Provides functionality similar to the Electron autoUpdater:
//! - Check for updates
//! - Download updates in the background
//! - Update progress callbacks
//! - Install updates

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Update status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// Checking
    Checking,
    /// New version available
    UpdateAvailable,
    /// Already up to date
    UpToDate,
    /// Downloading
    Downloading,
    /// Download complete
    Downloaded,
    /// Check failed
    Error(String),
}

/// Update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// New version number
    pub version: String,
    /// Release date
    pub release_date: Option<String>,
    /// Release notes
    pub release_notes: Option<String>,
    /// Download URL
    pub download_url: String,
    /// File size (bytes)
    pub file_size: Option<u64>,
}

/// Progress callback
pub type ProgressCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Update checker
pub struct AutoUpdater {
    /// Update URL
    update_url: String,
    /// Current version
    current_version: String,
    /// Whether currently downloading
    downloading: Arc<AtomicBool>,
    /// Download progress callback
    progress_callback: Option<ProgressCallback>,
    /// Latest update information
    latest_update: Option<UpdateInfo>,
}

impl AutoUpdater {
    /// Create an update checker
    ///
    /// # Parameters
    /// - `update_url`: Update check URL
    /// - `current_version`: Current version number
    pub fn new(update_url: String, current_version: String) -> Self {
        Self {
            update_url,
            current_version,
            downloading: Arc::new(AtomicBool::new(false)),
            progress_callback: None,
            latest_update: None,
        }
    }

    /// Set the download progress callback
    ///
    /// # Parameters
    /// - `callback`: Progress callback function (downloaded bytes, total bytes)
    pub fn set_progress_callback<F>(&mut self, callback: F)
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
    }

    /// Check for updates (synchronous)
    ///
    /// # Returns
    /// Update information (if an update is available)
    pub fn check_for_updates(&mut self) -> Result<UpdateStatus> {
        log::info!("Checking for updates: {}", self.update_url);

        let body = match ureq::get(&self.update_url)
            .timeout(std::time::Duration::from_secs(15))
            .call()
        {
            Ok(resp) => resp.into_string().unwrap_or_default(),
            Err(e) => {
                log::debug!("Update check failed: {}", e);
                return Ok(UpdateStatus::Error(format!("Check failed: {}", e)));
            }
        };

        if body.is_empty() {
            return Ok(UpdateStatus::Error("Empty response".to_string()));
        }

        // Parse the JSON only once to avoid repeated parsing
        let json: Option<serde_json::Value> = serde_json::from_str(&body).ok();

        let remote_version = json
            .as_ref()
            .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(String::from))
            .unwrap_or_else(|| body.trim().to_string());

        if remote_version.is_empty() {
            return Ok(UpdateStatus::Error("Unable to parse version number".to_string()));
        }

        // Compare version numbers
        if remote_version == self.current_version {
            log::info!("Already up to date: {}", self.current_version);
            return Ok(UpdateStatus::UpToDate);
        }

        log::info!("New version found: {} (current: {})", remote_version, self.current_version);

        // Extract update information from the parsed JSON
        let release_notes = json
            .as_ref()
            .and_then(|v| v.get("notes").and_then(|x| x.as_str()).map(String::from));

        let download_url = json
            .as_ref()
            .and_then(|v| v.get("download_url").and_then(|x| x.as_str()).map(String::from))
            .unwrap_or_else(|| self.update_url.clone());

        let file_size = json
            .as_ref()
            .and_then(|v| v.get("file_size").and_then(|x| x.as_u64()));

        let release_date = json
            .as_ref()
            .and_then(|v| v.get("release_date").and_then(|x| x.as_str()).map(String::from));

        // Cache the latest update information
        self.latest_update = Some(UpdateInfo {
            version: remote_version,
            release_date,
            release_notes,
            download_url,
            file_size,
        });

        Ok(UpdateStatus::UpdateAvailable)
    }

    /// Check for updates on a background thread
    ///
    /// # Parameters
    /// - `callback`: Result callback
    pub fn check_for_updates_async<F>(&self, callback: F)
    where
        F: Fn(UpdateStatus) + Send + 'static,
    {
        let update_url = self.update_url.clone();
        let current_version = self.current_version.clone();

        std::thread::spawn(move || {
            let mut updater = AutoUpdater::new(update_url, current_version);
            let status = updater.check_for_updates().unwrap_or(UpdateStatus::Error("Check failed".to_string()));
            callback(status);
        });
    }

    /// Download an update (synchronous)
    ///
    /// # Parameters
    /// - `url`: Download URL
    /// - `target_path`: Save path
    ///
    /// # Returns
    /// Whether the download succeeded
    pub fn download_update(&self, url: &str, target_path: &std::path::Path) -> Result<bool> {
        if self.downloading.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Download in progress"));
        }

        self.downloading.store(true, Ordering::SeqCst);
        let result = self.download_internal(url, target_path);
        self.downloading.store(false, Ordering::SeqCst);
        result
    }

    /// Internal download implementation (streaming write, supports binary files)
    fn download_internal(&self, url: &str, target_path: &std::path::Path) -> Result<bool> {
        log::info!("Starting update download: {}", url);

        // Cache check: avoid downloading again
        if target_path.exists() {
            if let Ok(meta) = std::fs::metadata(target_path) {
                if meta.len() > 0 {
                    log::info!("Update file already exists, skipping download: {}", target_path.display());
                    return Ok(true);
                }
            }
            let _ = std::fs::remove_file(target_path);
        }

        // Ensure the parent directory exists
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let response = ureq::get(url)
            .timeout(std::time::Duration::from_secs(600))
            .call()
            .context("Download request failed")?;

        // Get total size
        let total_size = response
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        // Stream-write to the file to avoid memory overflow with large files and binary corruption
        let mut file = std::fs::File::create(target_path)
            .with_context(|| format!("Failed to create file: {}", target_path.display()))?;

        let mut reader = response.into_reader();
        let mut downloaded: u64 = 0;
        let mut buf = [0u8; 65536];

        loop {
            let n = reader
                .read(&mut buf)
                .context("Failed to read download content")?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .context("Failed to write update file")?;
            downloaded += n as u64;

            // Progress callback
            if let Some(cb) = &self.progress_callback {
                cb(downloaded, total_size);
            }
        }

        file.flush()?;
        drop(file);

        log::info!("Update download complete: {} ({} bytes)", target_path.display(), downloaded);
        Ok(true)
    }

    /// Get the latest update information
    pub fn get_latest_update(&self) -> Option<&UpdateInfo> {
        self.latest_update.as_ref()
    }

    /// Whether currently downloading
    pub fn is_downloading(&self) -> bool {
        self.downloading.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_status_serde() {
        let status = UpdateStatus::Checking;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"checking\"");

        let status = UpdateStatus::Error("Network error".to_string());
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "{\"error\":\"Network error\"}");
    }

    #[test]
    fn test_auto_updater_creation() {
        let updater = AutoUpdater::new(
            "https://example.com/update.json".to_string(),
            "1.0.0".to_string(),
        );
        assert!(!updater.is_downloading());
        assert!(updater.get_latest_update().is_none());
    }
}
