//! Screen capture — macOS `screencapture`, Windows / Linux `xcap` + desktop tools.

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
///
/// Hides Mira's own windows for the duration (best-effort), matching Lark's
/// “hold Shift to hide the app” behaviour.
///
/// `Scroll` is handled by [`crate::scroll::ScrollSession`] — calling this
/// with [`CaptureMode::Scroll`] returns [`CaptureError::Unsupported`].
pub fn capture(mode: CaptureMode) -> Result<RgbaImage, CaptureError> {
    let _hidden = crate::chrome::HideGuard::acquire();
    match mode {
        CaptureMode::Scroll => Err(CaptureError::Unsupported("scroll")),
        CaptureMode::Region | CaptureMode::Fullscreen => capture_png(mode),
    }
}

/// Fullscreen capture used by the scroll stitcher (caller owns chrome hide).
pub fn platform_capture_fullscreen_for_scroll(path: &std::path::Path) -> Result<(), CaptureError> {
    platform_capture(CaptureMode::Fullscreen, path)
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
    match mode {
        CaptureMode::Fullscreen => windows_fullscreen(path),
        CaptureMode::Region => windows_region(path),
        CaptureMode::Scroll => Err(CaptureError::Unsupported("scroll")),
    }
}

/// Primary monitor via `xcap`.
#[cfg(target_os = "windows")]
fn windows_fullscreen(path: &std::path::Path) -> Result<(), CaptureError> {
    use xcap::Monitor;

    let monitor = Monitor::from_point(0, 0).map_err(|e| CaptureError::Message(e.to_string()))?;
    let image = monitor
        .capture_image()
        .map_err(|e| CaptureError::Message(e.to_string()))?;
    image
        .save(path)
        .map_err(|e| CaptureError::Image(e.to_string()))?;
    Ok(())
}

/// Opens the system snipping overlay (`ms-screenclip:`), then reads the
/// resulting image from the clipboard into `path`.
#[cfg(target_os = "windows")]
fn windows_region(path: &std::path::Path) -> Result<(), CaptureError> {
    use std::time::{Duration, Instant};

    use arboard::Clipboard;

    let _ = Command::new("explorer")
        .arg("ms-screenclip:")
        .spawn()
        .map_err(|e| CaptureError::Message(format!("failed to open snipping tool: {e}")))?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut clipboard =
        Clipboard::new().map_err(|e| CaptureError::Message(e.to_string()))?;
    // Give the overlay a moment to steal focus before we start polling.
    std::thread::sleep(Duration::from_millis(400));
    while Instant::now() < deadline {
        if let Ok(img) = clipboard.get_image() {
            let rgba = image::RgbaImage::from_raw(
                img.width as u32,
                img.height as u32,
                img.bytes.into_owned(),
            )
            .ok_or_else(|| CaptureError::Image("invalid clipboard image".into()))?;
            rgba.save(path)
                .map_err(|e| CaptureError::Image(e.to_string()))?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(CaptureError::Cancelled)
}

#[cfg(target_os = "linux")]
fn platform_capture(mode: CaptureMode, path: &std::path::Path) -> Result<(), CaptureError> {
    match mode {
        CaptureMode::Fullscreen => linux_fullscreen(path),
        CaptureMode::Region => linux_region(path),
        CaptureMode::Scroll => Err(CaptureError::Unsupported("scroll")),
    }
}

#[cfg(target_os = "linux")]
fn linux_fullscreen(path: &std::path::Path) -> Result<(), CaptureError> {
    use xcap::Monitor;

    let monitor = Monitor::from_point(0, 0).map_err(|e| CaptureError::Message(e.to_string()))?;
    let image = monitor
        .capture_image()
        .map_err(|e| CaptureError::Message(e.to_string()))?;
    image
        .save(path)
        .map_err(|e| CaptureError::Image(e.to_string()))?;
    Ok(())
}

/// Prefer Wayland `grim`+`slurp`, then GNOME / KDE / ImageMagick helpers.
#[cfg(target_os = "linux")]
fn linux_region(path: &std::path::Path) -> Result<(), CaptureError> {
    let path_str = path.to_string_lossy().into_owned();

    // grim -g "$(slurp)" out.png
    if which("slurp") && which("grim") {
        let geom = Command::new("slurp")
            .output()
            .map_err(|e| CaptureError::Message(e.to_string()))?;
        if !geom.status.success() {
            return Err(CaptureError::Cancelled);
        }
        let geometry = String::from_utf8_lossy(&geom.stdout).trim().to_owned();
        if geometry.is_empty() {
            return Err(CaptureError::Cancelled);
        }
        let status = Command::new("grim")
            .args(["-g", &geometry, &path_str])
            .status()?;
        return finish_tool_status(status, path);
    }

    if which("gnome-screenshot") {
        let status = Command::new("gnome-screenshot")
            .args(["-a", "-f", &path_str])
            .status()?;
        return finish_tool_status(status, path);
    }

    if which("spectacle") {
        let status = Command::new("spectacle")
            .args(["-b", "-r", "-o", &path_str])
            .status()?;
        return finish_tool_status(status, path);
    }

    if which("maim") && which("slop") {
        let status = Command::new("bash")
            .args([
                "-lc",
                &format!("maim -s {}", shell_escape(&path_str)),
            ])
            .status()?;
        return finish_tool_status(status, path);
    }

    Err(CaptureError::Message(
        "no region capture tool found (install grim+slurp, gnome-screenshot, spectacle, or maim)"
            .into(),
    ))
}

#[cfg(target_os = "linux")]
fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "linux")]
fn finish_tool_status(
    status: std::process::ExitStatus,
    path: &std::path::Path,
) -> Result<(), CaptureError> {
    if path.exists() {
        return Ok(());
    }
    if status.success() {
        return Err(CaptureError::Message("capture tool reported success but wrote no file".into()));
    }
    Err(CaptureError::Cancelled)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_capture(_mode: CaptureMode, _path: &std::path::Path) -> Result<(), CaptureError> {
    Err(CaptureError::Unsupported("this OS"))
}
