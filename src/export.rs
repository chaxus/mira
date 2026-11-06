//! Export: clipboard + save to PNG file.

use std::path::Path;

use arboard::{Clipboard, ImageData};
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use thiserror::Error;

/// Where a finished capture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportTarget {
    #[default]
    Clipboard,
    File,
    PinToScreen,
    ExtractText,
}

impl ExportTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clipboard => "clipboard",
            Self::File => "file",
            Self::PinToScreen => "pin",
            Self::ExtractText => "ocr",
        }
    }
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("clipboard: {0}")]
    Clipboard(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("encode: {0}")]
    Encode(String),
    #[error("not supported yet: {0}")]
    Unsupported(&'static str),
}

/// Copy RGBA image to the system clipboard as PNG pixels (arboard raw RGBA).
pub fn copy_to_clipboard(img: &RgbaImage) -> Result<(), ExportError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| ExportError::Clipboard(e.to_string()))?;
    let data = ImageData {
        width: img.width() as usize,
        height: img.height() as usize,
        bytes: std::borrow::Cow::Borrowed(img.as_raw()),
    };
    clipboard
        .set_image(data)
        .map_err(|e| ExportError::Clipboard(e.to_string()))
}

/// Write PNG to `path`.
pub fn save_png(img: &RgbaImage, path: &Path) -> Result<(), ExportError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let encoder = PngEncoder::new(file);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(())
}

/// Default save path under the user's Pictures (or temp) directory.
pub fn default_save_path() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut dir = dirs_pictures();
    dir.push(format!("Mira-{stamp}.png"));
    dir
}

fn dirs_pictures() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let pictures = std::path::PathBuf::from(home).join("Pictures");
        if pictures.is_dir() {
            return pictures;
        }
    }
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn save_png_roundtrip_header() {
        let img = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.png");
        save_png(&img, &path).unwrap();
        assert!(path.metadata().unwrap().len() > 8);
    }
}
