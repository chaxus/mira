//! Interactive region screenshot on macOS without `screencapture -i`.
//!
//! Freeze the display via SCK, show it in a borderless overlay, poll a mouse
//! drag, then crop the frozen bitmap.

use std::path::Path;
use std::time::{Duration, Instant};

use cidre::{arc, cf, cg, ns};
use image::RgbaImage;

use super::macos_sck;
use super::CaptureError;

/// Drag-select a rectangle on the active display and write a PNG.
pub fn capture_region_png(path: &Path, show_cursor: bool) -> Result<(), CaptureError> {
    let tmp = path.with_extension("full.png");
    macos_sck::capture_fullscreen_png(&tmp, show_cursor)?;

    let full = image::open(&tmp)
        .map_err(|e| CaptureError::Image(e.to_string()))?
        .to_rgba8();

    let display = active_display_frame()?;
    let overlay = match show_freeze_overlay(&tmp, display.frame) {
        Ok(o) => o,
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(err);
        }
    };
    let _ = std::fs::remove_file(&tmp);

    let selection = match wait_for_drag_selection(display.frame) {
        Ok(sel) => sel,
        Err(err) => {
            overlay.close();
            return Err(err);
        }
    };
    overlay.close();

    let cropped = crop_to_selection(&full, display.frame, selection)?;
    cropped
        .save(path)
        .map_err(|e| CaptureError::Image(e.to_string()))?;
    Ok(())
}

struct DisplayGeom {
    frame: cg::Rect,
}

fn active_display_frame() -> Result<DisplayGeom, CaptureError> {
    let loc = ns::Event::mouse_location();
    let screens = ns::Screen::screens();
    let mut first = None;
    for i in 0..screens.len() {
        let Ok(screen) = screens.get(i) else {
            continue;
        };
        let frame = screen.frame();
        if first.is_none() {
            first = Some(DisplayGeom { frame });
        }
        if frame.contains_point(&loc) {
            return Ok(DisplayGeom { frame });
        }
    }
    first.ok_or_else(|| CaptureError::Message("no screens available".into()))
}

struct Overlay {
    window: arc::R<ns::Window>,
}

impl Overlay {
    fn close(self) {
        self.window.close();
    }
}

fn show_freeze_overlay(image_path: &Path, frame: cg::Rect) -> Result<Overlay, CaptureError> {
    let path_str = ns::String::with_str(&image_path.to_string_lossy());
    let image = ns::Image::with_contents_of_file(&path_str)
        .ok_or_else(|| CaptureError::Message("failed to load freeze frame".into()))?;

    let style = ns::WindowStyleMask::BORDERLESS;
    let mut window = ns::Window::with_content_rect_style_mask_backing_defer(
        frame,
        style,
        ns::BackingStoreType::Buffered,
        false,
    );
    window.set_level(ns::WindowLevel::SCREEN_SAVER);
    window.set_opaque(true);
    window.set_collection_behavior(
        ns::WindowCollectionBehavior::CAN_JOIN_ALL_SPACES
            | ns::WindowCollectionBehavior::STATIONARY
            | ns::WindowCollectionBehavior::FULL_SCREEN_AUXILIARY,
    );

    let view = ns::ImageView::with_image(&image);
    window.set_content_view(Some(view.as_ref()));
    window.order_front(None);
    ns::App::shared().activate_ignoring_other_apps(true);
    Ok(Overlay { window })
}

#[derive(Clone, Copy)]
struct Selection {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn wait_for_drag_selection(frame: cg::Rect) -> Result<Selection, CaptureError> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut origin: Option<cg::Point> = None;

    while Instant::now() < deadline {
        pump();
        if ns::Event::pressed_mouse_buttons() & 1 != 0 {
            let loc = ns::Event::mouse_location();
            if frame.contains_point(&loc) {
                origin = Some(loc);
                break;
            }
        }
    }
    let start = origin.ok_or(CaptureError::Cancelled)?;

    while Instant::now() < deadline {
        pump();
        if ns::Event::pressed_mouse_buttons() & 1 == 0 {
            let end = ns::Event::mouse_location();
            let sel = Selection {
                x0: start.x,
                y0: start.y,
                x1: end.x,
                y1: end.y,
            };
            if (sel.x1 - sel.x0).abs() < 4.0 && (sel.y1 - sel.y0).abs() < 4.0 {
                return Err(CaptureError::Cancelled);
            }
            return Ok(sel);
        }
    }
    Err(CaptureError::Cancelled)
}

fn pump() {
    let _ = cf::RunLoop::run_in_mode(cf::RunLoopMode::default(), 0.016, true);
}

fn crop_to_selection(
    full: &RgbaImage,
    frame: cg::Rect,
    sel: Selection,
) -> Result<RgbaImage, CaptureError> {
    let xa = sel.x0.min(sel.x1);
    let xb = sel.x0.max(sel.x1);
    let ya = sel.y0.min(sel.y1);
    let yb = sel.y0.max(sel.y1);

    let scale_x = full.width() as f64 / frame.size.width.max(1.0);
    let scale_y = full.height() as f64 / frame.size.height.max(1.0);

    let left = ((xa - frame.origin.x) * scale_x).round().max(0.0) as u32;
    let right = ((xb - frame.origin.x) * scale_x).round() as u32;
    let top_cocoa = frame.origin.y + frame.size.height;
    let top = ((top_cocoa - yb) * scale_y).round().max(0.0) as u32;
    let bottom = ((top_cocoa - ya) * scale_y).round() as u32;

    let x = left.min(full.width().saturating_sub(1));
    let y = top.min(full.height().saturating_sub(1));
    let w = right.saturating_sub(left).max(1).min(full.width().saturating_sub(x));
    let h = bottom.saturating_sub(top).max(1).min(full.height().saturating_sub(y));

    Ok(image::imageops::crop_imm(full, x, y, w, h).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_math_covers_full_frame() {
        let img = RgbaImage::from_pixel(100, 50, image::Rgba([1, 2, 3, 255]));
        let frame = cg::Rect::new(0.0, 0.0, 100.0, 50.0);
        let sel = Selection {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 50.0,
        };
        let out = crop_to_selection(&img, frame, sel).unwrap();
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
    }
}
