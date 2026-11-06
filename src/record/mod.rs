//! Screen recording — ScreenCaptureKit in-process on macOS, encode/mux helpers.
//!
//! See `docs/TECH_PLAN.md` (option B). macOS prefers `SCRecordingOutput`
//! (VideoToolbox). Interactive region still uses `screencapture -v -i`.
//! Finalize for GIF / cross-tool mp4 uses trimmed/PATH ffmpeg when needed.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use thiserror::Error;

use crate::ffmpeg;

#[cfg(target_os = "macos")]
mod macos_sck;

/// Container for a recording session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOptions {
    pub show_cursor: bool,
    pub format: RecordFormat,
    /// When true, use interactive region select (`screencapture -i` on macOS).
    /// Full-display ScreenCaptureKit is used when false.
    pub region: bool,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            show_cursor: true,
            format: RecordFormat::Mp4,
            // Prefer in-process SCK full display; region still available via CLI.
            region: false,
        }
    }
}

/// Output format for recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordFormat {
    /// QuickTime / MOV container.
    Mov,
    /// Prefer direct SCK mp4 or ffmpeg remux; fall back to `avconvert` / raw.
    #[default]
    Mp4,
    /// Post-process capture to GIF via ffmpeg (requires side-car or PATH).
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

enum Backend {
    #[cfg(target_os = "macos")]
    Sck(macos_sck::SckSession),
    Cli {
        child: Child,
        raw_path: PathBuf,
    },
}

/// Active recording process / stream.
pub struct RecordSession {
    backend: Backend,
    options: RecordOptions,
    raw_path: PathBuf,
}

impl RecordSession {
    /// Start a screen recording via the platform native path.
    ///
    /// Capture uses ScreenCaptureKit when possible; `format` is applied in [`stop`]
    /// (GIF always post-processes).
    pub fn start(options: RecordOptions) -> Result<Self, RecordError> {
        if options.format == RecordFormat::Gif && !ffmpeg::available() {
            return Err(RecordError::Message(
                "gif needs ffmpeg — run scripts/fetch-ffmpeg.sh or set MIRA_FFMPEG / PATH".into(),
            ));
        }
        let (backend, raw_path) = platform_start(&options)?;
        Ok(Self {
            backend,
            options,
            raw_path,
        })
    }

    pub fn raw_path(&self) -> &Path {
        &self.raw_path
    }

    /// Whether this session uses in-process ScreenCaptureKit.
    #[cfg(target_os = "macos")]
    pub fn is_sck(&self) -> bool {
        matches!(self.backend, Backend::Sck(_))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn is_sck(&self) -> bool {
        false
    }

    /// Stop recording and return the final media path.
    pub fn stop(self) -> Result<PathBuf, RecordError> {
        let Self {
            backend,
            options,
            raw_path: _,
        } = self;

        let raw = match backend {
            #[cfg(target_os = "macos")]
            Backend::Sck(session) => session.stop()?,
            Backend::Cli {
                mut child,
                raw_path,
            } => {
                #[cfg(unix)]
                {
                    let _ = Command::new("kill")
                        .args(["-INT", &child.id().to_string()])
                        .status();
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
                let _ = child.kill();
                let status = child.wait()?;
                if !raw_path.exists() {
                    if status.success() {
                        return Err(RecordError::Message(
                            "recording ended but no file was written".into(),
                        ));
                    }
                    return Err(RecordError::Message(
                        "recording cancelled or permission denied".into(),
                    ));
                }
                raw_path
            }
        };

        finalize(&raw, options.format)
    }
}

pub(super) fn media_dir() -> PathBuf {
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

fn cli_raw_path() -> PathBuf {
    let mut path = media_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    path.push(format!("Mira-{stamp}.mov"));
    path
}

fn finalize(raw: &Path, format: RecordFormat) -> Result<PathBuf, RecordError> {
    match format {
        RecordFormat::Mov => Ok(raw.to_path_buf()),
        RecordFormat::Mp4 => {
            if raw.extension().and_then(|e| e.to_str()) == Some("mp4") {
                return Ok(raw.to_path_buf());
            }
            remux_mp4(raw)
        }
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

fn platform_start(options: &RecordOptions) -> Result<(Backend, PathBuf), RecordError> {
    #[cfg(target_os = "macos")]
    {
        // Interactive region still needs the system UI; use CLI for that path.
        if !options.region {
            let path = macos_sck::raw_path_for(options.format);
            match macos_sck::SckSession::start(options, &path) {
                Ok(session) => {
                    return Ok((Backend::Sck(session), path));
                }
                Err(e) => {
                    eprintln!("mira: ScreenCaptureKit unavailable ({e}); falling back to screencapture");
                }
            }
        }
        let path = cli_raw_path();
        let child = cli_start(options, &path)?;
        return Ok((
            Backend::Cli {
                child,
                raw_path: path.clone(),
            },
            path,
        ));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(RecordError::Message(
            "native recording backends for Windows/Linux are planned in docs/TECH_PLAN.md".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn cli_start(options: &RecordOptions, path: &Path) -> Result<Child, RecordError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_mp4_fullscreen_sck() {
        let opts = RecordOptions::default();
        assert_eq!(opts.format, RecordFormat::Mp4);
        assert!(!opts.region);
    }

    #[test]
    fn gif_start_requires_ffmpeg_when_missing() {
        if ffmpeg::available() {
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
