//! Floating always-on-top window for pinned captures.

use std::path::Path;
use std::sync::Mutex;

use cidre::{arc, ns};

use super::PinError;

struct PinHandle {
    /// Retained so the window stays visible after this function returns.
    _window: arc::R<ns::Window>,
}

static PINS: Mutex<Vec<PinHandle>> = Mutex::new(Vec::new());

pub fn show_floating(path: &Path) -> Result<(), PinError> {
    let path_str = ns::String::with_str(&path.to_string_lossy());
    let image = ns::Image::with_contents_of_file(&path_str)
        .ok_or_else(|| PinError::Message("failed to load pin image".into()))?;

    let size = image.size();
    let max_edge = 720.0_f64;
    let scale = (max_edge / size.width)
        .min(max_edge / size.height)
        .min(1.0);
    let width = (size.width * scale).max(80.0);
    let height = (size.height * scale).max(80.0);

    let style = ns::WindowStyleMask::TITLED | ns::WindowStyleMask::CLOSABLE;
    let rect = ns::Rect::new(120.0, 120.0, width, height);
    let mut window = ns::Window::with_content_rect_style_mask_backing_defer(
        rect,
        style,
        ns::BackingStoreType::Buffered,
        false,
    );

    window.set_title(&ns::String::with_str("Mira Pin"));
    window.set_level(ns::WindowLevel::FLOATING);
    window.set_collection_behavior(
        ns::WindowCollectionBehavior::CAN_JOIN_ALL_SPACES
            | ns::WindowCollectionBehavior::STATIONARY
            | ns::WindowCollectionBehavior::FULL_SCREEN_AUXILIARY,
    );
    window.set_opaque(true);

    let view = ns::ImageView::with_image(&image);
    window.set_content_view(Some(view.as_ref()));
    window.order_front(None);

    PINS.lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(PinHandle { _window: window });
    Ok(())
}
