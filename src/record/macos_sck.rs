//! In-process ScreenCaptureKit recording via `SCRecordingOutput` (macOS 15+).
//!
//! Fullscreen captures the primary display. `region` presents the system
//! content-sharing picker (window / display). Falls back to CLI in `super`
//! when the picker is cancelled or unavailable.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cidre::{arc, cf, define_obj_type, ns, objc, sc};

use super::{RecordError, RecordFormat, RecordOptions};

define_obj_type!(
    MiraRecDelegate + sc::RecordingOutputDelegateImpl,
    (),
    MIRA_REC_DELEGATE
);

impl sc::RecordingOutputDelegate for MiraRecDelegate {}

#[objc::add_methods]
impl sc::RecordingOutputDelegateImpl for MiraRecDelegate {}

struct PickerInner {
    tx: Option<mpsc::Sender<Result<arc::R<sc::ContentFilter>, RecordError>>>,
}

define_obj_type!(
    MiraPickerObserver + sc::ContentSharingPickerObserverImpl,
    PickerInner,
    MIRA_PICKER_OBSERVER
);

impl sc::ContentSharingPickerObserver for MiraPickerObserver {}

#[objc::add_methods]
impl sc::ContentSharingPickerObserverImpl for MiraPickerObserver {
    extern "C" fn impl_picker_did_cancel_for_stream(
        &mut self,
        _cmd: Option<&objc::Sel>,
        _picker: &mut sc::ContentSharingPicker,
        _stream: Option<&sc::Stream>,
    ) {
        if let Some(tx) = self.inner_mut().tx.take() {
            let _ = tx.send(Err(RecordError::Message(
                "content sharing picker cancelled".into(),
            )));
        }
    }

    extern "C" fn impl_picker_did_update_with_filter_for_stream(
        &mut self,
        _cmd: Option<&objc::Sel>,
        _picker: &mut sc::ContentSharingPicker,
        filter: &sc::ContentFilter,
        _stream: Option<&sc::Stream>,
    ) {
        if let Some(tx) = self.inner_mut().tx.take() {
            let _ = tx.send(Ok(filter.retained()));
        }
    }

    extern "C" fn impl_picker_start_did_fail_with_err(
        &mut self,
        _cmd: Option<&objc::Sel>,
        err: &ns::Error,
    ) {
        if let Some(tx) = self.inner_mut().tx.take() {
            let _ = tx.send(Err(RecordError::Message(format!(
                "content sharing picker failed: {err}"
            ))));
        }
    }
}

pub struct SckSession {
    stream: arc::R<sc::Stream>,
    recording: arc::R<sc::RecordingOutput>,
    /// Must outlive `recording` (SCRecordingOutput retains the delegate weakly).
    _delegate: arc::R<MiraRecDelegate>,
    pub path: PathBuf,
}

impl SckSession {
    pub fn start(options: &RecordOptions, path: &Path) -> Result<Self, RecordError> {
        let filter = if options.region {
            pick_content_filter()?
        } else {
            primary_display_filter()?
        };
        Self::start_with_filter(filter, options, path)
    }

    fn start_with_filter(
        filter: arc::R<sc::ContentFilter>,
        options: &RecordOptions,
        path: &Path,
    ) -> Result<Self, RecordError> {
        let rect = filter.content_rect();
        let scale = filter.point_pixel_scale().max(1.0) as f64;
        let width = ((rect.size.width * scale) as usize).max(2);
        let height = ((rect.size.height * scale) as usize).max(2);

        let mut cfg = sc::StreamCfg::new();
        cfg.set_width(width);
        cfg.set_height(height);
        cfg.set_shows_cursor(options.show_cursor);
        // Crop to the selected content bounds when the filter spans a display.
        cfg.set_src_rect(rect);

        let url = ns::Url::with_fs_path_str(&path.to_string_lossy(), false);
        let mut rec_cfg = sc::RecordingOutputCfg::new();
        rec_cfg.set_output_url(&url);

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

fn primary_display_filter() -> Result<arc::R<sc::ContentFilter>, RecordError> {
    let content = shareable_content()?;
    let displays = content.displays();
    let display = displays.get(0).map_err(|_| {
        RecordError::Message("no displays available for ScreenCaptureKit".into())
    })?;
    let exclude = self_windows(&content);
    Ok(sc::ContentFilter::with_display_excluding_windows(
        &display, &exclude,
    ))
}

/// Drop Mira's own windows from the capture so the panel is not recorded.
fn self_windows(content: &sc::ShareableContent) -> arc::R<ns::Array<sc::Window>> {
    let self_pid = std::process::id() as i32;
    let all = content.windows();
    let mut ours: Vec<arc::R<sc::Window>> = Vec::new();
    for i in 0..all.len() {
        let Ok(win) = all.get(i) else {
            continue;
        };
        if win
            .owning_app()
            .is_some_and(|app| app.process_id() == self_pid)
        {
            ours.push(win.retained());
        }
    }
    ns::Array::from_slice_retained(&ours)
}

fn pick_content_filter() -> Result<arc::R<sc::ContentFilter>, RecordError> {
    let (tx, rx) = mpsc::channel();
    let observer = MiraPickerObserver::with(PickerInner { tx: Some(tx) });

    let mut cfg = sc::ContentSharingPickerCfg::new();
    cfg.set_allowed_picker_modes(
        sc::ContentSharingPickerMode::SINGLE_WINDOW
            | sc::ContentSharingPickerMode::MULTIPLE_WINDOWS
            | sc::ContentSharingPickerMode::SINGLE_DISPLAY
            | sc::ContentSharingPickerMode::SINGLE_APP,
    );
    cfg.set_allows_changing_selected_content(false);

    let mut picker = sc::ContentSharingPicker::shared();
    picker.set_default_cfg(&cfg);
    picker.add_observer(observer.as_ref());
    picker.set_active(true);
    picker.present();

    let deadline = Instant::now() + Duration::from_secs(120);
    let result = loop {
        if let Ok(msg) = rx.try_recv() {
            break msg;
        }
        if Instant::now() >= deadline {
            break Err(RecordError::Message(
                "timed out waiting for content sharing picker".into(),
            ));
        }
        // Keep AppKit / picker UI responsive while we block for a selection.
        let _ = cf::RunLoop::run_in_mode(cf::RunLoopMode::default(), 0.05, false);
    };

    picker.set_active(false);
    picker.remove_observer(observer.as_ref());
    result
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
