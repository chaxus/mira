# Mira — technical plan

Living decisions for capture / annotate / record / OCR. Product checklist stays in [`SPEC.md`](SPEC.md).

## Principles

1. **Ship like Lark / WeChat, not like a ffmpeg wrapper** — prefer OS capture + system encoders; never bundle a full ffmpeg into the release by default.
2. **Desktop-first on NSH** (`chaxus/nsh`) — one app model, platform backends behind thin modules.
3. **Optional tools stay optional** — `tesseract` / `ffmpeg` / `grim` may enhance a machine that already has them; they are not install requirements.

---

## Screen recording (decision)

### What commercial apps do

Feishu / WeChat / similar desktop clients typically:

| Platform | Capture | Encode |
|---|---|---|
| macOS | ScreenCaptureKit / AVFoundation | VideoToolbox (H.264 / HEVC) |
| Windows | Graphics Capture / Desktop Duplication | Media Foundation |
| Linux | PipeWire / X11 | platform encoders or small in-process libs |

They do **not** ship a full ffmpeg binary (~tens of MB) to end users. Package size stays in the app’s own code + UI assets.

### Mira policy

| Layer | Choice | Notes |
|---|---|---|
| Default record path | **Native OS APIs** | No bundled ffmpeg |
| macOS (near-term) | `screencapture -v` → `.mov` (system) | Already OS-native; stop UX still coarse |
| macOS (target) | ScreenCaptureKit + VideoToolbox | Region, cursor, clean start/stop in-process |
| Windows (target) | Windows.Graphics.Capture + Media Foundation | |
| Linux (target) | PipeWire / portal screencast | |
| Container | Default **`.mov` / system MP4** | Remux only with **system** tools (e.g. macOS `avconvert`) |
| GIF | **Deferred** | Optional later; never a release dependency |

### Explicit non-goals for recording

- Bundling ffmpeg / libav\* into Mira installers.
- Requiring users to `brew install ffmpeg` for the basic record → save loop.

### Migration steps

1. **Now** — drop ffmpeg from the default finalize path; keep `.mov` (and `avconvert` → `.mp4` when present).
2. **Next** — replace CLI `screencapture -v` with in-process ScreenCaptureKit session (start / stop / region).
3. **Then** — Windows / Linux native backends behind the same `RecordSession` API.

---

## OCR (decision)

Current path: `tesseract` if installed, else macOS Vision via a short Swift helper.

### Preferred local stack (small / CPU-friendly)

| Option | Why |
|---|---|
| **RapidOCR + PP-OCRv6 small/mobile (ONNX)** | Strong CN/EN, Apache-2.0, ONNX Runtime, no PaddlePaddle runtime required; good fit for a desktop helper |
| **PP-OCRv6 tiny** | Smaller / faster when quality tradeoff is OK |
| macOS Vision (keep as fallback) | Zero extra model download on Apple Silicon / recent macOS |
| Avoid as default | Full PaddlePaddle wheel; GPU-only VL OCR (GOT / large VLMs) for the tray-app path |

### OCR migration steps

1. Keep Vision / tesseract working.
2. Add optional RapidOCR ONNX side-car or embed PP-OCRv6 mobile weights under Apache-2.0.
3. Default order: RapidOCR (if assets present) → Vision (macOS) → tesseract → error with install hint.

---

## Capture / annotate (status pointers)

- Capture: macOS `screencapture` + Windows snip/`xcap` + Linux tools/`xcap` — see SPEC.
- Long-term capture: ScreenCaptureKit / Graphics Capture still preferred over shelling out, same as record.
- Annotate: bitmap strokes in-process; interactive preview is P1.1.
- Pin: macOS Quick Look is a stand-in; true always-on-top `NSPanel` is a later polish item.

---

## Packaging implications

| Dependency | In release? |
|---|---|
| NSH / Mira binary | Yes |
| System Screen Recording permission | Yes (user grant) |
| ffmpeg | **No** |
| tesseract | **No** (optional) |
| ONNX Runtime + PP-OCR weights (if OCR ships embedded) | Optional product decision (~tens of MB if yes; document clearly) |

---

## Changelog of decisions

| Date | Decision |
|---|---|
| 2026-11-06 | Record path must be native / system-encoder first; do not bundle ffmpeg. GIF deferred. |
| 2026-11-06 | OCR roadmap: RapidOCR / PP-OCRv6 small as preferred local model; Vision / tesseract as fallbacks. |
