# Mira

**Screenshot and screen recording** for macOS, Windows, and Linux — capability target aligned with Feishu / Lark’s desktop capture toolkit. Built on **[Native Shell](https://github.com/chaxus/nsh)** (`chaxus/nsh`).

> Latin *mīrā*: look / wonder. Capture what’s on screen; keep the annotation loop close to Lark.

## Status

**P3 (in progress):** packaging script + macOS keep-alive-on-close (Dock reopen); capture/record/OCR/pin already on three OSes.

| Phase | Deliverable |
|---|---|
| P0 | Shell UI, domain types, Apache-2.0 public repo |
| P1 | Hotkeys, region/fullscreen (macOS), core annotate, clipboard / file |
| P1.1 | Windows / Linux capture, hide Mira while capturing, drag-to-draw preview |
| P2 | Scroll stitch, pin, OCR, watermark, region record, remappable hotkeys |
| P3 | Packaging (`scripts/package.sh`), macOS close→Dock keep-alive; menubar tray later |

Spec: [`docs/SPEC.md`](docs/SPEC.md) · Tech plan: [`docs/TECH_PLAN.md`](docs/TECH_PLAN.md)

## Hotkeys

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+A` (default) | Region capture → clipboard |
| `Ctrl+Shift+S` (default) | Fullscreen capture → clipboard |

Remap in-app (Hotkeys section). macOS needs **Screen Recording** permission (System Settings → Privacy & Security).

## Requirements

- macOS 14+ / Windows 10 1809+ / Linux (GTK 4.14+)
- [nsh](https://nsh.dev/docs/installation) 0.4+ — prefer [`chaxus/nsh`](https://github.com/chaxus/nsh)
- Rust **nightly** (see `rust-toolchain.toml`)
- Optional OCR: `./scripts/fetch-ocr-models.sh` (RapidOCR) or `tesseract`; macOS Vision is a fallback
- Optional: `grim`+`slurp` / `gnome-screenshot` (Linux region)
- Capture / record: macOS **ScreenCaptureKit** (screenshot + record), Windows snip / **Graphics Capture**, Linux tools / **wf-recorder** / **x11grab** — see [`docs/TECH_PLAN.md`](docs/TECH_PLAN.md)

```sh
curl -fsSL https://nsh.dev/install.sh | sh
```

## Run

```sh
export PATH="$HOME/.nsh/bin:$PATH"
nsh run --target macos
# or:
cargo run
```

## Test

```sh
cargo test --lib
```

## Package

```sh
./scripts/package.sh
```

Produces host artifacts under `dist/` via `nsh package` (`.app` / `.tar.gz` / `.zip` depending on OS). Falls back to `cargo build --release` if `nsh` is missing.

On macOS, closing the panel keeps Mira alive (reopen from the Dock); global hotkeys continue to work.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
