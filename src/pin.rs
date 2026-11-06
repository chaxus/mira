//! Pin a capture above other windows (Lark "固定到屏幕").

use std::path::PathBuf;
use std::process::Command;

use image::RgbaImage;
use thiserror::Error;

use crate::export;

#[derive(Debug, Error)]
pub enum PinError {
    #[error("export: {0}")]
    Export(#[from] export::ExportError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Float `img` above other windows. macOS uses Quick Look (`qlmanage -p`);
/// other platforms open the PNG with the default viewer as a fallback.
pub fn pin_to_screen(img: &RgbaImage) -> Result<PathBuf, PinError> {
    let path = pin_path();
    export::save_png(img, &path)?;
    platform_pin(&path)?;
    Ok(path)
}

fn pin_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mira-pin-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    path
}

#[cfg(target_os = "macos")]
fn platform_pin(path: &std::path::Path) -> Result<(), PinError> {
    // Quick Look panel floats above other windows and is dismissible — closest
    // stock equivalent to Lark's pin without a custom NSPanel yet.
    Command::new("qlmanage")
        .args(["-p", &path.to_string_lossy()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| PinError::Message(format!("qlmanage failed: {e}")))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn platform_pin(path: &std::path::Path) -> Result<(), PinError> {
    Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .spawn()
        .map_err(|e| PinError::Message(format!("open failed: {e}")))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_pin(path: &std::path::Path) -> Result<(), PinError> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| PinError::Message(format!("xdg-open failed: {e}")))?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_pin(_path: &std::path::Path) -> Result<(), PinError> {
    Err(PinError::Message("pin not supported on this OS".into()))
}
