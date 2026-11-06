//! Screen capture (P1: macOS `screencapture`; other platforms stubbed).

use std::path::PathBuf;
use std::process::Command;

use image::RgbaImage;
use thiserror::Error;

/// How Mira grabs pixels before annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureMode {
    /// Interactive region select (Lark default).
    #[default]
    Region,
    /// Entire main display.
    Fullscreen,
    /// Long / scrolling capture (P2).
    Scroll,
}

impl CaptureMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Fullscreen => "fullscreen",
            Self::Scroll => "scroll",
        }
    }
}

/// Capture failures.
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture cancelled")]
    Cancelled,
    #[error("mode not supported yet: {0}")]
    Unsupported(&'static str),
    #[error("permission denied or capture tool missing")]
    PermissionDenied,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("image: {0}")]
    Image(String),
    #[error("{0}")]
    Message(String),
}

/// Run a capture for `mode` and return RGBA pixels.
pub fn capture(mode: CaptureMode) -> Result<RgbaImage, CaptureError> {
    match mode {
        CaptureMode::Scroll => Err(CaptureError::Unsupported("scroll")),
        CaptureMode::Region | CaptureMode::Fullscreen => capture_png(mode),
    }
}

fn capture_png(mode: CaptureMode) -> Result<RgbaImage, CaptureError> {
    let path = temp_png_path()?;
    platform_capture(mode, &path)?;
    let img = image::open(&path)
        .map_err(|e| CaptureError::Image(e.to_string()))?
        .to_rgba8();
    let _ = std::fs::remove_file(&path);
    Ok(img)
}

fn temp_png_path() -> Result<PathBuf, CaptureError> {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mira-capture-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    Ok(path)
}

#[cfg(target_os = "macos")]
fn platform_capture(mode: CaptureMode, path: &std::path::Path) -> Result<(), CaptureError> {
    // -x: no sound; -t png; -i: interactive region (Lark-like drag).
    let mut cmd = Command::new("screencapture");
    cmd.arg("-x").arg("-t").arg("png");
    match mode {
        CaptureMode::Region => {
            cmd.arg("-i");
        }
        CaptureMode::Fullscreen => {}
        CaptureMode::Scroll => return Err(CaptureError::Unsupported("scroll")),
    }
    cmd.arg(path);
    let status = cmd.status()?;
    if !status.success() {
        // User pressed Esc in interactive mode, or Screen Recording permission denied.
        if !path.exists() {
            return Err(CaptureError::Cancelled);
        }
        return Err(CaptureError::PermissionDenied);
    }
    if !path.exists() {
        return Err(CaptureError::Cancelled);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn platform_capture(mode: CaptureMode, path: &std::path::Path) -> Result<(), CaptureError> {
    let _ = (mode, path);
    Err(CaptureError::Unsupported(
        "windows capture lands in P1.1 — use macOS for now",
    ))
}

#[cfg(target_os = "linux")]
fn platform_capture(mode: CaptureMode, path: &std::path::Path) -> Result<(), CaptureError> {
    let _ = (mode, path);
    Err(CaptureError::Unsupported(
        "linux capture lands in P1.1 — use macOS for now",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_capture(_mode: CaptureMode, _path: &std::path::Path) -> Result<(), CaptureError> {
    Err(CaptureError::Unsupported("this OS"))
}
