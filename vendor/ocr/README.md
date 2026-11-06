# RapidOCR / PP-OCRv6 models (optional)

Mira prefers **RapidOCR-style ONNX** models when this directory (or `MIRA_OCR_MODELS`) contains a complete set. Otherwise OCR falls back to macOS Vision / `tesseract`.

## Layout

Files are **gitignored**. Fetch with:

```bash
./scripts/fetch-ocr-models.sh          # default: ppocrv6-small
./scripts/fetch-ocr-models.sh tiny     # smaller / faster
```

Runtime resolution:

1. `MIRA_OCR_MODELS` — absolute directory override  
2. `vendor/ocr` (next to the binary or crate root)  
3. Fall back to Vision / tesseract  

Optional: `MIRA_OCR_MODEL_SET` (`ppocrv6-small` default, or `ppocrv6-tiny`, … — see `rapidocr-core::model::available_model_set_names`).

## License

PP-OCR ONNX weights redistributed by RapidAI are **Apache-2.0**. Keep attribution when bundling in releases.

## Size

Expect roughly **tens of MB** for `ppocrv6-small` (det + rec + dict). Prefer tiny for slim installs.