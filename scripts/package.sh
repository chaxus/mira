#!/usr/bin/env bash
# Build a release Mira binary / NSH package for the current host.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="${HOME}/.nsh/bin:${PATH}"

case "$(uname -s)" in
  Darwin) TARGET=macos ;;
  Linux) TARGET=linux ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT) TARGET=windows ;;
  *)
    echo "unsupported host: $(uname -s)" >&2
    exit 1
    ;;
esac

echo "==> cargo test --lib"
cargo test --lib

mkdir -p dist

if command -v nsh >/dev/null 2>&1; then
  echo "==> nsh build --release"
  nsh build --release -o .nsh/out

  echo "==> nsh package --target ${TARGET}"
  nsh package --target "${TARGET}" \
    --name Mira \
    --id com.chaxus.mira \
    --app-version 0.1.0 \
    -o dist
else
  echo "nsh not on PATH; falling back to cargo build --release"
  cargo build --release
  BIN="target/release/mira"
  if [[ -f "$BIN" ]]; then
    cp "$BIN" "dist/mira"
  elif [[ -f "${BIN}.exe" ]]; then
    cp "${BIN}.exe" "dist/mira.exe"
  fi
fi

echo "Done. Artifacts in dist/ (and .nsh/out/ when using nsh)."
