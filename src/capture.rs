//! Screen capture modes (Lark-parity targets).

/// How Mira grabs pixels before annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureMode {
    /// Drag a rectangle (Lark default).
    #[default]
    Region,
    /// Entire display.
    Fullscreen,
    /// Long / scrolling capture.
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

/// Placeholder for the native capture pipeline (ScreenCaptureKit / DXGI / PipeWire).
pub trait Capturer {
    fn capture(&self, mode: CaptureMode) -> Result<CaptureFrame, CaptureError>;
}

/// Raw frame handed to the annotate stage.
#[derive(Debug, Clone)]
pub struct CaptureFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Capture failures.
#[derive(Debug, Clone)]
pub enum CaptureError {
    Unsupported,
    PermissionDenied,
    Cancelled,
    Message(String),
}
