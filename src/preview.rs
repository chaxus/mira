//! On-disk preview PNG for the annotate canvas.

use std::path::{Path, PathBuf};

use crate::annotate::Document;
use crate::export::{self, ExportError};

/// Directory + file used for the live preview image.
pub fn preview_path(rev: u64) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("mira-preview-{}-{rev}.png", std::process::id()));
    path
}

/// Render `doc` to a unique preview file and return a `file://` URI.
pub fn refresh(doc: &Document, rev: u64) -> Result<String, ExportError> {
    let path = preview_path(rev);
    let img = doc.render();
    export::save_png(&img, &path)?;
    Ok(file_uri(&path))
}

fn file_uri(path: &Path) -> String {
    let path = path.display().to_string();
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// Map a point inside the preview view to image pixel coordinates.
pub fn map_point(
    view_x: f32,
    view_y: f32,
    view_w: f32,
    img_w: u32,
    img_h: u32,
) -> (i32, i32) {
    if view_w <= 0.0 || img_w == 0 || img_h == 0 {
        return (0, 0);
    }
    let scale = view_w / img_w as f32;
    let x = (view_x / scale).round() as i32;
    let y = (view_y / scale).round() as i32;
    (
        x.clamp(0, img_w.saturating_sub(1) as i32),
        y.clamp(0, img_h.saturating_sub(1) as i32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_point_scales() {
        let (x, y) = map_point(190.0, 95.0, 380.0, 760, 380);
        assert_eq!((x, y), (380, 190));
    }
}
