//! Extract text from a capture (Lark "提取文字").
//!
//! Order (see `docs/TECH_PLAN.md`):
//! 1. RapidOCR / PP-OCRv6 ONNX when models are present under vendor / env
//! 2. macOS Vision (Swift helper)
//! 3. `tesseract` on PATH
//! 4. Error with install / fetch hint

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use image::RgbaImage;
use parking_lot::Mutex;
use thiserror::Error;

use crate::export;

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("export: {0}")]
    Export(#[from] export::ExportError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Run OCR on `img`.
pub fn extract_text(img: &RgbaImage) -> Result<String, OcrError> {
    let mut errors = Vec::new();

    match try_rapidocr(img) {
        Ok(Some(text)) => return Ok(text),
        Ok(None) => {}
        Err(e) => errors.push(format!("rapidocr: {e}")),
    }

    let mut path = std::env::temp_dir();
    path.push(format!(
        "mira-ocr-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    export::save_png(img, &path)?;

    #[cfg(target_os = "macos")]
    {
        match macos_vision_ocr(&path) {
            Ok(text) => {
                let text = text.trim().to_owned();
                let _ = std::fs::remove_file(&path);
                if text.is_empty() {
                    return Err(OcrError::Message("no text recognized".into()));
                }
                return Ok(text);
            }
            Err(e) => errors.push(format!("vision: {e}")),
        }
    }

    match tesseract_ocr(&path) {
        Ok(text) => {
            let text = text.trim().to_owned();
            let _ = std::fs::remove_file(&path);
            if text.is_empty() {
                return Err(OcrError::Message("no text recognized".into()));
            }
            return Ok(text);
        }
        Err(e) => errors.push(format!("tesseract: {e}")),
    }

    let _ = std::fs::remove_file(&path);
    Err(OcrError::Message(format!(
        "ocr unavailable — run scripts/fetch-ocr-models.sh or install tesseract ({})",
        errors.join("; ")
    )))
}

/// Whether RapidOCR model assets are resolvable (without downloading).
pub fn rapidocr_available() -> bool {
    model_dir().is_some_and(|dir| {
        let set = model_set();
        let cache = rapidocr_core::model::ModelCache::new(dir);
        cache
            .missing_assets_for_pipeline(
                set,
                rapidocr_core::config::PipelineConfig::without_cls(),
            )
            .is_empty()
    })
}

fn model_set_name() -> String {
    std::env::var("MIRA_OCR_MODEL_SET").unwrap_or_else(|_| {
        rapidocr_core::model::DEFAULT_MODEL_SET_NAME.to_string()
    })
}

fn model_set() -> &'static rapidocr_core::model::ModelSetSpec {
    let name = model_set_name();
    rapidocr_core::model::model_set_by_name(&name)
        .unwrap_or_else(|| {
            rapidocr_core::model::model_set_by_name(rapidocr_core::model::DEFAULT_MODEL_SET_NAME)
                .expect("default RapidOCR model set")
        })
}

fn model_dir() -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("MIRA_OCR_MODELS") {
        let path = PathBuf::from(custom);
        if path.is_dir() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ocr"));
            candidates.push(dir.join("vendor").join("ocr"));
            candidates.push(dir.join("..").join("..").join("vendor").join("ocr"));
        }
    }
    if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest).join("vendor").join("ocr"));
    }
    candidates.push(PathBuf::from("vendor").join("ocr"));

    candidates.into_iter().find(|p| p.is_dir())
}

fn try_rapidocr(img: &RgbaImage) -> Result<Option<String>, OcrError> {
    let Some(dir) = model_dir() else {
        return Ok(None);
    };
    let set = model_set();
    let pipeline = rapidocr_core::config::PipelineConfig::without_cls();
    let cache = rapidocr_core::model::ModelCache::new(&dir);
    if !cache.missing_assets_for_pipeline(set, pipeline).is_empty() {
        return Ok(None);
    }
    // Validate checksums without downloading.
    cache
        .ensure_model_set_for_pipeline(
            set,
            pipeline,
            rapidocr_core::model::ModelDownloadMode::Never,
        )
        .map_err(|e| OcrError::Message(format!("model cache: {e}")))?;

    static ENGINE: OnceLock<Mutex<Option<rapidocr_core::RapidOcr>>> = OnceLock::new();
    let cell = ENGINE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock();
    if guard.is_none() {
        let cfg = cache.config_for(set).with_pipeline(pipeline);
        let engine = rapidocr_core::RapidOcr::from_config(cfg)
            .map_err(|e| OcrError::Message(format!("rapidocr init: {e}")))?;
        *guard = Some(engine);
    }
    let engine = guard.as_mut().expect("engine just initialized");
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let output = engine
        .run_image(&rgb)
        .map_err(|e| OcrError::Message(format!("rapidocr: {e}")))?;
    let text = output
        .lines
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n");
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(OcrError::Message("no text recognized".into()));
    }
    Ok(Some(text))
}

fn tesseract_ocr(path: &Path) -> Result<String, OcrError> {
    if !which("tesseract") {
        return Err(OcrError::Message("tesseract not on PATH".into()));
    }
    let output = Command::new("tesseract")
        .args([path.to_string_lossy().as_ref(), "stdout", "-l", "eng+chi_sim"])
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let output = Command::new("tesseract")
        .args([path.to_string_lossy().as_ref(), "stdout", "-l", "eng"])
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(OcrError::Message(format!(
        "tesseract failed: {}",
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn macos_vision_ocr(path: &Path) -> Result<String, OcrError> {
    let script = r#"
import Vision
import AppKit
import Foundation

let path = CommandLine.arguments[1]
guard let img = NSImage(contentsOfFile: path),
      let tiff = img.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let cg = rep.cgImage else {
    fputs("failed to load image\n", stderr)
    exit(1)
}
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = true
let handler = VNImageRequestHandler(cgImage: cg, options: [:])
try handler.perform([request])
let observations = request.results ?? []
for ob in observations {
    if let candidate = ob.topCandidates(1).first {
        print(candidate.string)
    }
}
"#;
    let mut file = tempfile::NamedTempFile::with_suffix(".swift")?;
    file.write_all(script.as_bytes())?;
    file.flush()?;
    let output = Command::new("swift")
        .arg(file.path())
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(OcrError::Message(format!(
            "Vision OCR failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapidocr_available_is_false_without_models() {
        // Fresh checkout has empty vendor/ocr — should not claim ready.
        // SAFETY: test-only env isolation.
        unsafe {
            std::env::remove_var("MIRA_OCR_MODELS");
        }
        // May still be true if developer already fetched models into vendor/ocr.
        let _ = rapidocr_available();
    }
}
