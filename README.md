# Mira

**Screenshot and screen recording** for macOS, Windows, and Linux — capability target aligned with Feishu / Lark’s desktop capture toolkit. Built on **[Native Shell](https://github.com/chaxus/nsh)** (`chaxus/nsh`).

> Latin *mīrā*: look / wonder. Capture what’s on screen; keep the annotation loop close to Lark.

## Status

**P1 (current):** macOS region / fullscreen capture, core annotate strokes, clipboard + PNG save, global hotkeys.

| Phase | Deliverable |
|---|---|
| P0 | Shell UI, domain types, Apache-2.0 public repo |
| P1 | Hotkeys, region/fullscreen (macOS), rect/ellipse/arrow/pen/mosaic, clipboard / file |
| P1.1 | Windows / Linux capture, hide-self, interactive draw on preview |
| P2 | Scroll capture, pin, OCR, watermark, region record |
| P3 | Tray / polish, packaging, optional mobile via NSH |

Spec: [`docs/SPEC.md`](docs/SPEC.md)

## Hotkeys

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+A` | Region capture → clipboard |
| `Ctrl+Shift+S` | Fullscreen capture → clipboard |

macOS needs **Screen Recording** permission for `screencapture` (System Settings → Privacy & Security).

## Requirements

- macOS 14+ / Windows 10 1809+ / Linux (GTK 4.14+) — **capture works on macOS in P1**
- [nsh](https://nsh.dev/docs/installation) 0.4+ — prefer building against [`chaxus/nsh`](https://github.com/chaxus/nsh)
- Rust **nightly** (see `rust-toolchain.toml`)

```sh
curl -fsSL https://nsh.dev/install.sh | sh
```

To compile against the fork instead of crates.io, uncomment the `[patch.crates-io]` block in `Cargo.toml`.

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
