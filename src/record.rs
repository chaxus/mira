//! Screen recording (Lark-parity region / fullscreen).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use thiserror::Error;

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
    #[default]
    Mp4,
    Gif,
}

impl RecordFormat {
    pub fn as_str(self) -> &'static str {
        match self {
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
    /// Path where `screencapture` writes (`.mov` on macOS).
    raw_path: PathBuf,
    options: RecordOptions,
}

impl RecordSession {
    /// Start a screen recording.
    pub fn start(options: RecordOptions) -> Result<Self, RecordError> {
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
        RecordFormat::Mp4 => {
            let mut mp4 = raw.to_path_buf();
            mp4.set_extension("mp4");
            if which("ffmpeg") {
                let status = Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-i",
                        &raw.to_string_lossy(),
                        "-c",
                        "copy",
                        &mp4.to_string_lossy(),
                    ])
                    .status()?;
                if status.success() && mp4.exists() {
                    let _ = std::fs::remove_file(raw);
                    return Ok(mp4);
                }
            }
            Ok(raw.to_path_buf())
        }
        RecordFormat::Gif => {
            if !which("ffmpeg") {
                return Err(RecordError::Message(
                    "gif export needs ffmpeg on PATH".into(),
                ));
            }
            let mut gif = raw.to_path_buf();
            gif.set_extension("gif");
            let status = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i",
                    &raw.to_string_lossy(),
                    "-vf",
                    "fps=10,scale=640:-1:flags=lanczos",
                    &gif.to_string_lossy(),
                ])
                .status()?;
            if status.success() && gif.exists() {
                let _ = std::fs::remove_file(raw);
                Ok(gif)
            } else {
                Err(RecordError::Message("ffmpeg gif conversion failed".into()))
            }
        }
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
        "screen recording is implemented for macOS in this release; Windows/Linux follow".into(),
    ))
}
