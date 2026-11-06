//! Screen recording — native OS capture, encode/mux via trimmed ffmpeg when present.
//!
//! See `docs/TECH_PLAN.md` (option B). Capture still uses system tools
//! (`screencapture -v` on macOS). Finalize prefers bundled/PATH ffmpeg for
//! `.mp4` / `.gif`, then macOS `avconvert`, then keeps the raw `.mov`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use thiserror::Error;

use crate::ffmpeg;

/// Container for a recording session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOptions {
    pub show_cursor: bool,
    pub format: RecordFormat,
    /// When true, use interactive region select (macOS `screencapture -i`).
    pub region: bool,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            show_cursor: true,
            format: RecordFormat::Mp4,
            region: true,
        }
    }
}

/// Output format for recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordFormat {
    /// System QuickTime / ScreenCapture container.
    Mov,
    /// Prefer ffmpeg remux/transcode; fall back to `avconvert` / raw `.mov`.
    #[default]
    Mp4,
    /// Post-process raw capture to GIF via ffmpeg (requires side-car or PATH).
    Gif,
}

impl RecordFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mov => "mov",
            Self::Mp4 => "mp4",
            Self::Gif => "gif",
        }
    }
}

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Active recording process.
pub struct RecordSession {
    child: Child,
    /// Path where the OS writer places media (`.mov` on macOS).
    raw_path: PathBuf,
    options: RecordOptions,
}

impl RecordSession {
    /// Start a screen recording via the platform native path.
    ///
    /// Always captures to a system container first; `format` is applied in [`stop`].
    pub fn start(options: RecordOptions) -> Result<Self, RecordError> {
        if options.format == RecordFormat::Gif && !ffmpeg::available() {
            return Err(RecordError::Message(
                "gif needs ffmpeg — run scripts/fetch-ffmpeg.sh or set MIRA_FFMPEG / PATH".into(),
            ));
        }
        let raw_path = raw_output_path();
        let child = platform_start(&options, &raw_path)?;
        Ok(Self {
            child,
            raw_path,
            options,
        })
    }

    pub fn raw_path(&self) -> &Path {
        &self.raw_path
    }

    /// Stop recording and return the final media path.
    pub fn stop(mut self) -> Result<PathBuf, RecordError> {
        // Prefer a graceful interrupt so screencapture can flush the file.
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-INT", &self.child.id().to_string()])
                .status();
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        let _ = self.child.kill();
        let status = self.child.wait()?;
        if !self.raw_path.exists() {
            if status.success() {
                return Err(RecordError::Message(
                    "recording ended but no file was written".into(),
                ));
            }
            return Err(RecordError::Message(
                "recording cancelled or permission denied".into(),
            ));
        }
        finalize(&self.raw_path, self.options.format)
    }
}

fn raw_output_path() -> PathBuf {
    let mut path = media_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    path.push(format!("Mira-{stamp}.mov"));
    path
}

fn media_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let movies = PathBuf::from(&home).join("Movies");
        if movies.is_dir() {
            return movies;
        }
        let videos = PathBuf::from(home).join("Videos");
        if videos.is_dir() {
            return videos;
        }
    }
    std::env::temp_dir()
}

fn finalize(raw: &Path, format: RecordFormat) -> Result<PathBuf, RecordError> {
    match format {
        RecordFormat::Mov => Ok(raw.to_path_buf()),
        RecordFormat::Mp4 => remux_mp4(raw),
        RecordFormat::Gif => convert_gif(raw),
    }
}

fn remux_mp4(raw: &Path) -> Result<PathBuf, RecordError> {
    let mut mp4 = raw.to_path_buf();
    mp4.set_extension("mp4");

    if ffmpeg::available() {
        match ffmpeg::remux_mp4(raw, &mp4) {
            Ok(()) => {
                let _ = std::fs::remove_file(raw);
                return Ok(mp4);
            }
            Err(e) => {
                // Fall through to system tools / keep .mov.
                eprintln!("mira: ffmpeg mp4 failed ({e}); trying system fallback");
            }
        }
    }

    remux_mp4_system(raw, &mp4)
}

fn convert_gif(raw: &Path) -> Result<PathBuf, RecordError> {
    let mut gif = raw.to_path_buf();
    gif.set_extension("gif");
    ffmpeg::to_gif(raw, &gif).map_err(|e| RecordError::Message(e.to_string()))?;
    let _ = std::fs::remove_file(raw);
    Ok(gif)
}

/// macOS `avconvert` when present; otherwise keep the raw container.
fn remux_mp4_system(raw: &Path, mp4: &Path) -> Result<PathBuf, RecordError> {
    #[cfg(target_os = "macos")]
    {
        if which("avconvert") {
            let attempts = [
                vec![
                    "-s".into(),
                    raw.to_string_lossy().into_owned(),
                    "-o".into(),
                    mp4.to_string_lossy().into_owned(),
                    "--preset".into(),
                    "PresetHighestQuality".into(),
                ],
                vec![
                    "--source".into(),
                    raw.to_string_lossy().into_owned(),
                    "--output".into(),
                    mp4.to_string_lossy().into_owned(),
                    "--preset".into(),
                    "PresetHighestQuality".into(),
                ],
            ];
            for args in attempts {
                let status = Command::new("avconvert").args(&args).status()?;
                if status.success() && mp4.exists() {
                    let _ = std::fs::remove_file(raw);
                    return Ok(mp4.to_path_buf());
                }
            }
        }
        Ok(raw.to_path_buf())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = mp4;
        Ok(raw.to_path_buf())
    }
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn platform_start(options: &RecordOptions, path: &Path) -> Result<Child, RecordError> {
    // OS-native writer (ScreenCapture stack behind Apple's CLI). Next step in
    // TECH_PLAN is in-process ScreenCaptureKit + VideoToolbox.
    let mut cmd = Command::new("screencapture");
    cmd.arg("-v").arg("-x");
    if options.region {
        cmd.arg("-i");
    }
    if options.show_cursor {
        cmd.arg("-k");
    }
    cmd.arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(cmd.spawn()?)
}

#[cfg(not(target_os = "macos"))]
fn platform_start(options: &RecordOptions, path: &Path) -> Result<Child, RecordError> {
    let _ = (options, path);
    Err(RecordError::Message(
        "native recording backends for Windows/Linux are planned in docs/TECH_PLAN.md".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_mp4() {
        assert_eq!(RecordOptions::default().format, RecordFormat::Mp4);
    }

    #[test]
    fn gif_start_requires_ffmpeg_when_missing() {
        if ffmpeg::available() {
            // Host already has a side-car / PATH binary; gate is a no-op.
            return;
        }
        let err = RecordSession::start(RecordOptions {
            format: RecordFormat::Gif,
            ..Default::default()
        })
        .err()
        .unwrap();
        assert!(err.to_string().contains("gif needs ffmpeg"));
    }
}
