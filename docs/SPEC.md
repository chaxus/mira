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
| Region select | Planned (P1) |
| Fullscreen | Planned (P1) |
| Scroll / long capture | Planned (P2) |
| Hide Mira window while capturing | Planned (P1) |
| Custom global hotkey | Planned (P1) |

### Annotate

| Feature | Status |
|---|---|
| Rect / ellipse / arrow / pen | Planned (P1) |
| Text | Planned (P1) |
| Tag / callout | Planned (P2) |
| Mosaic (brush + box) | Planned (P1) |
| Watermark | Planned (P2) |
| Undo | Planned (P1) |

### Export

| Feature | Status |
|---|---|
| Copy to clipboard | Planned (P1) |
| Save to file | Planned (P1) |
| Pin to screen | Planned (P2) |
| Extract text (OCR) | Planned (P2) |

### Record

| Feature | Status |
|---|---|
| Region record | Planned (P2) |
| Show cursor | Planned (P2) |
| Format (mp4 / gif) | Planned (P2) |
| Preview · download · clipboard | Planned (P2) |

## Architecture

```
hotkey → capture → annotate → export
                 ↘ record → export
```

- `capture` — OS pixel pipelines (ScreenCaptureKit / DXGI Desktop Duplication / PipeWire).
- `annotate` — vector overlay on a bitmap frame.
- `record` — timed frame encoder.
- `export` — clipboard, filesystem, pin window, OCR.
- Shell UI — NSH Rust components (`App`), tray / floating editor later.

## Non-goals (v0)

- Feishu IM integration or bot upload.
- Cloud sync of captures.
- Mobile system-gesture replacement.
