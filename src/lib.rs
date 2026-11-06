//! Mira — cross-platform screenshot and screen recording on Native Shell.

mod annotate;
mod capture;
mod export;
mod record;

use annotate::AnnotateTool;
use capture::CaptureMode;
use export::ExportTarget;
use nsh_base::theme;
use nsh_core::prelude::*;
use record::RecordOptions;

/// What a future TSX entry may import from `mira/native`.
#[native_module("mira/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// Compact control panel for capture / annotate / record (P0 shell).
#[component]
pub fn App() -> impl IntoElement {
    let mode = signal(CaptureMode::Region);
    let tool = signal(AnnotateTool::Arrow);
    let export = signal(ExportTarget::Clipboard);

    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_1()
                .p_5()
                .gap_4()
                .child(title())
                .child(section_label("Capture"))
                .child(capture_row(&mode))
                .child(section_label("Annotate"))
                .child(annotate_row(&tool))
                .child(section_label("Export"))
                .child(export_row(&export))
                .child(section_label("Record"))
                .child(record_blurb())
                .child(primary_hint()),
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

fn capture_row(mode: &Signal<CaptureMode>) -> impl IntoElement {
    let mode = mode.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(chip("Region", CaptureMode::Region, &mode))
        .child(chip("Fullscreen", CaptureMode::Fullscreen, &mode))
        .child(chip("Scroll", CaptureMode::Scroll, &mode))
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

fn export_row(export: &Signal<ExportTarget>) -> impl IntoElement {
    let export = export.clone();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(export_chip("Clipboard", ExportTarget::Clipboard, &export))
        .child(export_chip("File", ExportTarget::File, &export))
        .child(export_chip("Pin", ExportTarget::PinToScreen, &export))
        .child(export_chip("OCR", ExportTarget::ExtractText, &export))
}

fn record_blurb() -> impl IntoElement {
    let opts = RecordOptions::default();
    text(format!(
        "Region record · show cursor: {} · format: {} (wired in P1)",
        if opts.show_cursor { "on" } else { "off" },
        opts.format.as_str()
    ))
    .text_size(px(13.0))
    .text_color(move || theme().muted_foreground)
}

fn primary_hint() -> impl IntoElement {
    view()
        .mt_2()
        .p_3()
        .rounded_md()
        .bg(move || theme().muted)
        .child(
            text("P0 shell — hotkeys and real capture land next. Default: ⌘⇧A / Ctrl⇧A.")
                .text_size(px(12.0))
                .text_color(move || theme().muted_foreground),
        )
}

fn chip(label: &'static str, value: CaptureMode, mode: &Signal<CaptureMode>) -> impl IntoElement {
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
        .on_click(move |_| mode.set(value))
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
        .on_click(move |_| tool.set(value))
        .child(text(label).text_size(px(13.0)))
}

fn export_chip(
    label: &'static str,
    value: ExportTarget,
    export: &Signal<ExportTarget>,
) -> impl IntoElement {
    let export = export.clone();
    let selected = move || export.get() == value;
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
        .on_click(move |_| export.set(value))
        .child(text(label).text_size(px(13.0)))
}
