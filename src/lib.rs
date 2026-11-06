//! Mira — cross-platform screenshot and screen recording on Native Shell.

mod annotate;
mod capture;
mod chrome;
mod export;
mod hotkey;
mod record;

use annotate::{AnnotateTool, Document, Stroke};
use capture::CaptureMode;
use export::ExportTarget;
use nsh_base::theme;
use nsh_core::prelude::*;
use parking_lot::Mutex;
use record::RecordOptions;
use std::sync::Arc;

/// Shared editing document for the panel (UI thread + capture actions).
type DocCell = Arc<Mutex<Option<Document>>>;

/// What a future TSX entry may import from `mira/native`.
#[native_module("mira/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// Compact control panel for capture / annotate / export (P1).
#[component]
pub fn App() -> impl IntoElement {
    hotkey::ensure_started();

    let mode = signal(CaptureMode::Region);
    let tool = signal(AnnotateTool::Arrow);
    let status = signal(String::from("Ready · Ctrl+Shift+A region · Ctrl+Shift+S screen"));
    let doc: DocCell = Arc::new(Mutex::new(None));
    let meta = signal(String::from("No capture yet"));

    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_1()
                .p_5()
                .gap_3()
                .child(title())
                .child(section_label("Capture"))
                .child(capture_row(&mode))
                .child(action_row_capture(&mode, &status, &meta, doc.clone()))
                .child(section_label("Annotate"))
                .child(annotate_row(&tool))
                .child(action_row_annotate(&tool, &status, &meta, doc.clone()))
                .child(section_label("Export"))
                .child(action_row_export(&status, &meta, doc.clone()))
                .child(section_label("Record"))
                .child(record_blurb())
                .child(status_line(&status))
                .child(meta_line(&meta)),
        )
}

fn title() -> impl IntoElement {
    view()
        .flex_col()
        .gap_1()
        .child(
            text("Mira")
                .text_size(px(24.0))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            text("Screenshot & record — Lark-parity on Native Shell")
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
}

fn section_label(label: &'static str) -> impl IntoElement {
    text(label)
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(move || theme().muted_foreground)
}

fn status_line(status: &Signal<String>) -> impl IntoElement {
    let status = status.clone();
    view()
        .mt_2()
        .p_3()
        .rounded_md()
        .bg(move || theme().muted)
        .child(
            text(move || status.get())
                .text_size(px(12.0))
                .text_color(move || theme().muted_foreground),
        )
}

fn meta_line(meta: &Signal<String>) -> impl IntoElement {
    let meta = meta.clone();
    text(move || meta.get())
        .text_size(px(12.0))
        .text_color(move || theme().muted_foreground)
}

fn capture_row(mode: &Signal<CaptureMode>) -> impl IntoElement {
    let mode = mode.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(mode_chip("Region", CaptureMode::Region, &mode))
        .child(mode_chip("Fullscreen", CaptureMode::Fullscreen, &mode))
        .child(mode_chip("Scroll (P2)", CaptureMode::Scroll, &mode))
}

fn annotate_row(tool: &Signal<AnnotateTool>) -> impl IntoElement {
    let tool = tool.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(tool_chip("Rect", AnnotateTool::Rect, &tool))
        .child(tool_chip("Ellipse", AnnotateTool::Ellipse, &tool))
        .child(tool_chip("Arrow", AnnotateTool::Arrow, &tool))
        .child(tool_chip("Pen", AnnotateTool::Pen, &tool))
        .child(tool_chip("Mosaic", AnnotateTool::Mosaic, &tool))
}

fn action_row_capture(
    mode: &Signal<CaptureMode>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
) -> impl IntoElement {
    let mode = mode.clone();
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(
            press_chip("Capture now", move || {
                // Pull hotkey status if any.
                let hk = hotkey::last_status();
                if !hk.is_empty() {
                    status.set(hk);
                }
                match capture::capture(mode.get()) {
                    Ok(img) => {
                        let w = img.width();
                        let h = img.height();
                        *doc.lock() = Some(Document::new(img));
                        meta.set(format!("{w}×{h} px"));
                        status.set("captured — annotate or export".into());
                    }
                    Err(capture::CaptureError::Cancelled) => {
                        status.set("capture cancelled".into());
                    }
                    Err(e) => status.set(format!("capture failed: {e}")),
                }
            }),
        )
        .child(
            press_chip("Refresh hotkey status", {
                let status = status.clone();
                move || status.set(hotkey::last_status())
            }),
        )
}

fn action_row_annotate(
    tool: &Signal<AnnotateTool>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
) -> impl IntoElement {
    let tool = tool.clone();
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(press_chip("Apply sample stroke", {
            let tool = tool.clone();
            let status = status.clone();
            let meta = meta.clone();
            let doc = doc.clone();
            move || {
                let mut guard = doc.lock();
                let Some(document) = guard.as_mut() else {
                    status.set("capture something first".into());
                    return;
                };
                let w = document.width() as i32;
                let h = document.height() as i32;
                let color = [255, 59, 48, 255];
                let thickness = 3;
                let stroke = match tool.get() {
                    AnnotateTool::Rect => Stroke::Rect {
                        x0: w / 8,
                        y0: h / 8,
                        x1: w * 3 / 8,
                        y1: h * 3 / 8,
                        color,
                        thickness,
                    },
                    AnnotateTool::Ellipse => Stroke::Ellipse {
                        x0: w / 8,
                        y0: h / 8,
                        x1: w * 3 / 8,
                        y1: h * 3 / 8,
                        color,
                        thickness,
                    },
                    AnnotateTool::Arrow => Stroke::Arrow {
                        x0: w / 6,
                        y0: h / 2,
                        x1: w * 2 / 3,
                        y1: h / 3,
                        color,
                        thickness,
                    },
                    AnnotateTool::Pen => Stroke::Pen {
                        points: vec![
                            (w / 5, h / 2),
                            (w / 3, h / 3),
                            (w / 2, h / 2),
                            (w * 2 / 3, h * 2 / 5),
                        ],
                        color,
                        thickness,
                    },
                    AnnotateTool::Mosaic => Stroke::Mosaic {
                        x0: w / 2,
                        y0: h / 2,
                        x1: w * 3 / 4,
                        y1: h * 3 / 4,
                        block: 12,
                    },
                    AnnotateTool::Text | AnnotateTool::Tag | AnnotateTool::Watermark => {
                        status.set(format!("{} lands in P2", tool.get().as_str()));
                        return;
                    }
                };
                document.push(stroke);
                meta.set(format!(
                    "{}×{} · {} strokes",
                    document.width(),
                    document.height(),
                    document.strokes.len()
                ));
                status.set(format!("applied {}", tool.get().as_str()));
            }
        }))
        .child(press_chip("Undo", {
            let status = status.clone();
            let meta = meta.clone();
            move || {
                let mut guard = doc.lock();
                let Some(document) = guard.as_mut() else {
                    status.set("nothing to undo".into());
                    return;
                };
                if document.undo() {
                    meta.set(format!(
                        "{}×{} · {} strokes",
                        document.width(),
                        document.height(),
                        document.strokes.len()
                    ));
                    status.set("undone".into());
                } else {
                    status.set("undo stack empty".into());
                }
            }
        }))
}

fn action_row_export(
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
) -> impl IntoElement {
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(press_chip("Copy", {
            let status = status.clone();
            let doc = doc.clone();
            move || {
                let guard = doc.lock();
                let Some(document) = guard.as_ref() else {
                    status.set("capture something first".into());
                    return;
                };
                let img = document.render();
                match export::copy_to_clipboard(&img) {
                    Ok(()) => status.set(format!(
                        "copied {}×{} to clipboard",
                        img.width(),
                        img.height()
                    )),
                    Err(e) => status.set(format!("clipboard failed: {e}")),
                }
            }
        }))
        .child(press_chip("Save PNG", {
            let status = status.clone();
            let meta = meta.clone();
            let doc = doc.clone();
            move || {
                let guard = doc.lock();
                let Some(document) = guard.as_ref() else {
                    status.set("capture something first".into());
                    return;
                };
                let img = document.render();
                let path = export::default_save_path();
                match export::save_png(&img, &path) {
                    Ok(()) => {
                        meta.set(path.display().to_string());
                        status.set("saved PNG".into());
                    }
                    Err(e) => status.set(format!("save failed: {e}")),
                }
            }
        }))
        .child(press_chip("Pin (P2)", {
            let status = status.clone();
            move || status.set(format!("{} lands in P2", ExportTarget::PinToScreen.as_str()))
        }))
        .child(press_chip("OCR (P2)", {
            let status = status.clone();
            move || status.set(format!("{} lands in P2", ExportTarget::ExtractText.as_str()))
        }))
}

fn record_blurb() -> impl IntoElement {
    let opts = RecordOptions::default();
    text(format!(
        "Region record · cursor {} · {} — P2",
        if opts.show_cursor { "on" } else { "off" },
        opts.format.as_str()
    ))
    .text_size(px(13.0))
    .text_color(move || theme().muted_foreground)
}

fn mode_chip(label: &'static str, value: CaptureMode, mode: &Signal<CaptureMode>) -> impl IntoElement {
    let mode = mode.clone();
    let selected = move || mode.get() == value;
    view()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(move || {
            if selected() {
                theme().primary
            } else {
                theme().muted
            }
        })
        .text_color(move || {
            if selected() {
                theme().primary_foreground
            } else {
                theme().foreground
            }
        })
        .on_press(move || mode.set(value))
        .child(text(label).text_size(px(13.0)))
}

fn tool_chip(
    label: &'static str,
    value: AnnotateTool,
    tool: &Signal<AnnotateTool>,
) -> impl IntoElement {
    let tool = tool.clone();
    let selected = move || tool.get() == value;
    view()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(move || {
            if selected() {
                theme().primary
            } else {
                theme().muted
            }
        })
        .text_color(move || {
            if selected() {
                theme().primary_foreground
            } else {
                theme().foreground
            }
        })
        .on_press(move || tool.set(value))
        .child(text(label).text_size(px(13.0)))
}

fn press_chip(label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    view()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(move || theme().secondary)
        .text_color(move || theme().secondary_foreground)
        .on_press(move || on_press())
        .child(text(label).text_size(px(13.0)))
}
