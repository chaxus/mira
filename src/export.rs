//! Export / post-capture actions (clipboard, file, pin, OCR).

/// Where a finished capture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportTarget {
    #[default]
    Clipboard,
    File,
    /// Float the image above other windows (Lark "pin to screen").
    PinToScreen,
    /// Run OCR and copy / edit extracted text.
    ExtractText,
}

impl ExportTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clipboard => "clipboard",
            Self::File => "file",
            Self::PinToScreen => "pin",
            Self::ExtractText => "ocr",
        }
    }
}
