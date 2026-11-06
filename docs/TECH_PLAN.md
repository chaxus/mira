# Mira — technical plan

Living decisions for capture / annotate / record / OCR. Product checklist stays in [`SPEC.md`](SPEC.md).

## Principles

1. **Capture like Lark / WeChat** — region UI, permissions, and frame grab use **OS APIs** (ScreenCaptureKit / Graphics Capture / PipeWire). Do not pretend a CLI ffmpeg alone is the product.
2. **Encode may use a trimmed, bundled ffmpeg** — chosen deliberately (option **B**, 2026-11-06) to unify mp4 / gif / mux across macOS · Windows · Linux without maintaining three encoder stacks first.
3. **Desktop-first on NSH** (`chaxus/nsh`) — one app model, platform backends behind thin modules.
4. **Prefer small over full** — never ship a “kitchen-sink” static ffmpeg; only the codecs Mira needs.

---

## Screen recording (decision)

### What commercial apps do

Feishu / WeChat typically use OS capture + system hardware encode (VideoToolbox / Media Foundation). They usually **do not** ship a full ffmpeg. That remains the long-term quality bar for *capture*.

Mira additionally accepts a **minimal bundled ffmpeg** for *encode / remux / GIF*, because:

- Cross-platform file output converges faster.
- Full static builds are large (~40–120MB+ per OS); a **custom-trimmed** build can target roughly **~10–25MB per platform** if limited to H.264 + AAC + mp4/mov + gif.
- Capture UX still stays native; ffmpeg is a side-car, not the screen grabber.

### Mira policy

| Layer | Choice | Notes |
|---|---|---|
| Capture | **Native OS APIs** | Permissions, region, cursor, hide-self |
| Encode / mux / GIF | **Trimmed bundled ffmpeg** (option B) | Per-platform binary under `vendor/ffmpeg/<triple>/` or download-on-build |
| Fallback encode | System tools when present | e.g. macOS VideoToolbox / `avconvert`; PATH `ffmpeg` |
| macOS capture | **ScreenCaptureKit** `SCRecordingOutput` (cidre) for fullscreen; `screencapture -i` for interactive region | |
| Windows / Linux capture | Graphics Capture / PipeWire (planned) | |
| Default container | **mp4** (H.264) via trimmed ffmpeg | `.mov` acceptable interim on macOS |
| GIF | **In scope** once bundled ffmpeg ships | Was deferred when ffmpeg was banned |

### Bundled ffmpeg constraints

| Rule | Detail |
|---|---|
| Size budget | Aim **≤ ~25MB uncompressed per target triple**; reject full GPL “everything” builds (~150MB+) |
| Codecs | Enable only what we use: **libx264 or videotoolbox/nvenc/mf** + **aac** + **mp4/mov** + **gif**; strip unused demuxers/filters |
| License | Prefer **LGPL** build if we only use LGPL pieces; if libx264 (GPL) is enabled, document GPL implications for distribution |
| Layout | `vendor/ffmpeg/<os>-<arch>/ffmpeg` (+ optional `ffprobe`); never require users to `brew install ffmpeg` |
| Resolution order | 1) bundled path 2) `MIRA_FFMPEG` env 3) system `PATH` |

### Explicit non-goals

- Shipping BtbN/gyan **full** static builds as the default release artifact.
- Using ffmpeg as the **only** screen-capture backend (no region UI / OS permissions story).

### Migration steps

1. ~~Drop ffmpeg from the *required* finalize path~~ (done) — system `.mov` / `avconvert` still work without a bundle.
2. ~~**Add trimmed ffmpeg vendor pipeline**~~ (done) — `mira::ffmpeg` resolves `MIRA_FFMPEG` → `vendor/ffmpeg/<triple>` → `PATH`; `scripts/fetch-ffmpeg.sh` + README for trimmed builds; `RecordSession` finalize uses ffmpeg for mp4/gif with system fallbacks.
3. ~~**ScreenCaptureKit (macOS) in-process capture**~~ (done) — `cidre` + `SCRecordingOutput` (H.264) for fullscreen; CLI fallback on failure / interactive region.
4. Windows / Linux native capture backends; keep the same ffmpeg side-car for mux/GIF.

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
- Long-term capture: ScreenCaptureKit / Graphics Capture still preferred over shelling out.
- Annotate: bitmap strokes in-process; interactive preview is P1.1.
- Pin: macOS Quick Look is a stand-in; true always-on-top `NSPanel` is a later polish item.

---

## Packaging implications

| Dependency | In release? |
|---|---|
| NSH / Mira binary | Yes |
| System Screen Recording permission | Yes (user grant) |
| **Trimmed ffmpeg** (per platform) | **Yes (option B)** — size-budgeted, documented |
| Full/untrimmed ffmpeg | **No** |
| tesseract | **No** (optional) |
| ONNX Runtime + PP-OCR weights (if OCR ships embedded) | Optional product decision (~tens of MB if yes; document clearly) |

Rough release add-on if three desktop triples each carry ~15–25MB ffmpeg: **~45–75MB** total across all platform artifacts (not all in one universal binary).

---

## Changelog of decisions

| Date | Decision |
|---|---|
| 2026-11-06 | Record path: native capture first; initially “do not bundle ffmpeg”; GIF deferred. |
| 2026-11-06 | OCR roadmap: RapidOCR / PP-OCRv6 small as preferred local model; Vision / tesseract as fallbacks. |
| 2026-11-06 | **Option B:** allow a **trimmed, bundled ffmpeg** for encode/mux/GIF under a per-platform size budget; capture remains native. GIF back in scope once the bundle exists. |
| 2026-11-06 | Landed `mira::ffmpeg` resolver + `RecordSession` mp4/gif finalize (ffmpeg → avconvert → keep `.mov`). |
| 2026-11-06 | macOS record default: in-process ScreenCaptureKit via `cidre` (`SCRecordingOutput`); region stays on `screencapture -i`. |
