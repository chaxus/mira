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
| Fullscreen | **P1** macOS `screencapture`; **P1.1** Windows / Linux `xcap` |
| Scroll / long capture | **P2** (multi-step fullscreen stitch) |
| Hide Mira window while capturing | **P1.1** (macOS System Events / Win EnumWindows / Linux xdotool) |
| Custom global hotkey | Planned (P1.1); defaults ship in P1 |

### Annotate

| Feature | Status |
|---|---|
| Rect / ellipse / arrow / pen | **P1** (bitmap strokes + sample apply) |
| Mosaic (box) | **P1** |
| Undo | **P1** |
| Text / tag | Planned (later) |
| Watermark | **P2** |
| Interactive drag-to-draw on preview | **P1.1** |

### Export

| Feature | Status |
|---|---|
| Copy to clipboard | **P1** |
| Save to file | **P1** (Pictures/Mira-*.png) |
| Pin to screen | **P2** (macOS Quick Look panel; other OS open PNG) |
| Extract text (OCR) | **P2** (tesseract or macOS Vision via Swift) |

### Record

| Feature | Status |
|---|---|
| Region record | **P2** (macOS native `screencapture -v -i`; SCK planned) |
| Show cursor | **P2** (`-k` clicks) |
| Format | **P2** interim `.mov` / system tools; **target** trimmed bundled ffmpeg → mp4 (TECH_PLAN option B) |
| GIF | Planned once trimmed ffmpeg is vendored (TECH_PLAN) |
| Preview · download · clipboard | Partial (save path; clipboard later) |

## Hotkeys (P1 defaults)

| Shortcut | Action |
|---|---|
| Ctrl+Shift+A | Region capture → clipboard |
| Ctrl+Shift+S | Fullscreen capture → clipboard |

## Architecture

```
hotkey → capture → annotate → export
                 ↘ record → export
```

- `capture` — macOS `screencapture` today; Windows / Linux native paths next.
- `annotate` — vector-ish strokes flattened onto RGBA.
- `record` — native capture + encode via system tools and/or **trimmed bundled ffmpeg** (see [`TECH_PLAN.md`](TECH_PLAN.md) option B).
- `export` — clipboard (`arboard`), filesystem PNG.
- `hotkey` — `global-hotkey` background listener.
- Shell UI — NSH Rust `App`.

## Non-goals (v0)

- Feishu IM integration or bot upload.
- Cloud sync of captures.
- Mobile system-gesture replacement.
