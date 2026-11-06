//! In-process ScreenCaptureKit recording via `SCRecordingOutput` (macOS 15+).
//!
//! Uses `cidre` ObjC bindings (VideoToolbox-backed file writer). Interactive
//! region selection still uses the `screencapture` CLI fallback in `super`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use cidre::{arc, define_obj_type, ns, objc, sc};

use super::{RecordError, RecordFormat, RecordOptions};

define_obj_type!(
    MiraRecDelegate + sc::RecordingOutputDelegateImpl,
    (),
    MIRA_REC_DELEGATE
);

impl sc::RecordingOutputDelegate for MiraRecDelegate {}

#[objc::add_methods]
impl sc::RecordingOutputDelegateImpl for MiraRecDelegate {}

pub struct SckSession {
    stream: arc::R<sc::Stream>,
    recording: arc::R<sc::RecordingOutput>,
    /// Must outlive `recording` (SCRecordingOutput retains the delegate weakly).
    _delegate: arc::R<MiraRecDelegate>,
    pub path: PathBuf,
}

impl SckSession {
    pub fn start(options: &RecordOptions, path: &Path) -> Result<Self, RecordError> {
        let content = shareable_content()?;
        let displays = content.displays();
        let display = displays.get(0).map_err(|_| {
            RecordError::Message("no displays available for ScreenCaptureKit".into())
        })?;

        let windows = ns::Array::new();
        let filter = sc::ContentFilter::with_display_excluding_windows(&display, &windows);

        let mut cfg = sc::StreamCfg::new();
        // Retina: request pixel dimensions (points × 2 is a common default).
        let width = (display.width() as usize).saturating_mul(2).max(2);
        let height = (display.height() as usize).saturating_mul(2).max(2);
        cfg.set_width(width);
        cfg.set_height(height);
        cfg.set_shows_cursor(options.show_cursor);

        let url = ns::Url::with_fs_path_str(&path.to_string_lossy(), false);
        let mut rec_cfg = sc::RecordingOutputCfg::new();
        rec_cfg.set_output_url(&url);
        // Default codec is H.264 / mp4-friendly; container follows path extension.

        let delegate = MiraRecDelegate::new();
        let recording = sc::RecordingOutput::with_cfg(&rec_cfg, delegate.as_ref());

        let mut stream = sc::Stream::new(&filter, &cfg);
        stream
            .add_recording_output(&recording)
            .map_err(|e| RecordError::Message(format!("add recording output: {e}")))?;

        wait_err("startCapture", |tx| {
            stream.start_with_ch(move |err| {
                let _ = tx.send(err.map(|e| e.to_string()));
            });
        })?;

        Ok(Self {
            stream,
            recording,
            _delegate: delegate,
            path: path.to_path_buf(),
        })
    }

    pub fn stop(mut self) -> Result<PathBuf, RecordError> {
        self.stream
            .remove_recording_output(&self.recording)
            .map_err(|e| RecordError::Message(format!("remove recording output: {e}")))?;

        // Best-effort stop if remove did not already halt capture.
        let _ = wait_err("stopCapture", |tx| {
            self.stream.stop_with_ch(move |err| {
                let _ = tx.send(err.map(|e| e.to_string()));
            });
        });

        std::thread::sleep(Duration::from_millis(250));

        if !self.path.exists() {
            return Err(RecordError::Message(
                "ScreenCaptureKit finished but no file was written — check Screen Recording permission"
                    .into(),
            ));
        }
        Ok(self.path)
    }
}

/// Preferred raw container path for an SCK session (before GIF post-process).
pub fn raw_path_for(format: RecordFormat) -> PathBuf {
    let mut path = super::media_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = match format {
        RecordFormat::Mov => "mov",
        RecordFormat::Mp4 | RecordFormat::Gif => "mp4",
    };
    path.push(format!("Mira-{stamp}.{ext}"));
    path
}

fn shareable_content() -> Result<arc::R<sc::ShareableContent>, RecordError> {
    let (tx, rx) = mpsc::channel();
    sc::ShareableContent::current_with_ch(move |content, error| {
        let result = match (content, error) {
            (Some(c), _) => Ok(c.retained()),
            (_, Some(e)) => Err(RecordError::Message(format!(
                "ScreenCaptureKit content failed ({e}) — grant Screen Recording in System Settings"
            ))),
            (None, None) => Err(RecordError::Message(
                "ScreenCaptureKit returned no content".into(),
            )),
        };
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_secs(15))
        .map_err(|_| RecordError::Message("timed out waiting for shareable content".into()))?
}

fn wait_err(
    label: &str,
    start: impl FnOnce(mpsc::Sender<Option<String>>),
) -> Result<(), RecordError> {
    let (tx, rx) = mpsc::channel();
    start(tx);
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(None) => Ok(()),
        Ok(Some(msg)) => Err(RecordError::Message(format!("{label}: {msg}"))),
        Err(_) => Err(RecordError::Message(format!(
            "timed out waiting for {label}"
        ))),
    }
}
