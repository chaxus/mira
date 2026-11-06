//! Fullscreen screenshot via ScreenCaptureKit (`SCScreenshotManager`).
//!
//! Interactive region still uses `screencapture -i` (system drag UX). This
//! path avoids shelling out for fullscreen and matches the record stack.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use cidre::{arc, blocks, cf, cg, ns, sc};

use super::CaptureError;

/// Capture the display under the mouse cursor (falls back to the first display).
pub fn capture_fullscreen_png(path: &Path, show_cursor: bool) -> Result<(), CaptureError> {
    let content = shareable_content()?;
    let display = display_under_cursor(&content)?;

    let exclude = self_windows(&content);
    let filter = sc::ContentFilter::with_display_excluding_windows(&display, &exclude);

    let rect = filter.content_rect();
    let scale = f64::from(filter.point_pixel_scale()).max(1.0);
    let width = ((rect.size.width * scale) as usize).max(2);
    let height = ((rect.size.height * scale) as usize).max(2);

    let mut cfg = sc::StreamCfg::new();
    cfg.set_width(width);
    cfg.set_height(height);
    cfg.set_shows_cursor(show_cursor);
    cfg.set_src_rect(rect);

    let image = capture_cg_image(&filter, &cfg)?;
    write_png(&image, path)
}

fn display_under_cursor(
    content: &sc::ShareableContent,
) -> Result<arc::R<sc::Display>, CaptureError> {
    let loc = ns::Event::mouse_location();
    let displays = content.displays();
    let mut first = None;
    for i in 0..displays.len() {
        let Ok(display) = displays.get(i) else {
            continue;
        };
        if first.is_none() {
            first = Some(display.retained());
        }
        if display.frame().contains_point(&loc) {
            return Ok(display.retained());
        }
    }
    first.ok_or_else(|| CaptureError::Message("no displays available".into()))
}

/// Windows owned by this process — excluded so Mira chrome stays out of the shot
/// without relying solely on the System Events hide dance.
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

fn shareable_content() -> Result<arc::R<sc::ShareableContent>, CaptureError> {
    let (tx, rx) = mpsc::channel();
    sc::ShareableContent::current_with_ch(move |content, error| {
        let result = match (content, error) {
            (Some(c), _) => Ok(c.retained()),
            (_, Some(e)) => Err(CaptureError::Message(format!(
                "ScreenCaptureKit content failed ({e}) — grant Screen Recording in System Settings"
            ))),
            (None, None) => Err(CaptureError::Message(
                "ScreenCaptureKit returned no content".into(),
            )),
        };
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_secs(15))
        .map_err(|_| CaptureError::Message("timed out waiting for shareable content".into()))?
}

fn capture_cg_image(
    filter: &sc::ContentFilter,
    cfg: &sc::StreamCfg,
) -> Result<arc::R<cg::Image>, CaptureError> {
    let (tx, rx) = mpsc::channel();
    let mut block = blocks::ResultCh::<cg::Image>::new2(move |image, error| {
        let result = match (image, error) {
            (Some(img), _) => Ok(img.retained()),
            (_, Some(e)) => Err(CaptureError::Message(format!("screenshot: {e}"))),
            (None, None) => Err(CaptureError::Message("screenshot returned nothing".into())),
        };
        let _ = tx.send(result);
    });
    sc::ScreenshotManager::capture_image_ch(filter, cfg, Some(&mut block));
    // Keep `block` alive until the completion fires (API may copy it; hold for safety).
    let result = rx
        .recv_timeout(Duration::from_secs(30))
        .map_err(|_| CaptureError::Message("timed out waiting for screenshot".into()))?;
    drop(block);
    result
}

fn write_png(image: &cg::Image, path: &Path) -> Result<(), CaptureError> {
    let url = cf::Url::with_path(path, false)
        .ok_or_else(|| CaptureError::Message("invalid capture path".into()))?;
    let ty = cf::String::from_str("public.png");
    let mut dst = cg::ImageDst::with_url(&url, &ty, 1)
        .ok_or_else(|| CaptureError::Message("failed to create PNG destination".into()))?;
    dst.add_image(image, None);
    if !dst.finalize() {
        return Err(CaptureError::Message("failed to finalize PNG".into()));
    }
    if !path.exists() {
        return Err(CaptureError::Message("PNG was not written".into()));
    }
    Ok(())
}
