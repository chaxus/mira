//! Mira — cross-platform screenshot and screen recording on Native Shell.

mod annotate;
mod capture;
mod chrome;
mod export;
mod ffmpeg;
mod hotkey;
mod ocr;
mod pin;
mod preview;
mod record;
mod scroll;
mod tray;

use annotate::{AnnotateTool, Document, Stroke};
use capture::CaptureMode;
use export::ExportTarget;
use nsh_base::{input, theme};
use nsh_core::prelude::*;
use parking_lot::Mutex;
use record::{RecordFormat, RecordOptions, RecordSession};
use scroll::ScrollSession;
use std::sync::Arc;

/// Shared editing document for the panel (UI thread + capture actions).
type DocCell = Arc<Mutex<Option<Document>>>;
type DragCell = Arc<Mutex<Option<(i32, i32)>>>;
type PenCell = Arc<Mutex<Vec<(i32, i32)>>>;
type ScrollCell = Arc<Mutex<Option<ScrollSession>>>;
type RecordCell = Arc<Mutex<Option<RecordSession>>>;

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
    // Must run on the UI/main thread (macOS NSStatusItem requirement).
    tray::ensure_started();

    let mode = signal(CaptureMode::Region);
    let show_cursor = signal(capture::options().show_cursor);
    let hk = hotkey::current_bindings();
    let region_hk = signal(hk.region.clone());
    let full_hk = signal(hk.fullscreen.clone());
    let tool = signal(AnnotateTool::Arrow);
    let label = signal(String::from("Note"));
    let status = signal(format!(
        "Ready · {} region · {} screen",
        hk.region, hk.fullscreen
    ));
    let doc: DocCell = Arc::new(Mutex::new(None));
    let meta = signal(String::from("No capture yet"));
    let preview_uri = signal(String::new());
    let preview_rev = signal(0u64);
    let drag: DragCell = Arc::new(Mutex::new(None));
    let pen: PenCell = Arc::new(Mutex::new(Vec::new()));
    let scroll: ScrollCell = Arc::new(Mutex::new(None));
    let recorder: RecordCell = Arc::new(Mutex::new(None));

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
                .child(capture_row(&mode, &show_cursor))
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
                .child(section_label("Hotkeys"))
                .child(hotkey_row(&region_hk, &full_hk, &status))
                .child(section_label("Annotate"))
                .child(annotate_row(&tool))
                .child(label_row(&label))
                .child(preview_canvas(
                    &tool,
                    &label,
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
                    &label,
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
                .child(action_row_record(&status, &meta, recorder))
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

fn capture_row(mode: &Signal<CaptureMode>, show_cursor: &Signal<bool>) -> impl IntoElement {
    let mode = mode.clone();
    let show_cursor = show_cursor.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(mode_chip("Region", CaptureMode::Region, &mode))
        .child(mode_chip("Fullscreen", CaptureMode::Fullscreen, &mode))
        .child(mode_chip("Scroll", CaptureMode::Scroll, &mode))
        .child({
            let show_cursor = show_cursor.clone();
            let selected = move || show_cursor.get();
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
                .on_press(move || {
                    let next = !show_cursor.get();
                    show_cursor.set(next);
                    capture::set_show_cursor(next);
                })
                .child(text(move || {
                    if show_cursor.get() {
                        "Cursor on"
                    } else {
                        "Cursor off"
                    }
                }).text_size(px(13.0)))
        })
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
        .child(tool_chip("Text", AnnotateTool::Text, &tool))
        .child(tool_chip("Tag", AnnotateTool::Tag, &tool))
        .child(tool_chip("Mosaic", AnnotateTool::Mosaic, &tool))
        .child(tool_chip("Watermark", AnnotateTool::Watermark, &tool))
}

fn label_row(label: &Signal<String>) -> impl IntoElement {
    let label = label.clone();
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text("Label")
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            input()
                .flex_1()
                .value(move || label.get())
                .placeholder("Text / tag content")
                .on_change(move |value: &str| label.set(value.to_string())),
        )
}

fn hotkey_row(
    region_hk: &Signal<String>,
    full_hk: &Signal<String>,
    status: &Signal<String>,
) -> impl IntoElement {
    let region_hk = region_hk.clone();
    let full_hk = full_hk.clone();
    let status = status.clone();
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text("Region")
                        .text_sm()
                        .w(px(72.0))
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    input()
                        .flex_1()
                        .value(move || region_hk.get())
                        .placeholder("Ctrl+Shift+A")
                        .on_change(move |value: &str| region_hk.set(value.to_string())),
                ),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text("Screen")
                        .text_sm()
                        .w(px(72.0))
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    input()
                        .flex_1()
                        .value(move || full_hk.get())
                        .placeholder("Ctrl+Shift+S")
                        .on_change(move |value: &str| full_hk.set(value.to_string())),
                ),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(press_chip("Apply hotkeys", {
                    let region_hk = region_hk.clone();
                    let full_hk = full_hk.clone();
                    let status = status.clone();
                    move || {
                        let bindings = hotkey::HotkeyBindings {
                            region: region_hk.get(),
                            fullscreen: full_hk.get(),
                        };
                        match hotkey::apply_bindings(bindings) {
                            Ok(()) => status.set(hotkey::last_status()),
                            Err(e) => status.set(format!("hotkey apply failed: {e}")),
                        }
                    }
                }))
                .child(press_chip("Reset defaults", {
                    let region_hk = region_hk.clone();
                    let full_hk = full_hk.clone();
                    let status = status.clone();
                    move || {
                        let defaults = hotkey::HotkeyBindings::default();
                        region_hk.set(defaults.region.clone());
                        full_hk.set(defaults.fullscreen.clone());
                        match hotkey::apply_bindings(defaults) {
                            Ok(()) => status.set(hotkey::last_status()),
                            Err(e) => status.set(format!("hotkey reset failed: {e}")),
                        }
                    }
                })),
        )
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
    label: &Signal<String>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
    drag: DragCell,
    pen: PenCell,
) -> impl IntoElement {
    let tool = tool.clone();
    let label = label.clone();
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
                    let label = label.clone();
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
                            AnnotateTool::Watermark => Stroke::Watermark {
                                text: "Mira".into(),
                                color: [0, 0, 0, 70],
                                size: 28.0,
                            },
                            AnnotateTool::Text => Stroke::Text {
                                x: x0.min(x1),
                                y: y0.min(y1),
                                text: label.get(),
                                color,
                                size: 28.0,
                            },
                            AnnotateTool::Tag => Stroke::Tag {
                                x: x0.min(x1),
                                y: y0.min(y1),
                                text: label.get(),
                                color,
                                size: 22.0,
                            },
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
    label: &Signal<String>,
    status: &Signal<String>,
    meta: &Signal<String>,
    doc: DocCell,
    preview_uri: Signal<String>,
    preview_rev: Signal<u64>,
) -> impl IntoElement {
    let tool = tool.clone();
    let label = label.clone();
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(press_chip("Apply sample stroke", {
            let tool = tool.clone();
            let label = label.clone();
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
                    AnnotateTool::Watermark => Stroke::Watermark {
                        text: "Mira".into(),
                        color: [0, 0, 0, 70],
                        size: 28.0,
                    },
                    AnnotateTool::Text => Stroke::Text {
                        x: w / 5,
                        y: h / 5,
                        text: label.get(),
                        color,
                        size: 28.0,
                    },
                    AnnotateTool::Tag => Stroke::Tag {
                        x: w / 5,
                        y: h / 5,
                        text: label.get(),
                        color,
                        size: 22.0,
                    },
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
                        let revealed = export::reveal(&path).is_ok();
                        status.set(if revealed {
                            format!(
                                "{} · revealed in file manager",
                                ExportTarget::File.as_str()
                            )
                        } else {
                            format!("{} · {}", ExportTarget::File.as_str(), path.display())
                        });
                    }
                    Err(e) => status.set(format!("save failed: {e}")),
                }
            }
        }))
        .child(press_chip("Pin", {
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
                match pin::pin_to_screen(&img) {
                    Ok(path) => {
                        meta.set(path.display().to_string());
                        status.set(format!("{} — floating preview", ExportTarget::PinToScreen.as_str()));
                    }
                    Err(e) => status.set(format!("pin failed: {e}")),
                }
            }
        }))
        .child(press_chip("OCR", {
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
                match ocr::extract_text(&img) {
                    Ok(text) => {
                        let preview: String = text.chars().take(160).collect();
                        meta.set(preview);
                        let engine = if ocr::rapidocr_available() {
                            "RapidOCR"
                        } else {
                            "Vision/tesseract"
                        };
                        if let Err(e) = arboard::Clipboard::new()
                            .and_then(|mut c| c.set_text(text.clone()))
                        {
                            status.set(format!(
                                "{} via {engine} (clipboard copy failed: {e})",
                                ExportTarget::ExtractText.as_str()
                            ));
                        } else {
                            status.set(format!(
                                "{} via {engine} — copied {} chars",
                                ExportTarget::ExtractText.as_str(),
                                text.len()
                            ));
                        }
                    }
                    Err(e) => status.set(format!("ocr failed: {e}")),
                }
            }
        }))
}

fn record_blurb() -> impl IntoElement {
    let opts = RecordOptions::default();
    let ff = if crate::ffmpeg::available() {
        "ffmpeg ready"
    } else {
        "ffmpeg missing (PATH / vendor / MIRA_FFMPEG)"
    };
    let backend = if cfg!(target_os = "macos") {
        "SCK / screencapture"
    } else if cfg!(target_os = "windows") {
        "Graphics Capture"
    } else if cfg!(target_os = "linux") {
        "wf-recorder / x11grab"
    } else {
        "unsupported"
    };
    text(format!(
        "{backend} · cursor {} · {} · {ff}",
        if opts.show_cursor { "on" } else { "off" },
        opts.format.as_str()
    ))
    .text_size(px(13.0))
    .text_color(move || theme().muted_foreground)
}

fn action_row_record(
    status: &Signal<String>,
    meta: &Signal<String>,
    recorder: RecordCell,
) -> impl IntoElement {
    let status = status.clone();
    let meta = meta.clone();
    view()
        .flex_row()
        .gap_2()
        .child(press_chip("Start record", {
            let status = status.clone();
            let meta = meta.clone();
            let recorder = recorder.clone();
            move || {
                if recorder.lock().is_some() {
                    status.set("already recording".into());
                    return;
                }
                match RecordSession::start(RecordOptions::default()) {
                    Ok(session) => {
                        let backend = session.backend_label();
                        meta.set(session.raw_path().display().to_string());
                        *recorder.lock() = Some(session);
                        status.set(format!("recording mp4 via {backend} — press Stop"));
                    }
                    Err(e) => status.set(format!("record failed: {e}")),
                }
            }
        }))
        .child(press_chip("Start GIF", {
            let status = status.clone();
            let meta = meta.clone();
            let recorder = recorder.clone();
            move || {
                if recorder.lock().is_some() {
                    status.set("already recording".into());
                    return;
                }
                match RecordSession::start(RecordOptions {
                    format: RecordFormat::Gif,
                    ..Default::default()
                }) {
                    Ok(session) => {
                        meta.set(session.raw_path().display().to_string());
                        *recorder.lock() = Some(session);
                        status.set("recording gif — stop when done".into());
                    }
                    Err(e) => status.set(format!("record failed: {e}")),
                }
            }
        }))
        .child(press_chip("Start region", {
            let status = status.clone();
            let meta = meta.clone();
            let recorder = recorder.clone();
            move || {
                if recorder.lock().is_some() {
                    status.set("already recording".into());
                    return;
                }
                match RecordSession::start(RecordOptions {
                    region: true,
                    ..Default::default()
                }) {
                    Ok(session) => {
                        meta.set(session.raw_path().display().to_string());
                        *recorder.lock() = Some(session);
                        status.set("region record — pick window/display, then Stop".into());
                    }
                    Err(e) => status.set(format!("record failed: {e}")),
                }
            }
        }))
        .child(press_chip("Stop record", {
            let status = status.clone();
            let meta = meta.clone();
            move || {
                let session = recorder.lock().take();
                let Some(session) = session else {
                    status.set("not recording".into());
                    return;
                };
                match session.stop() {
                    Ok(path) => {
                        meta.set(path.display().to_string());
                        match export::copy_recording(&path) {
                            Ok("file") => {
                                status.set("recording saved · file on clipboard".into())
                            }
                            Ok(_) => {
                                status.set("recording saved · path on clipboard".into())
                            }
                            Err(e) => status.set(format!(
                                "recording saved · clipboard failed: {e}"
                            )),
                        }
                    }
                    Err(e) => status.set(format!("stop failed: {e}")),
                }
            }
        }))
        .child(press_chip("Copy last path", {
            let status = status.clone();
            let meta = meta.clone();
            move || {
                let path = meta.get();
                if path.is_empty() || path == "No capture yet" {
                    status.set("no recording path yet".into());
                    return;
                }
                let p = std::path::PathBuf::from(&path);
                match export::copy_recording(&p) {
                    Ok(kind) => status.set(format!("copied recording {kind}")),
                    Err(e) => {
                        if let Err(e2) = export::copy_text(&path) {
                            status.set(format!("clipboard failed: {e} / {e2}"));
                        } else {
                            status.set("copied path text".into());
                        }
                    }
                }
            }
        }))
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
