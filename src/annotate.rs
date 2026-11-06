//! Annotation tools aligned with Lark's screenshot toolbar.

/// Active tool while editing a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotateTool {
    Rect,
    Ellipse,
    #[default]
    Arrow,
    Pen,
    Text,
    /// Callout / pin label.
    Tag,
    /// Brush or box mosaic.
    Mosaic,
    Watermark,
}

impl AnnotateTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Arrow => "arrow",
            Self::Pen => "pen",
            Self::Text => "text",
            Self::Tag => "tag",
            Self::Mosaic => "mosaic",
            Self::Watermark => "watermark",
        }
    }
}
