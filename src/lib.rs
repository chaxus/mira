//! Mira — cross-platform screenshot and screen recording on Native Shell.

mod annotate;
mod capture;
mod chrome;
mod export;
mod hotkey;
mod preview;
mod record;
mod scroll;

use annotate::{AnnotateTool, Document, Stroke};
use capture::CaptureMode;
use export::ExportTarget;
use nsh_base::theme;
use nsh_core::prelude::*;
use parking_lot::Mutex;
use record::RecordOptions;
use scroll::ScrollSession;
use std::sync::Arc;

/// Shared editing document for the panel (UI thread + capture actions).
type DocCell = Arc<Mutex<Option<Document>>>;
type DragCell = Arc<Mutex<Option<(i32, i32)>>>;
type PenCell = Arc<Mutex<Vec<(i32, i32)>>>;
type ScrollCell = Arc<Mutex<Option<ScrollSession>>>;

const PREVIEW_WIDTH: f32 = 380.0;

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
    let preview_uri = signal(String::new());
    let preview_rev = signal(0u64);
    let drag: DragCell = Arc::new(Mutex::new(None));
    let pen: PenCell = Arc::new(Mutex::new(Vec::new()));
    let scroll: ScrollCell = Arc::new(Mutex::new(None));

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
                .child(action_row_capture(
                    &mode,
                    &status,
                    &meta,
                    doc.clone(),
                    preview_uri.clone(),
                    preview_rev.clone(),
                    scroll.clone(),
                ))
                .child(action_row_scroll(
                    &status,
                    &meta,
                    doc.clone(),
                    preview_uri.clone(),
                    preview_rev.clone(),
                    scroll,
                ))
                .child(section_label("Annotate"))
                .child(annotate_row(&tool))
                .child(preview_canvas(
                    &tool,
                    &status,
                    &meta,
                    doc.clone(),
                    preview_uri.clone(),
                    preview_rev.clone(),
                    drag,
                    pen,
                ))
                .child(action_row_annotate(
                    &tool,
                    &status,
                    &meta,
                    doc.clone(),
                    preview_uri.clone(),
                    preview_rev.clone(),
                ))
                .child(section_label("Export"))
                .child(action_row_export(&status, &meta, doc.clone()))
                .child(section_label("Record"))
                .child(record_blurb())
                .child(status_line(&status))
                .child(meta_line(&meta)),
        )
}

fn sync_preview(
    doc: &DocCell,
    preview_uri: &Signal<String>,
    preview_rev: &Signal<u64>,
    meta: &Signal<String>,
) {
    let guard = doc.lock();
    let Some(document) = guard.as_ref() else {
        preview_uri.set(String::new());
        return;
    };
    let rev = preview_rev.get() + 1;
    match preview::refresh(document, rev) {
        Ok(uri) => {
            preview_rev.set(rev);
            preview_uri.set(uri);
            meta.set(format!(
                "{}×{} · {} strokes · drag on preview to draw",
                document.width(),
                document.height(),
                document.strokes.len()
            ));
        }
        Err(e) => meta.set(format!("preview failed: {e}")),
    }
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
        .child(mode_chip("Scroll", CaptureMode::Scroll, &mode))
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
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
    scroll: ScrollCell,
) -> impl IntoElement {
    let mode = mode.clone();
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(
            press_chip("Capture now", move || {
                let hk = hotkey::last_status();
                if !hk.is_empty() {
                    status.set(hk);
                }
                if mode.get() == CaptureMode::Scroll {
                    match ScrollSession::begin() {
                        Ok(session) => {
                            let n = session.steps();
                            *scroll.lock() = Some(session);
                            status.set(format!(
                                "scroll step {n} — scroll the target, then Add page / Finish"
                            ));
                        }
                        Err(e) => status.set(format!("scroll begin failed: {e}")),
                    }
                    return;
                }
                match capture::capture(mode.get()) {
                    Ok(img) => {
                        *doc.lock() = Some(Document::new(img));
                        status.set("captured — drag on preview to annotate".into());
                        sync_preview(&doc, &preview_uri, &preview_rev, &meta);
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

fn action_row_scroll(
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
    scroll: ScrollCell,
) -> impl IntoElement {
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(press_chip("Add page", {
            let status = status.clone();
            let scroll = scroll.clone();
            move || {
                let mut guard = scroll.lock();
                let Some(session) = guard.as_mut() else {
                    status.set("start scroll with Capture now (Scroll mode)".into());
                    return;
                };
                match session.add_step() {
                    Ok(()) => status.set(format!(
                        "scroll step {} — scroll again or Finish",
                        session.steps()
                    )),
                    Err(e) => status.set(format!("add page failed: {e}")),
                }
            }
        }))
        .child(press_chip("Finish scroll", {
            let status = status.clone();
            let meta = meta.clone();
            move || {
                let session = scroll.lock().take();
                let Some(session) = session else {
                    status.set("no scroll session".into());
                    return;
                };
                match session.finish() {
                    Ok(img) => {
                        *doc.lock() = Some(Document::new(img));
                        status.set("scroll capture stitched".into());
                        sync_preview(&doc, &preview_uri, &preview_rev, &meta);
                    }
                    Err(e) => status.set(format!("stitch failed: {e}")),
                }
            }
        }))
}

fn preview_canvas(
    tool: &Signal<AnnotateTool>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
    drag: DragCell,
    pen: PenCell,
) -> impl IntoElement {
    let tool = tool.clone();
    let status = status.clone();
    let meta = meta.clone();
    let drag_down = drag.clone();
    let pen_down = pen.clone();
    let doc_down = doc.clone();
    let tool_down = tool.clone();

    view()
        .w(px(PREVIEW_WIDTH))
        .min_h(px(120.0))
        .rounded_md()
        .bg(move || theme().muted)
        .child(
            text(move || {
                if preview_uri.get().is_empty() {
                    "Capture to show preview — then drag to draw".into()
                } else {
                    String::new()
                }
            })
            .text_size(px(12.0))
            .text_color(move || theme().muted_foreground),
        )
        .child(
            image(move || preview_uri.get())
                .w(px(PREVIEW_WIDTH))
                .object_fit(ResizeMode::Contain)
                .on_press_down(move |e: &PressEvent| {
                    let guard = doc_down.lock();
                    let Some(document) = guard.as_ref() else {
                        return;
                    };
                    let (x, y) = preview::map_point(
                        e.position.x.0,
                        e.position.y.0,
                        PREVIEW_WIDTH,
                        document.width(),
                        document.height(),
                    );
                    *drag_down.lock() = Some((x, y));
                    if tool_down.get() == AnnotateTool::Pen {
                        *pen_down.lock() = vec![(x, y)];
                    }
                })
                .on_mouse_move({
                    let tool = tool.clone();
                    let pen = pen.clone();
                    let doc = doc.clone();
                    move |e: &MouseEvent| {
                        if tool.get() != AnnotateTool::Pen || e.pressed_button.is_none() {
                            return;
                        }
                        let guard = doc.lock();
                        let Some(document) = guard.as_ref() else {
                            return;
                        };
                        let (x, y) = preview::map_point(
                            e.position.x.0,
                            e.position.y.0,
                            PREVIEW_WIDTH,
                            document.width(),
                            document.height(),
                        );
                        pen.lock().push((x, y));
                    }
                })
                .on_press_up({
                    let tool = tool.clone();
                    let status = status.clone();
                    let meta = meta.clone();
                    let doc = doc.clone();
                    let preview_uri = preview_uri.clone();
                    let preview_rev = preview_rev.clone();
                    let drag = drag.clone();
                    let pen = pen.clone();
                    move |e: &PressEvent| {
                        let origin = drag.lock().take();
                        let Some((x0, y0)) = origin else {
                            return;
                        };
                        let mut guard = doc.lock();
                        let Some(document) = guard.as_mut() else {
                            return;
                        };
                        let (x1, y1) = preview::map_point(
                            e.position.x.0,
                            e.position.y.0,
                            PREVIEW_WIDTH,
                            document.width(),
                            document.height(),
                        );
                        let color = [255, 59, 48, 255];
                        let thickness = 3;
                        let stroke = match tool.get() {
                            AnnotateTool::Rect => Stroke::Rect {
                                x0,
                                y0,
                                x1,
                                y1,
                                color,
                                thickness,
                            },
                            AnnotateTool::Ellipse => Stroke::Ellipse {
                                x0,
                                y0,
                                x1,
                                y1,
                                color,
                                thickness,
                            },
                            AnnotateTool::Arrow => Stroke::Arrow {
                                x0,
                                y0,
                                x1,
                                y1,
                                color,
                                thickness,
                            },
                            AnnotateTool::Mosaic => Stroke::Mosaic {
                                x0,
                                y0,
                                x1,
                                y1,
                                block: 12,
                            },
                            AnnotateTool::Pen => {
                                let mut points = std::mem::take(&mut *pen.lock());
                                if points.len() < 2 {
                                    points.push((x1, y1));
                                }
                                Stroke::Pen {
                                    points,
                                    color,
                                    thickness,
                                }
                            }
                            AnnotateTool::Text | AnnotateTool::Tag | AnnotateTool::Watermark => {
                                status.set(format!("{} lands in P2", tool.get().as_str()));
                                return;
                            }
                        };
                        document.push(stroke);
                        status.set(format!("drew {}", tool.get().as_str()));
                        drop(guard);
                        sync_preview(&doc, &preview_uri, &preview_rev, &meta);
                    }
                }),
        )
}

fn action_row_annotate(
    tool: &Signal<AnnotateTool>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
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
            let preview_uri = preview_uri.clone();
            let preview_rev = preview_rev.clone();
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
                status.set(format!("applied {}", tool.get().as_str()));
                drop(guard);
                sync_preview(&doc, &preview_uri, &preview_rev, &meta);
            }
        }))
        .child(press_chip("Undo", {
            let status = status.clone();
            let meta = meta.clone();
            let preview_uri = preview_uri.clone();
            let preview_rev = preview_rev.clone();
            move || {
                let mut guard = doc.lock();
                let Some(document) = guard.as_mut() else {
                    status.set("nothing to undo".into());
                    return;
                };
                if document.undo() {
                    status.set("undone".into());
                    drop(guard);
                    sync_preview(&doc, &preview_uri, &preview_rev, &meta);
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
