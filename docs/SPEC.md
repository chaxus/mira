# Mira — product spec

Cross-platform screenshot and screen recording. Capability target: **Feishu / Lark desktop** screenshot + record toolbar. UI and packaging: **[chaxus/nsh](https://github.com/chaxus/nsh)** (Native Shell).

## Goals

- Desktop-first (macOS · Windows · Linux); mobile / web later via NSH.
- Match Lark’s capture → annotate → export / record loop, without embedding Feishu.
- Apache-2.0, public, no telemetry by default.

## Feature parity (Lark)

### Capture

| Feature | Status |
|---|---|
| Region select | **P1** macOS `screencapture -i`; **P1.1** Windows snipping / Linux grim·GNOME·KDE |
| Fullscreen | **P1** macOS ScreenCaptureKit (`SCScreenshotManager`, display under cursor, CLI fallback); **P1.1** Windows / Linux `xcap` |
| Include mouse cursor | **P2** (macOS SCK / `screencapture -C`; toggle in Capture row) |
| Scroll / long capture | **P2** (multi-step fullscreen stitch) |
| Hide Mira window while capturing | **P1.1** (macOS System Events + SCK exclude-self; Win EnumWindows; Linux xdotool) |
| Custom global hotkey | **P2** (panel editor + `~/…/mira/hotkeys.conf`; defaults Ctrl+Shift+A/S) |

### Annotate

| Feature | Status |
|---|---|
| Rect / ellipse / arrow / pen | **P1** (bitmap strokes + sample apply) |
| Mosaic (box) | **P1** |
| Undo | **P1** |
| Text / tag | **P2** (label field + click-to-place; tag is a filled pill) |
| Watermark | **P2** |
| Interactive drag-to-draw on preview | **P1.1** |

### Export

| Feature | Status |
|---|---|
| Copy to clipboard | **P1** |
| Save to file | **P1** (Pictures/Mira-*.png; reveal in Finder/Explorer) |
| Pin to screen | **P2** (macOS floating window; other OS open PNG) |
| Extract text (OCR) | **P2** RapidOCR/PP-OCRv6 ONNX (optional) → Vision → tesseract |

### Record

| Feature | Status |
|---|---|
| Region record | **P2** macOS SCK picker (CLI fallback) / Windows picker / Linux `slurp` |
| Show cursor | **P2** (`-k` clicks) |
| Format | **P2** **mp4 only** (H.264); remux via ffmpeg / `avconvert` — no `.mov` delivery |
| GIF | Supported when ffmpeg is resolved (vendor / `MIRA_FFMPEG` / PATH) |
| Preview · download · clipboard | **P2** stop copies file (macOS) or path text; “Copy last path” button |

## Hotkeys

| Shortcut | Action |
|---|---|
| Ctrl+Shift+A (default) | Region capture → clipboard |
| Ctrl+Shift+S (default) | Fullscreen capture → clipboard |

Remap in the Hotkeys section (`Ctrl+Shift+A` style). Persisted under macOS `~/Library/Application Support/mira/hotkeys.conf`, elsewhere `~/.config/mira/hotkeys.conf`.

## Architecture

```
hotkey → capture → annotate → export
                 ↘ record → export
```

- `capture` — macOS ScreenCaptureKit fullscreen + `screencapture -i` region; Windows snip/`xcap`; Linux tools/`xcap`.
- `annotate` — vector-ish strokes flattened onto RGBA.
- `record` — macOS SCK / Windows WGC / Linux wf-recorder·x11grab; encode/mux via system tools and/or **trimmed bundled ffmpeg** (see [`TECH_PLAN.md`](TECH_PLAN.md) option B).
- `export` — clipboard (`arboard`), filesystem PNG.
- `hotkey` — `global-hotkey` background listener.
- Shell UI — NSH Rust `App`.

## Non-goals (v0)

- Feishu IM integration or bot upload.
- Cloud sync of captures.
- Mobile system-gesture replacement.
