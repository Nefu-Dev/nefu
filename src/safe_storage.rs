//! Secure storage module
//!
//! Provides functionality similar to the Electron safeStorage:
//! - encryptString(plainText) - encrypt a string
//! - decryptString(encrypted) - decrypt a string
//! - isEncryptionAvailable() - check whether encryption is available
//! - Uses the system keychain or encrypted file storage

use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Encrypted data type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Encryption algorithm identifier
    pub algorithm: String,
    /// Initialization vector (hex)
    pub iv: String,
    /// Encrypted data (base64)
    pub data: String,
    /// Authentication tag (hex)
    pub tag: String,
    /// Salt (hex)
    pub salt: String,
}

/// Secure storage manager
///
/// Uses AES-256-GCM encryption; key derivation uses PBKDF2.
/// The key is stored in the user data directory.
pub struct SafeStorage {
    /// Storage directory
    storage_dir: PathBuf,
    /// Encryption key
    encryption_key: Option<Vec<u8>>,
}

impl SafeStorage {
    /// Create a new secure storage manager
    ///
    /// # Parameters
    /// - `app_name`: Application name (used to generate the key file path)
    pub fn new(app_name: &str) -> Self {
        let storage_dir = get_safe_storage_dir(app_name);
        let key = load_or_create_key(&storage_dir);
        Self {
            storage_dir,
            encryption_key: key.ok(),
        }
    }

    /// Check whether encryption is available
    pub fn is_encryption_available(&self) -> bool {
        self.encryption_key.is_some()
    }

    /// Encrypt a string
    ///
    /// # Parameters
    /// - `plain_text`: The plaintext to encrypt
    ///
    /// # Returns
    /// The encrypted data (contains all information needed for decryption)
    pub fn encrypt_string(&self, plain_text: &str) -> Result<EncryptedData> {
        let key = self.encryption_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Encryption key unavailable"))?;

        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Nonce};
        use rand::RngCore;

        // Generate a random salt
        let mut salt = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut salt);

        // Derive the key using PBKDF2
        let mut derived_key = [0u8; 32];
        pbkdf2_hmac_sha256(key, &salt, &mut derived_key);

        // Create an AES-256-GCM instance
        let cipher = Aes256Gcm::new_from_slice(&derived_key)
            .map_err(|e| anyhow::anyhow!("Failed to create encryptor: {}", e))?;

        // Generate a random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plain_text.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Split off the authentication tag (last 16 bytes)
        let tag_start = ciphertext.len() - 16;
        let encrypted_data = &ciphertext[..tag_start];
        let tag = &ciphertext[tag_start..];

        Ok(EncryptedData {
            algorithm: "AES-256-GCM".to_string(),
            iv: hex::encode(&nonce_bytes),
            data: base64::engine::general_purpose::STANDARD.encode(encrypted_data),
            tag: hex::encode(tag),
            salt: hex::encode(&salt),
        })
    }

    /// Decrypt a string
    ///
    /// # Parameters
    /// - `encrypted`: Encrypted data
    ///
    /// # Returns
    /// The decrypted plaintext
    pub fn decrypt_string(&self, encrypted: &EncryptedData) -> Result<String> {
        let key = self.encryption_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Encryption key unavailable"))?;

        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Nonce};

        // Decode the parameters
        let iv = hex::decode(&encrypted.iv)
            .map_err(|e| anyhow::anyhow!("IV decoding failed: {}", e))?;
        let ciphertext_b64 = base64::engine::general_purpose::STANDARD
            .decode(&encrypted.data)
            .map_err(|e| anyhow::anyhow!("Data decoding failed: {}", e))?;
        let tag = hex::decode(&encrypted.tag)
            .map_err(|e| anyhow::anyhow!("Tag decoding failed: {}", e))?;
        let salt = hex::decode(&encrypted.salt)
            .map_err(|e| anyhow::anyhow!("Salt decoding failed: {}", e))?;

        // Derive the key
        let mut derived_key = [0u8; 32];
        pbkdf2_hmac_sha256(key, &salt, &mut derived_key);

        // Create the decryptor
        let cipher = Aes256Gcm::new_from_slice(&derived_key)
            .map_err(|e| anyhow::anyhow!("Failed to create decryptor: {}", e))?;

        let nonce = Nonce::from_slice(&iv);

        // Combine the ciphertext and authentication tag
        let mut ciphertext_with_tag = ciphertext_b64;
        ciphertext_with_tag.extend_from_slice(&tag);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("Failed to decode decrypted result: {}", e))
    }
}

/// PBKDF2-HMAC-SHA256 key derivation
fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], output: &mut [u8]) {
    use sha2::Sha256;

    // Simplified implementation: iterate HMAC-SHA256
    let mut derived = Vec::from(password);
    for _ in 0..100_000 {
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<Sha256>::new_from_slice(&derived)
            .expect("HMAC creation failed");
        mac.update(salt);
        derived = mac.finalize().into_bytes().to_vec();
    }

    let len = output.len().min(derived.len());
    output[..len].copy_from_slice(&derived[..len]);
}

/// Get the secure storage directory
fn get_safe_storage_dir(app_name: &str) -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local/share"))
            .unwrap_or_else(|_| PathBuf::from("."))
    };

    base.join(app_name).join("safe_storage")
}

/// Load or create the encryption key
fn load_or_create_key(storage_dir: &PathBuf) -> Result<Vec<u8>> {
    std::fs::create_dir_all(storage_dir)?;

    let key_path = storage_dir.join("encryption.key");

    if key_path.exists() {
        // Load the existing key
        let key_data = std::fs::read(&key_path)?;
        if key_data.len() == 32 {
            return Ok(key_data);
        }
        log::warn!("Key file has an invalid length, regenerating");
    }

    // Generate a new key
    let mut key = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut key);

    std::fs::write(&key_path, &key)?;
    log::info!("New encryption key generated: {}", key_path.display());

    Ok(key.to_vec())
}

/// Check whether encryption is available (convenience function)
pub fn is_encryption_available() -> bool {
    let storage = SafeStorage::new("nefu");
    storage.is_encryption_available()
}

/// Encrypt a string (convenience function)
pub fn encrypt_string(plain_text: &str) -> Result<EncryptedData> {
    let storage = SafeStorage::new("nefu");
    storage.encrypt_string(plain_text)
}

/// Decrypt a string (convenience function)
pub fn decrypt_string(encrypted: &EncryptedData) -> Result<String> {
    let storage = SafeStorage::new("nefu");
    storage.decrypt_string(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let storage = SafeStorage::new("nefu_test");
        if !storage.is_encryption_available() {
            // Skip the test if encryption is unavailable
            return;
        }

        let original = "Hello, Nefu Safe Storage!";
        let encrypted = storage.encrypt_string(original).unwrap();
        let decrypted = storage.decrypt_string(&encrypted).unwrap();

        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_encrypted_data_structure() {
        let storage = SafeStorage::new("nefu_test");
        if !storage.is_encryption_available() {
            return;
        }

        let data = storage.encrypt_string("test").unwrap();
        assert_eq!(data.algorithm, "AES-256-GCM");
        assert!(!data.iv.is_empty());
        assert!(!data.data.is_empty());
        assert!(!data.tag.is_empty());
        assert!(!data.salt.is_empty());
    }

    #[test]
    fn test_different_encryptions_produce_different_output() {
        let storage = SafeStorage::new("nefu_test");
        if !storage.is_encryption_available() {
            return;
        }

        let data1 = storage.encrypt_string("same text").unwrap();
        let data2 = storage.encrypt_string("same text").unwrap();

        // Due to the random nonce, each encryption result should differ
        assert_ne!(data1.data, data2.data);
        assert_ne!(data1.iv, data2.iv);
    }

    #[test]
    fn test_is_encryption_available() {
        // Just verify that it can be called
        let _ = is_encryption_available();
    }
}
