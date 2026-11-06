#!/usr/bin/env bash
# Download RapidOCR / PP-OCRv6 ONNX assets into vendor/ocr (or $MIRA_OCR_MODELS).
#
# Usage:
#   ./scripts/fetch-ocr-models.sh           # ppocrv6-small
#   ./scripts/fetch-ocr-models.sh tiny      # ppocrv6-tiny
#   MIRA_OCR_MODELS=/path ./scripts/fetch-ocr-models.sh

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

set_name="ppocrv6-small"
case "${1:-}" in
  ""|small) set_name="ppocrv6-small" ;;
  tiny) set_name="ppocrv6-tiny" ;;
  *) set_name="$1" ;;
esac

dest="${MIRA_OCR_MODELS:-$root/vendor/ocr}"
mkdir -p "$dest"

echo "fetching model set '$set_name' → $dest"
cargo run --quiet --bin mira-fetch-ocr -- "$dest" "$set_name"
echo "resolve check: MIRA_OCR_MODELS=$dest cargo test -q --lib ocr::"
