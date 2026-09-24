//! Nefu packaging module
//!
//! Core packaging logic: collects, compresses, encrypts project files and embeds them into the executable.
//!
//! ## Binary format
//! ```text
//! [Original executable]
//! [AES-256-GCM encrypted ZIP data]
//! [8 bytes: encrypted data length (little-endian u64)]
//! [32 bytes: SHA-256 checksum]
//! [8 bytes: magic marker "NEFUPACK"]
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use lru::LruCache;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::config::NefuConfig;

// Conditional compilation: import rcedit-related functions on Windows
#[cfg(windows)]
use crate::utils::rcedit_path;

/// Magic marker used to locate the start of packaged data
const MAGIC: &[u8; 8] = b"NEFUPACK";

/// Magic marker length
const MAGIC_LEN: usize = 8;

/// SHA-256 checksum length
const CHECKSUM_LEN: usize = 32;

/// Data length field size (u64 little-endian)
const LENGTH_FIELD_SIZE: usize = 8;

/// Total trailer metadata length
const TRAILER_SIZE: usize = MAGIC_LEN + CHECKSUM_LEN + LENGTH_FIELD_SIZE;

/// AES-256 key length (bytes)
const AES_KEY_LEN: usize = 32;

/// AES-GCM nonce length (bytes)
const NONCE_LEN: usize = 12;

/// Default LRU cache capacity
const DEFAULT_CACHE_CAPACITY: usize = 64;

// ==================== Resource pack structure ====================

/// In-memory resource pack manager
///
/// Manages all resource files unpacked from the executable,
/// providing fast access with an LRU cache.
pub struct ResourcePack {
    /// Mapping of file names to file contents
    files: HashMap<String, Vec<u8>>,
    /// LRU cache to speed up frequently accessed resources
    cache: LruCache<String, Vec<u8>>,
    /// AES-256 decryption key (only present at runtime)
    decryption_key: Option<[u8; AES_KEY_LEN]>,
}

impl ResourcePack {
    /// Create an empty resource pack
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            cache: LruCache::new(
                NonZeroUsize::new(DEFAULT_CACHE_CAPACITY).unwrap(),
            ),
            decryption_key: None,
        }
    }

    /// Load the resource pack from the current executable
    ///
    /// Reads the trailer data of its own executable, verifies the magic marker,
    /// then decrypts and decompresses all resource files.
    pub fn load_from_self() -> Result<Self> {
        let exe_path = std::env::current_exe()
            .context("failed to get the current executable path")?;

        let data = std::fs::read(&exe_path)
            .context("failed to read the executable")?;

        Self::unpack(&data)
    }

    /// Unpack resources from byte data
    ///
    /// # Arguments
    /// - `data`: complete executable byte data
    ///
    /// # Returns
    /// The unpacked resource pack
    pub fn unpack(data: &[u8]) -> Result<Self> {
        // Check minimum data length
        if data.len() < TRAILER_SIZE {
            bail!("data too short, no valid packaging info");
        }

        let len_of_data = data.len();

        // Read the trailer metadata.
        // The writer layout (see build_executable) is:
        //   [payload][length u64 LE][SHA-256 32 bytes][magic 8 bytes]
        // So the last 8 bytes are the magic marker, the preceding 32 bytes are the checksum, and the 8 bytes before those are the length.
        let end = len_of_data;
        let magic_start = end - MAGIC_LEN;                 // last 8 bytes
        let checksum_start = end - MAGIC_LEN - CHECKSUM_LEN; // 40 bytes earlier
        let length_start = end - MAGIC_LEN - CHECKSUM_LEN - LENGTH_FIELD_SIZE; // 48 bytes earlier

        // Verify the magic marker
        let magic = &data[magic_start..end];
        if magic != MAGIC {
            bail!(
                "invalid magic marker: expected {:?}, got {:?}",
                MAGIC,
                magic
            );
        }

        // Read the SHA-256 checksum
        let stored_checksum = &data[checksum_start..checksum_start + CHECKSUM_LEN];

        // Read the encrypted data length
        let encrypted_len = u64::from_le_bytes(
            data[length_start..length_start + LENGTH_FIELD_SIZE]
                .try_into()
                .context("failed to read the data length field")?,
        ) as usize;

        // Validate the encrypted data length
        if encrypted_len == 0 || encrypted_len > length_start {
            bail!(
                "invalid encrypted data length: {} (available space: {})",
                encrypted_len,
                length_start
            );
        }

        // Extract the encrypted data (before the length field)
        let encrypted_start = length_start - encrypted_len;
        let encrypted_data = &data[encrypted_start..length_start];

        // Verify the SHA-256 checksum
        let mut hasher = Sha256::new();
        hasher.update(encrypted_data);
        let computed_checksum = hasher.finalize();

        if computed_checksum.as_slice() != stored_checksum {
            bail!("SHA-256 checksum mismatch, data may be corrupted");
        }

        log::info!("checksum verification passed");

        // Decrypt the data
        // The key and nonce are embedded at the front of the encrypted data.
        // For compatibility with --no-encrypt debug artifacts (plain ZIP starting with PK\x03\x04),
        // if the payload starts with a ZIP header, decompress it directly as plaintext.
        let decompressed: Vec<u8>;
        let mut decryption_key: Option<[u8; AES_KEY_LEN]> = None;

        if encrypted_data.starts_with(b"PK\x03\x04") {
            log::info!("detected plaintext ZIP (--no-encrypt debug mode), skipping decryption");
            decompressed = encrypted_data.to_vec();
        } else {
            if encrypted_data.len() < AES_KEY_LEN + NONCE_LEN {
                bail!("encrypted data too short to contain the key and nonce");
            }

            let key = &encrypted_data[..AES_KEY_LEN];
            let nonce_bytes = &encrypted_data[AES_KEY_LEN..AES_KEY_LEN + NONCE_LEN];
            let ciphertext = &encrypted_data[AES_KEY_LEN + NONCE_LEN..];

            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| anyhow::anyhow!("failed to initialize AES decryptor: {}", e))?;

            let nonce = Nonce::from_slice(nonce_bytes);

            let plaintext = cipher
                .decrypt(nonce, ciphertext)
                .map_err(|e| anyhow::anyhow!("AES-256-GCM decryption failed: {}", e))?;

            log::info!("decryption succeeded, plaintext size: {} bytes", plaintext.len());

            // Save the decryption key for later use
            let mut key_array = [0u8; AES_KEY_LEN];
            key_array.copy_from_slice(key);
            decryption_key = Some(key_array);
            decompressed = plaintext;
        }

        // Decompress the ZIP data
        let cursor = Cursor::new(decompressed);
        let mut archive = ZipArchive::new(cursor)
            .context("failed to open ZIP archive")?;

        let mut pack = Self::new();
        pack.decryption_key = decryption_key;

        // Extract all files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .with_context(|| format!("failed to read ZIP entry #{}", i))?;

            let name = file.name().to_string();

            // Skip directory entries
            if name.ends_with('/') {
                continue;
            }

            let mut content = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut content)
                .with_context(|| format!("failed to read file content: {}", name))?;

            log::debug!("unpacked file: {} ({} bytes)", name, content.len());
            pack.files.insert(name, content);
        }

        log::info!("successfully unpacked {} files", pack.files.len());
        Ok(pack)
    }

    /// Get resource file content
    ///
    /// Looks in the LRU cache first; on a miss, fetches from the file map.
    ///
    /// # Arguments
    /// - `path`: relative path of the resource file
    ///
    /// # Returns
    /// A reference to the file content, or None if the file does not exist
    pub fn get(&mut self, path: &str) -> Option<&Vec<u8>> {
        // Normalize the path
        let normalized = normalize_resource_path(path);

        // Check the cache first
        if self.cache.contains(&normalized) {
            return self.cache.get(&normalized);
        }

        // Look up in the file map
        if let Some(content) = self.files.get(&normalized) {
            // Put it into the cache
            self.cache.put(normalized.clone(), content.clone());
            self.cache.get(&normalized)
        } else {
            None
        }
    }

    /// Check whether a resource file exists
    pub fn contains(&self, path: &str) -> bool {
        let normalized = normalize_resource_path(path);
        self.files.contains_key(&normalized)
    }

    /// List all resource file paths
    pub fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    /// Get the number of resource files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get the total size of the resource pack (bytes)
    pub fn total_size(&self) -> usize {
        self.files.values().map(|v| v.len()).sum()
    }
}

// ==================== Packaging build functions ====================

/// Collect all files in the project that need to be packaged
///
/// Walks the project directory, excluding build artifacts, version control, and other unrelated files.
///
/// # Arguments
/// - `project_dir`: project root directory
/// - `config`: project configuration (contains exclusion rules)
///
/// # Returns
/// A list of (relative path, absolute path) file pairs
pub fn collect_project_files(
    project_dir: &Path,
    config: &NefuConfig,
) -> Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(project_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let abs_path = entry.path().to_path_buf();

        // Compute the path relative to the project root
        let rel_path = abs_path
            .strip_prefix(project_dir)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .replace('\\', "/");

        // Check whether the file should be excluded
        if config.is_excluded(&rel_path) {
            log::debug!("excluded file: {}", rel_path);
            continue;
        }

        // Exclude the nefu executable itself
        if let Ok(exe_path) = std::env::current_exe() {
            if abs_path == exe_path {
                continue;
            }
        }

        // Exclude the Cargo build directory
        if rel_path.starts_with("target/") {
            continue;
        }

        // Exclude C/C++ library files (do not package .h .a .lib)
        let ext = abs_path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "h" || ext == "hpp" || ext == "a" || ext == "lib" || ext == "so" || ext == "dylib" || ext == "dll" || ext == "o" || ext == "obj" {
            log::debug!("excluded library file: {}", rel_path);
            continue;
        }

        files.push((rel_path, abs_path));
    }

    log::info!("collected {} project files", files.len());
    Ok(files)
}

/// Build the final executable
///
/// Runs the full packaging pipeline:
/// 1. Compress all files into a ZIP
/// 2. Generate a random AES-256 key and nonce
/// 3. AES-256-GCM encrypt the ZIP data (unless `no_encrypt`)
/// 4. Compute the SHA-256 checksum
/// 5. Read the nefu executable itself
/// 6. Concatenate: [executable][encrypted data][length][checksum][magic marker]
/// 7. Set the application icon (if icon_path is provided)
///
/// # Arguments
/// - `files`: file list to package (relative path, absolute path)
/// - `output_path`: output executable path
/// - `no_compress`: if true, ZIP uses Store (no compression) to speed up the build
/// - `no_encrypt`: if true, skip AES encryption (debug only)
/// - `icon_path`: application icon file path (optional, used to set the exe icon)
/// - `progress`: progress callback function
pub fn build_executable(
    files: &[(String, PathBuf)],
    output_path: &Path,
    no_compress: bool,
    no_encrypt: bool,
    icon_path: Option<&Path>,
    progress: Option<&dyn Fn(usize, usize)>,
) -> Result<()> {
    log::info!("start building executable...");

    // --- Incremental build cache check ---
    let cache_dir = output_path.parent().unwrap_or_else(|| Path::new("."))
        .join(".nefu_cache");
    let cache_meta_path = cache_dir.join("cache_meta.json");
    let cache_bin_path = cache_dir.join("cached_binary.bin");

    // Compute the aggregate hash (fingerprint) of the input files
    let current_fingerprint = compute_files_fingerprint(files, no_compress, no_encrypt);

    // Try to read the cache
    if let Ok(cached_meta) = read_cache_meta(&cache_meta_path) {
        if cached_meta.fingerprint == current_fingerprint && cache_bin_path.exists() {
            log::info!("detected unchanged files, using cached artifact...");
            std::fs::copy(&cache_bin_path, output_path)
                .with_context(|| format!("failed to copy from cache to: {}", output_path.display()))?;
            log::info!("restored build artifact from cache: {}", output_path.display());
            return Ok(());
        }
    }
    // --- End cache check ---

    // Step 1: create the ZIP archive
    let zip_data = create_zip_archive(files, no_compress, progress)?;
    if let Some(cb) = progress {
        cb(files.len(), files.len());
    }
    log::info!(
        "ZIP {} done: {} bytes ({} files)",
        if no_compress { "packed (uncompressed)" } else { "compressed" },
        zip_data.len(),
        files.len()
    );

    // The resulting payload (either ciphertext or plaintext)
    let payload: Vec<u8>;

    if no_encrypt {
        // Debug mode: no encryption, use the plaintext ZIP directly
        log::warn!("encryption skipped via --no-encrypt; the generated file will contain unprotected source code");
        payload = zip_data;
    } else {
        // Step 2: generate a random key and nonce
        let mut rng = rand::thread_rng();
        let mut aes_key = [0u8; AES_KEY_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rng.fill(&mut aes_key);
        rng.fill(&mut nonce_bytes);

        log::debug!("generated AES-256 key and nonce");

        // Step 3: AES-256-GCM encryption
        let cipher = Aes256Gcm::new_from_slice(&aes_key)
            .map_err(|e| anyhow::anyhow!("failed to initialize AES encryptor: {}", e))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, zip_data.as_ref())
            .map_err(|e| anyhow::anyhow!("AES-256-GCM encryption failed: {}", e))?;

        log::info!("encryption done: {} bytes of ciphertext", ciphertext.len());

        // Assemble the encrypted package: [key][nonce][ciphertext]
        let mut encrypted_package =
            Vec::with_capacity(AES_KEY_LEN + NONCE_LEN + ciphertext.len());
        encrypted_package.extend_from_slice(&aes_key);
        encrypted_package.extend_from_slice(&nonce_bytes);
        encrypted_package.extend_from_slice(&ciphertext);
        payload = encrypted_package;
    }

    // Step 4: compute the SHA-256 checksum
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    let checksum = hasher.finalize();

    log::debug!("SHA-256 checksum: {:x}", checksum);

    // Step 5: read the nefu executable itself
    let exe_path = std::env::current_exe()
        .context("failed to get the current executable path")?;

    let exe_data = std::fs::read(&exe_path)
        .context("failed to read the nefu executable")?;

    log::info!("base executable size: {} bytes", exe_data.len());

    // Step 6: assemble the final file
    let encrypted_len = payload.len() as u64;

    let mut output = Vec::with_capacity(
        exe_data.len() + payload.len() + TRAILER_SIZE,
    );

    // Write the original executable
    output.extend_from_slice(&exe_data);

    // Write the payload (ciphertext or plaintext)
    output.extend_from_slice(&payload);

    // Write the data length (little-endian u64)
    output.extend_from_slice(&encrypted_len.to_le_bytes());

    // Write the SHA-256 checksum
    output.extend_from_slice(checksum.as_slice());

    // Write the magic marker
    output.extend_from_slice(MAGIC);

    // Write the output file
    std::fs::write(output_path, &output)
        .with_context(|| format!("failed to write the output file: {}", output_path.display()))?;

    // --- Update cache ---
    if let Ok(()) = update_cache(&cache_dir, &cache_bin_path, &cache_meta_path, &output, &current_fingerprint) {
        log::debug!("build cache updated");
    }
    // --- End cache update ---

    // Set the application icon
    if let Some(icon) = icon_path {
        if icon.exists() {
            log::info!("setting application icon: {}", icon.display());
            if let Err(e) = set_exe_icon(output_path, icon) {
                log::warn!("failed to set application icon: {} (does not affect running)", e);
            }
        } else {
            log::warn!("icon file does not exist: {}, skipping icon setup", icon.display());
        }
    }

    log::info!("build complete! total size: {} bytes", output.len());

    Ok(())
}

/// Set the icon of a Windows executable
///
/// Uses the rcedit tool to embed the icon file into the PE file.
/// Supports embedding .ico directly; formats like .png/.jpg are automatically converted to .ico.
///
/// # Arguments
/// - `exe_path`: target executable path
/// - `icon_path`: icon file path
#[cfg(windows)]
fn set_exe_icon(exe_path: &Path, icon_path: &Path) -> Result<()> {
    use std::process::Command;

    // Get the rcedit path
    let rcedit = rcedit_path().ok_or_else(|| {
        anyhow::anyhow!("rcedit not found, run `nefu build` first to download it automatically")
    })?;

    // Check the icon file format; if not .ico, try to convert it
    let ico_path = if icon_path.extension().map_or(false, |ext| {
        ext.eq_ignore_ascii_case("ico")
    }) {
        icon_path.to_path_buf()
    } else {
        // Use the image crate to convert PNG/JPG etc. to ICO
        let temp_ico = std::env::temp_dir().join(format!(
            "nefu_icon_{}.ico",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        convert_to_ico(icon_path, &temp_ico)?;
        temp_ico
    };

    // Use rcedit to set the icon
    let output = Command::new(&rcedit)
        .arg(exe_path)
        .arg("--set-icon")
        .arg(&ico_path)
        .output()
        .with_context(|| format!("failed to run rcedit: {}", rcedit.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("rcedit failed to set the icon: {}", stderr.trim());
    }

    // Clean up the temporary file
    if ico_path != icon_path {
        let _ = std::fs::remove_file(&ico_path);
    }

    log::info!("application icon set: {}", icon_path.display());
    Ok(())
}

/// Non-Windows platforms: placeholder implementation (macOS icons are handled in create_macos_app_bundle)
#[cfg(not(windows))]
fn set_exe_icon(_exe_path: &Path, icon_path: &Path) -> Result<()> {
    log::debug!("non-Windows platform, skipping exe icon setup (macOS icons are handled in the .app bundle)");
    log::info!("application icon path: {}", icon_path.display());
    Ok(())
}

/// Convert an image file to ICO format
///
/// Uses the image crate to read the image and generate an ICO file.
/// Supports PNG, JPEG, GIF, BMP, and other input formats.
#[cfg(windows)]
fn convert_to_ico(input: &Path, output: &Path) -> Result<()> {
    use image::codecs::ico::IcoFrame;
    use image::ImageEncoder;
    use image::imageops::FilterType;

    // Read the image
    let img = image::open(input)
        .with_context(|| format!("failed to read the icon file: {}", input.display()))?;

    // Generate icons at multiple sizes (Windows standard sizes)
    let sizes = [16, 24, 32, 48, 64, 128, 256];
    let mut frames: Vec<IcoFrame> = Vec::new();

    for &size in &sizes {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();

        // First encode each frame as PNG
        let mut png_bytes = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(&rgba, size, size, image::ExtendedColorType::Rgba8)
            .with_context(|| format!("PNG encoding failed: {}x{}", size, size))?;

        let frame = IcoFrame::as_png(&png_bytes, size, size, image::ExtendedColorType::Rgba8)
            .with_context(|| format!("failed to create ICO frame: {}x{}", size, size))?;
        frames.push(frame);
    }

    // Write the ICO file
    let file = std::fs::File::create(output)
        .with_context(|| format!("failed to create ICO file: {}", output.display()))?;
    let encoder = image::codecs::ico::IcoEncoder::new(file);
    encoder
        .encode_images(&frames)
        .with_context(|| format!("ICO encoding failed: {}", output.display()))?;

    log::debug!("icon converted to ICO: {} ({} sizes)", output.display(), sizes.len());
    Ok(())
}

/// Build cache metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheMeta {
    /// Aggregate hash of the input files
    fingerprint: String,
    /// Cache creation timestamp
    timestamp: u64,
}

/// Compute the aggregate fingerprint (hash) of all input files
fn compute_files_fingerprint(files: &[(String, PathBuf)], no_compress: bool, no_encrypt: bool) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    
    // Include the build settings
    let compress_flag = if no_compress { "no_compress" } else { "compress" };
    let encrypt_flag = if no_encrypt { "no_encrypt" } else { "encrypt" };
    hasher.update(compress_flag.as_bytes());
    hasher.update(encrypt_flag.as_bytes());
    
    // Sort by path to ensure determinism
    let mut sorted_files: Vec<_> = files.iter().collect();
    sorted_files.sort_by(|a, b| a.0.cmp(&b.0));
    
    for (rel_path, abs_path) in sorted_files {
        // Include the relative path
        hasher.update(rel_path.as_bytes());
        hasher.update(b"\0");
        
        // Include the file content hash
        if let Ok(content) = std::fs::read(abs_path) {
            let mut file_hasher = Sha256::new();
            file_hasher.update(&content);
            let hash = file_hasher.finalize();
            hasher.update(hash);
        }
        hasher.update(b"\0");
    }
    
    format!("{:x}", hasher.finalize())
}

/// Read the cache metadata
fn read_cache_meta(path: &Path) -> Result<CacheMeta> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).context("failed to parse cache metadata")
}

/// Update the cache
fn update_cache(cache_dir: &Path, bin_path: &Path, meta_path: &Path, data: &[u8], fingerprint: &str) -> Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(bin_path, data)?;
    
    let meta = CacheMeta {
        fingerprint: fingerprint.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let meta_json = serde_json::to_string_pretty(&meta)?;
    std::fs::write(meta_path, meta_json)?;
    
    Ok(())
}

/// Create a ZIP archive
///
/// Compresses all project files into an in-memory ZIP archive.
///
/// # Arguments
/// - `files`: file list (relative path, absolute path)
/// - `no_compress`: if true, use the Store compression method
///
/// # Returns
/// The ZIP archive bytes
fn create_zip_archive(
    files: &[(String, PathBuf)],
    no_compress: bool,
    progress: Option<&dyn Fn(usize, usize)>,
) -> Result<Vec<u8>> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buffer);

    let options = FileOptions::default()
        .compression_method(if no_compress {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        })
        .unix_permissions(0o755);

    let total = files.len();
    for (i, (rel_path, abs_path)) in files.iter().enumerate() {
        // Read the file content
        let content = std::fs::read(abs_path)
            .with_context(|| format!("failed to read the file: {}", abs_path.display()))?;

        // Add it to the ZIP archive
        zip.start_file(rel_path.clone(), options)
            .with_context(|| format!("failed to add file to ZIP: {}", rel_path))?;

        zip.write_all(&content)
            .with_context(|| format!("failed to write file to ZIP: {}", rel_path))?;

        log::trace!("added to ZIP: {} ({} bytes)", rel_path, content.len());

        if let Some(cb) = progress {
            cb(i + 1, total);
        }
    }

    // Finish the ZIP write
    let cursor = zip.finish().context("failed to finish the ZIP archive")?;
    Ok(cursor.into_inner())
}

/// Normalize a resource path
///
/// Removes leading slashes and normalizes to forward-slash separators.
///
/// # Arguments
/// - `path`: the original resource path
///
/// # Returns
/// The normalized path string
fn normalize_resource_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");

    // Remove leading slashes
    while normalized.starts_with('/') {
        normalized.remove(0);
    }

    // Handle the nefu:// protocol prefix
    if let Some(stripped) = normalized.strip_prefix("nefu://") {
        normalized = stripped.to_string();
        // Remove a possible localhost prefix
        if let Some(after_host) = normalized.strip_prefix("localhost/") {
            normalized = after_host.to_string();
        }
    }

    normalized
}

/// Verify the integrity of packaged data
///
/// Only checks the magic marker and checksum without decrypting the data.
///
/// # Arguments
/// - `data`: complete executable byte data
///
/// # Returns
/// Returns Ok(true) if the data is valid, otherwise an error description
pub fn verify_package(data: &[u8]) -> Result<bool> {
    if data.len() < TRAILER_SIZE {
        bail!("data too short");
    }

    let end = data.len();
    let magic_start = end - MAGIC_LEN;
    let checksum_start = end - MAGIC_LEN - CHECKSUM_LEN;
    let length_start = end - MAGIC_LEN - CHECKSUM_LEN - LENGTH_FIELD_SIZE;

    // Check the magic marker (last 8 bytes of the file)
    let magic = &data[magic_start..end];
    if magic != MAGIC {
        bail!("invalid magic marker");
    }

    // Read the length and checksum
    let stored_checksum = &data[checksum_start..checksum_start + CHECKSUM_LEN];

    let encrypted_len = u64::from_le_bytes(
        data[length_start..length_start + LENGTH_FIELD_SIZE]
            .try_into()?,
    ) as usize;

    if encrypted_len == 0 || encrypted_len > length_start {
        bail!("invalid data length");
    }

    // Verify the checksum
    let encrypted_start = length_start - encrypted_len;
    let encrypted_data = &data[encrypted_start..length_start];

    let mut hasher = Sha256::new();
    hasher.update(encrypted_data);
    let computed = hasher.finalize();

    Ok(computed.as_slice() == stored_checksum)
}

/// Extract packaging info (does not decrypt)
///
/// Reads packaging metadata without decrypting the actual content.
///
/// # Arguments
/// - `data`: complete executable byte data
///
/// # Returns
/// Packaging metadata (encrypted data size, checksum, etc.)
pub fn extract_package_info(data: &[u8]) -> Result<PackageInfo> {
    if data.len() < TRAILER_SIZE {
        bail!("data too short, no packaging info");
    }

    let end = data.len();
    let magic_start = end - MAGIC_LEN;
    let checksum_start = end - MAGIC_LEN - CHECKSUM_LEN;
    let length_start = end - MAGIC_LEN - CHECKSUM_LEN - LENGTH_FIELD_SIZE;

    // Verify the magic marker (last 8 bytes of the file)
    let magic = &data[magic_start..end];
    if magic != MAGIC {
        bail!("not a valid Nefu packaged file");
    }

    // Read the checksum
    let mut checksum = [0u8; CHECKSUM_LEN];
    checksum.copy_from_slice(&data[checksum_start..checksum_start + CHECKSUM_LEN]);

    // Read the data length
    let encrypted_len = u64::from_le_bytes(
        data[length_start..length_start + LENGTH_FIELD_SIZE]
            .try_into()?,
    ) as usize;

    let original_exe_size = length_start.saturating_sub(encrypted_len);

    Ok(PackageInfo {
        original_exe_size,
        encrypted_data_size: encrypted_len,
        total_size: data.len(),
        checksum_hex: hex_encode(&checksum),
    })
}

/// Packaging metadata info
#[derive(Debug, Clone)]
pub struct PackageInfo {
    /// Original executable size
    pub original_exe_size: usize,
    /// Encrypted data size
    pub encrypted_data_size: usize,
    /// Total file size
    pub total_size: usize,
    /// SHA-256 checksum (hexadecimal)
    pub checksum_hex: String,
}

/// Encode a byte array as a hexadecimal string
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Detect whether a file is a Nefu packaged file
///
/// Only checks the magic marker at the end of the file.
///
/// # Arguments
/// - `path`: the file path to check
pub fn is_nefu_package(path: &Path) -> bool {
    if let Ok(mut file) = std::fs::File::open(path) {
        // Only need to read the last 8 bytes of the file
        use std::io::Seek;
        if file.seek(std::io::SeekFrom::End(-(MAGIC_LEN as i64))).is_ok() {
            let mut magic = [0u8; MAGIC_LEN];
            if file.read_exact(&mut magic).is_ok() {
                return &magic == MAGIC;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_resource_path() {
        assert_eq!(normalize_resource_path("/index.html"), "index.html");
        assert_eq!(normalize_resource_path("css/style.css"), "css/style.css");
        assert_eq!(
            normalize_resource_path("nefu://localhost/index.html"),
            "index.html"
        );
        assert_eq!(
            normalize_resource_path("path\\to\\file.txt"),
            "path/to/file.txt"
        );
    }

    #[test]
    fn test_magic_constant() {
        assert_eq!(MAGIC, b"NEFUPACK");
        assert_eq!(MAGIC_LEN, 8);
    }

    #[test]
    fn test_trailer_size() {
        assert_eq!(TRAILER_SIZE, 8 + 32 + 8); // magic + checksum + length
        assert_eq!(TRAILER_SIZE, 48);
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(hex_encode(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn test_verify_package_too_short() {
        let data = vec![0u8; 10];
        assert!(verify_package(&data).is_err());
    }

    #[test]
    fn test_verify_package_invalid_magic() {
        let mut data = vec![0u8; 100];
        // Set an invalid magic marker
        data[92..100].copy_from_slice(b"INVALID!");
        assert!(verify_package(&data).is_err());
    }

    #[test]
    fn test_resource_pack_new() {
        let pack = ResourcePack::new();
        assert_eq!(pack.file_count(), 0);
        assert_eq!(pack.total_size(), 0);
    }

    #[test]
    fn test_is_nefu_package_nonexistent() {
        assert!(!is_nefu_package(Path::new("/nonexistent/file")));
    }

    #[test]
    fn test_create_zip_archive_empty() {
        let files: Vec<(String, PathBuf)> = Vec::new();
        let result = create_zip_archive(&files, false, None);
        assert!(result.is_ok());

        let result_store = create_zip_archive(&files, true, None);
        assert!(result_store.is_ok());
    }

    #[test]
    fn test_extract_package_info_too_short() {
        let data = vec![0u8; 10];
        assert!(extract_package_info(&data).is_err());
    }

    /// End-to-end regression: the packaged artifact must be correctly parsed by unpack / verify / extract.
    /// This previously failed because the writer layout `[len][checksum][magic]` differed from the reader layout `[magic][checksum][len]`,
    /// so the packaged exe could not be unpacked after double-clicking ("invalid magic marker").
    #[test]
    fn test_build_then_unpack_roundtrip() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!("nefu_rt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Generate the "host exe" external file content
        let host_path = dir.join("host.bin");
        let mut fh = std::fs::File::create(&host_path).unwrap();
        fh.write_all(b"MZ fake host executable").unwrap();

        // Fake project files
        let src1 = dir.join("index.html");
        std::fs::write(&src1, "<html>hi</html>").unwrap();
        let src2 = dir.join("main.nefu");
        std::fs::write(&src2, "entry = \"index.html\"\noutput = \"t\"\n").unwrap();

        let files = vec![
            ("index.html".to_string(), src1),
            ("main.nefu".to_string(), src2),
        ];

        let out_path = dir.join("out.exe");

        // Note: build_executable reads its own current_exe; tests run as the test binary,
        // which does not affect format verification.
        build_executable(&files, &out_path, false, false, None).unwrap();

        let data = std::fs::read(&out_path).unwrap();

        // The magic number should be at the end of the file
        assert_eq!(&data[data.len() - MAGIC_LEN..], MAGIC);
        assert!(is_nefu_package(&out_path));

        // Both verify and extract should recognize it
        assert!(verify_package(&data).unwrap());

        let info = extract_package_info(&data).unwrap();
        assert!(info.total_size == data.len());
        assert!(info.encrypted_data_size > 0);

        // unpack must extract both files
        let mut pack = ResourcePack::unpack(&data).unwrap();
        assert_eq!(pack.file_count(), 2);
        assert!(pack.get("index.html").is_some());
        assert!(pack.get("main.nefu").is_some());
        assert_eq!(
            pack.get("index.html").unwrap().as_slice(),
            b"<html>hi</html>"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
