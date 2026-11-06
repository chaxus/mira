# Mira

**Screenshot and screen recording** for macOS, Windows, and Linux — capability target aligned with Feishu / Lark’s desktop capture toolkit. Built on **[Native Shell](https://github.com/chaxus/nsh)** (`chaxus/nsh`).

> Latin *mīrā*: look / wonder. Capture what’s on screen; keep the annotation loop close to Lark.

## Status

**P2 (current):** desktop capture on three OSes, hide-self, interactive annotate, scroll stitch, pin, OCR, watermark, macOS region record.

| Phase | Deliverable |
|---|---|
| P0 | Shell UI, domain types, Apache-2.0 public repo |
| P1 | Hotkeys, region/fullscreen (macOS), core annotate, clipboard / file |
| P1.1 | Windows / Linux capture, hide Mira while capturing, drag-to-draw preview |
| P2 | Scroll stitch, pin, OCR, watermark, macOS region record |
| P3 | Tray / polish, packaging, Win/Linux record, richer text/tag tools |

Spec: [`docs/SPEC.md`](docs/SPEC.md) · Tech plan: [`docs/TECH_PLAN.md`](docs/TECH_PLAN.md)

## Hotkeys

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+A` | Region capture → clipboard |
| `Ctrl+Shift+S` | Fullscreen capture → clipboard |

macOS needs **Screen Recording** permission (System Settings → Privacy & Security).

## Requirements

- macOS 14+ / Windows 10 1809+ / Linux (GTK 4.14+)
- [nsh](https://nsh.dev/docs/installation) 0.4+ — prefer [`chaxus/nsh`](https://github.com/chaxus/nsh)
- Rust **nightly** (see `rust-toolchain.toml`)
- Optional: `tesseract` (OCR), `grim`+`slurp` / `gnome-screenshot` (Linux region)
- Recording: macOS **ScreenCaptureKit** fullscreen (region via system UI); encode/mux via trimmed ffmpeg — see [`docs/TECH_PLAN.md`](docs/TECH_PLAN.md)

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

## License

Apache License 2.0 — see [LICENSE](LICENSE).
