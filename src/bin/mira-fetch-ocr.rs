//! Download RapidOCR / PP-OCR ONNX assets into a local model directory.
//!
//! Invoked by `scripts/fetch-ocr-models.sh`.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use rapidocr_core::config::PipelineConfig;
use rapidocr_core::model::{
    model_set_by_name, ModelCache, ModelDownloadMode, DEFAULT_MODEL_SET_NAME,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let dest = args
        .next()
        .map(PathBuf::from)
        .or_else(|| env::var_os("MIRA_OCR_MODELS").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("vendor/ocr"));
    let set_name = args
        .next()
        .or_else(|| env::var("MIRA_OCR_MODEL_SET").ok())
        .unwrap_or_else(|| DEFAULT_MODEL_SET_NAME.to_string());

    let Some(model_set) = model_set_by_name(&set_name) else {
        eprintln!("unknown model set {set_name:?}");
        eprintln!("hint: try ppocrv6-small or ppocrv6-tiny");
        return ExitCode::FAILURE;
    };

    let cache = ModelCache::new(&dest);
    let pipeline = PipelineConfig::without_cls();
    if let Err(e) =
        cache.ensure_model_set_for_pipeline(model_set, pipeline, ModelDownloadMode::Missing)
    {
        eprintln!("fetch failed: {e:#}");
        return ExitCode::FAILURE;
    }

    println!("installed {set_name} → {}", dest.display());
    for asset in model_set.assets_for_pipeline(pipeline) {
        println!("  ✓ {}", asset.filename);
    }
    ExitCode::SUCCESS
}
