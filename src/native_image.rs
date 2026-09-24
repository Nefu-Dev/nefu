//! Native image module
//!
//! Provides functionality similar to the Electron nativeImage:
//! - createFromPath(path) - create an image from a file path
//! - createFromBuffer(buffer, options) - create an image from a buffer
//! - createEmpty() - create an empty image
//! - resize(options) - resize the image
//! - crop(rect) - crop the image
//! - toDataURL() - convert to a Data URL
//! - toPNG() / toJPEG() - convert to the specified format
//! - getSize() - get the image size
//! - getBitmap() - get the bitmap data
//! - addRepresentation() - add an image representation

use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Image size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSize {
    /// Width (pixels)
    pub width: u32,
    /// Height (pixels)
    pub height: u32,
}

/// Crop region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropRect {
    /// X coordinate
    pub x: u32,
    /// Y coordinate
    pub y: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

/// Resize options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeOptions {
    /// Target width (optional)
    pub width: Option<u32>,
    /// Target height (optional)
    pub height: Option<u32>,
    /// Quality (0-100, JPEG only)
    pub quality: Option<u8>,
}

/// Native image representation
pub struct NativeImage {
    /// Image data (RGBA format)
    data: Vec<u8>,
    /// Width
    width: u32,
    /// Height
    height: u32,
    /// Whether it is a template image (for macOS light/dark mode)
    pub is_template_image: bool,
}

impl NativeImage {
    /// Create an image from a file path
    ///
    /// # Parameters
    /// - `path`: Image file path
    ///
    /// # Returns
    /// A NativeImage instance if successful
    pub fn create_from_path(path: &str) -> Result<Self> {
        let p = Path::new(path);
        if !p.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", path));
        }

        let ext = p.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let data = std::fs::read(p)?;

        // Decode using the image crate
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" => {
                let img = image::load_from_memory(&data)
                    .map_err(|e| anyhow::anyhow!("Image decoding failed: {}", e))?;
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                Ok(Self {
                    data: rgba.into_raw(),
                    width,
                    height,
                    is_template_image: false,
                })
            }
            _ => Err(anyhow::anyhow!("Unsupported image format: {}", ext)),
        }
    }

    /// Create an image from a buffer
    ///
    /// # Parameters
    /// - `buffer`: Image data buffer
    /// - `options`: Optional creation options
    ///
    /// # Returns
    /// A NativeImage instance if successful
    pub fn create_from_buffer(buffer: &[u8], options: Option<&ImageSize>) -> Result<Self> {
        // Try to decode according to the image format
        if let Ok(img) = image::load_from_memory(buffer) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            return Ok(Self {
                data: rgba.into_raw(),
                width,
                height,
                is_template_image: false,
            });
        }

        // If a size is provided, try to create from raw RGBA data
        if let Some(size) = options {
            let expected = (size.width * size.height * 4) as usize;
            if buffer.len() >= expected {
                return Ok(Self {
                    data: buffer[..expected].to_vec(),
                    width: size.width,
                    height: size.height,
                    is_template_image: false,
                });
            }
        }

        Err(anyhow::anyhow!("Unable to create an image from the buffer"))
    }

    /// Create an empty image
    ///
    /// # Parameters
    /// - `width`: Width
    /// - `height`: Height
    pub fn create_empty(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
            is_template_image: false,
        }
    }

    /// Resize the image
    ///
    /// # Parameters
    /// - `options`: Resize options
    ///
    /// # Returns
    /// A new resized image
    pub fn resize(&self, options: &ResizeOptions) -> Result<Self> {
        let new_width = options.width.unwrap_or(self.width);
        let new_height = options.height.unwrap_or(self.height);

        if new_width == 0 || new_height == 0 {
            return Err(anyhow::anyhow!("Invalid dimensions"));
        }

        // Resize using the image crate
        let img = image::RgbaImage::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Unable to create image buffer"))?;

        let resized = image::imageops::resize(
            &img,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );

        Ok(Self {
            data: resized.into_raw(),
            width: new_width,
            height: new_height,
            is_template_image: self.is_template_image,
        })
    }

    /// Crop the image
    ///
    /// # Parameters
    /// - `rect`: Crop region
    ///
    /// # Returns
    /// A new cropped image
    pub fn crop(&self, rect: &CropRect) -> Result<Self> {
        if rect.x + rect.width > self.width || rect.y + rect.height > self.height {
            return Err(anyhow::anyhow!("Crop region exceeds image bounds"));
        }

        let img = image::RgbaImage::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Unable to create image buffer"))?;

        let cropped = image::imageops::crop_imm(&img, rect.x, rect.y, rect.width, rect.height);

        Ok(Self {
            data: cropped.to_image().into_raw(),
            width: rect.width,
            height: rect.height,
            is_template_image: self.is_template_image,
        })
    }

    /// Get the image size
    pub fn get_size(&self) -> ImageSize {
        ImageSize {
            width: self.width,
            height: self.height,
        }
    }

    /// Convert to a Data URL (PNG format)
    pub fn to_data_url(&self) -> Result<String> {
        let png_data = self.to_png()?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Ok(format!("data:image/png;base64,{}", b64))
    }

    /// Convert to PNG bytes
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Unable to create image buffer"))?;

        let mut buffer = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png)?;
        Ok(buffer.into_inner())
    }

    /// Convert to JPEG bytes
    ///
    /// # Parameters
    /// - `quality`: Quality (0-100)
    pub fn to_jpeg(&self, quality: u8) -> Result<Vec<u8>> {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Unable to create image buffer"))?;

        // Convert to RGB (JPEG does not support transparency)
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();

        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
        encoder.encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)?;
        Ok(buffer.into_inner())
    }

    /// Get the bitmap data (RGBA)
    pub fn get_bitmap(&self) -> &[u8] {
        &self.data
    }

    /// Set whether it is a template image (macOS)
    pub fn set_template_image(&mut self, is_template: bool) {
        self.is_template_image = is_template;
    }

    /// Ensure the image is at least the specified size (for icons)
    ///
    /// # Parameters
    /// - `min_size`: Minimum size
    pub fn ensure_min_size(&self, min_size: u32) -> Result<Self> {
        if self.width >= min_size && self.height >= min_size {
            return Ok(Self {
                data: self.data.clone(),
                width: self.width,
                height: self.height,
                is_template_image: self.is_template_image,
            });
        }

        self.resize(&ResizeOptions {
            width: Some(min_size),
            height: Some(min_size),
            quality: None,
        })
    }

    /// Create an image thumbnail
    ///
    /// # Parameters
    /// - `max_size`: Maximum size (keeps the aspect ratio)
    pub fn create_thumbnail(&self, max_size: u32) -> Result<Self> {
        let (new_w, new_h) = if self.width > self.height {
            (max_size, (self.height * max_size / self.width).max(1))
        } else {
            ((self.width * max_size / self.height).max(1), max_size)
        };

        self.resize(&ResizeOptions {
            width: Some(new_w),
            height: Some(new_h),
            quality: None,
        })
    }
}

/// Create an image from a file path (convenience function)
pub fn create_from_path(path: &str) -> Result<NativeImage> {
    NativeImage::create_from_path(path)
}

/// Create an image from a buffer (convenience function)
pub fn create_from_buffer(buffer: &[u8]) -> Result<NativeImage> {
    NativeImage::create_from_buffer(buffer, None)
}

/// Create an empty image (convenience function)
pub fn create_empty(width: u32, height: u32) -> NativeImage {
    NativeImage::create_empty(width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty() {
        let img = NativeImage::create_empty(100, 100);
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
        assert_eq!(img.data.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_get_size() {
        let img = NativeImage::create_empty(64, 48);
        let size = img.get_size();
        assert_eq!(size.width, 64);
        assert_eq!(size.height, 48);
    }

    #[test]
    fn test_resize() {
        let img = NativeImage::create_empty(100, 100);
        let resized = img.resize(&ResizeOptions {
            width: Some(50),
            height: Some(50),
            quality: None,
        }).unwrap();
        assert_eq!(resized.width, 50);
        assert_eq!(resized.height, 50);
    }

    #[test]
    fn test_crop() {
        let img = NativeImage::create_empty(100, 100);
        let cropped = img.crop(&CropRect {
            x: 10,
            y: 10,
            width: 50,
            height: 50,
        }).unwrap();
        assert_eq!(cropped.width, 50);
        assert_eq!(cropped.height, 50);
    }

    #[test]
    fn test_crop_out_of_bounds() {
        let img = NativeImage::create_empty(100, 100);
        let result = img.crop(&CropRect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_to_data_url() {
        let img = NativeImage::create_empty(10, 10);
        let url = img.to_data_url().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_thumbnail() {
        let img = NativeImage::create_empty(200, 100);
        let thumb = img.create_thumbnail(50).unwrap();
        assert!(thumb.width <= 50);
        assert!(thumb.height <= 50);
    }

    #[test]
    fn test_set_template_image() {
        let mut img = NativeImage::create_empty(32, 32);
        assert!(!img.is_template_image);
        img.set_template_image(true);
        assert!(img.is_template_image);
    }
}
