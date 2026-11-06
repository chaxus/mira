//! Screen recording options (Lark-parity) — wired in P2.

/// Container for a recording session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOptions {
    pub show_cursor: bool,
    pub format: RecordFormat,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            show_cursor: true,
            format: RecordFormat::Mp4,
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
