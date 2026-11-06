#!/usr/bin/env bash
# Place a local ffmpeg under vendor/ffmpeg/<triple>/ for Mira (option B).
#
# Default: copy from PATH for development only (may exceed the ~25MB budget).
# Release builds should use a custom-trimmed binary — see vendor/ffmpeg/README.md.
#
# Usage:
#   ./scripts/fetch-ffmpeg.sh              # copy $(command -v ffmpeg)
#   ./scripts/fetch-ffmpeg.sh /path/to/ffmpeg
#   MIRA_FFMPEG_SRC=/path/to/ffmpeg ./scripts/fetch-ffmpeg.sh

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

uname_s="$(uname -s)"
uname_m="$(uname -m)"
case "${uname_s}-${uname_m}" in
  Darwin-arm64) triple="macos-aarch64" ;;
  Darwin-x86_64) triple="macos-x86_64" ;;
  Linux-x86_64|Linux-amd64) triple="linux-x86_64" ;;
  Linux-aarch64|Linux-arm64) triple="linux-aarch64" ;;
  MINGW*|MSYS*|CYGWIN*)
    case "$uname_m" in
      aarch64|arm64) triple="windows-aarch64" ;;
      *) triple="windows-x86_64" ;;
    esac
    ;;
  *)
    echo "unsupported host: ${uname_s}-${uname_m}" >&2
    exit 1
    ;;
esac

dest_dir="vendor/ffmpeg/${triple}"
mkdir -p "$dest_dir"
bin_name="ffmpeg"
case "$triple" in
  windows-*) bin_name="ffmpeg.exe" ;;
esac
dest="${dest_dir}/${bin_name}"

src="${1:-${MIRA_FFMPEG_SRC:-}}"
if [[ -z "$src" ]]; then
  src="$(command -v ffmpeg || true)"
fi
if [[ -z "$src" || ! -f "$src" ]]; then
  echo "ffmpeg source not found. Pass a path or install ffmpeg on PATH." >&2
  exit 1
fi

cp "$src" "$dest"
chmod +x "$dest" 2>/dev/null || true

size_mb="$(du -m "$dest" | awk '{print $1}')"
echo "installed: $dest (${size_mb} MB)"
if [[ "$size_mb" -gt 25 ]]; then
  echo "warning: exceeds ~25MB trimmed budget — OK for local PATH copy; rebuild trimmed for release." >&2
fi
echo "resolve check: MIRA_FFMPEG=$dest cargo test -q --lib ffmpeg::"
