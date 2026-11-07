//! Windows.Graphics.Capture + Media Foundation encode via `windows-capture`.
//!
//! Fullscreen uses the primary monitor; `region` opens the system capture picker.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::{
    AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::graphics_capture_picker::GraphicsCapturePicker;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use super::{RecordError, RecordFormat, RecordOptions};

struct SessionFlags {
    stop_flag: Arc<AtomicBool>,
    width: u32,
    height: u32,
    path: String,
}

struct Handler {
    encoder: Option<VideoEncoder>,
    stop_flag: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = SessionFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let encoder = VideoEncoder::new(
            VideoSettingsBuilder::new(ctx.flags.width, ctx.flags.height)
                .bitrate(8_000_000)
                .frame_rate(30),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &ctx.flags.path,
        )?;
        Ok(Self {
            encoder: Some(encoder),
            stop_flag: ctx.flags.stop_flag,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if let Some(encoder) = self.encoder.as_mut() {
            encoder.send_frame(frame)?;
        }
        if self.stop_flag.load(Ordering::SeqCst) {
            if let Some(encoder) = self.encoder.take() {
                encoder.finish()?;
            }
            capture_control.stop();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(encoder) = self.encoder.take() {
            let _ = encoder.finish();
        }
        Ok(())
    }
}

pub struct WgcSession {
    control: CaptureControl<Handler, Box<dyn std::error::Error + Send + Sync>>,
    stop_flag: Arc<AtomicBool>,
    pub path: PathBuf,
}

impl WgcSession {
    pub fn start(options: &RecordOptions, path: &Path) -> Result<Self, RecordError> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let path_str = path.to_string_lossy().into_owned();

        let cursor = if options.show_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        };

        let control = if options.region {
            let item = GraphicsCapturePicker::pick_item()
                .map_err(|e| RecordError::Message(format!("capture picker: {e}")))?
                .ok_or_else(|| RecordError::Message("capture cancelled".into()))?;
            let (w, h) = item
                .size()
                .map_err(|e| RecordError::Message(format!("item size: {e}")))?;
            let flags = SessionFlags {
                stop_flag: stop_flag.clone(),
                width: w.max(2) as u32,
                height: h.max(2) as u32,
                path: path_str,
            };
            let settings = Settings::new(
                item,
                cursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            );
            Handler::start_free_threaded(settings)
                .map_err(|e| RecordError::Message(format!("start capture: {e}")))?
        } else {
            let monitor = Monitor::primary()
                .map_err(|e| RecordError::Message(format!("primary monitor: {e}")))?;
            let width = monitor
                .width()
                .map_err(|e| RecordError::Message(format!("monitor width: {e}")))?
                .max(2);
            let height = monitor
                .height()
                .map_err(|e| RecordError::Message(format!("monitor height: {e}")))?
                .max(2);
            let flags = SessionFlags {
                stop_flag: stop_flag.clone(),
                width,
                height,
                path: path_str,
            };
            let settings = Settings::new(
                monitor,
                cursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            );
            Handler::start_free_threaded(settings)
                .map_err(|e| RecordError::Message(format!("start capture: {e}")))?
        };

        Ok(Self {
            control,
            stop_flag,
            path: path.to_path_buf(),
        })
    }

    pub fn stop(self) -> Result<PathBuf, RecordError> {
        self.stop_flag.store(true, Ordering::SeqCst);

        let deadline = Instant::now() + Duration::from_secs(8);
        while !self.control.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }

        if self.control.is_finished() {
            self.control
                .wait()
                .map_err(|e| RecordError::Message(format!("wait capture: {e}")))?;
        } else {
            self.control
                .stop()
                .map_err(|e| RecordError::Message(format!("stop capture: {e}")))?;
        }

        std::thread::sleep(Duration::from_millis(200));
        if !self.path.exists() {
            return Err(RecordError::Message(
                "Windows Graphics Capture finished but no file was written".into(),
            ));
        }
        Ok(self.path)
    }
}

pub fn raw_path_for(format: RecordFormat) -> PathBuf {
    let mut path = super::media_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = match format {
        RecordFormat::Mp4 | RecordFormat::Gif => "mp4",
    };
    path.push(format!("Mira-{stamp}.{ext}"));
    path
}
