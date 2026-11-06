//! Extract text from a capture (Lark "提取文字").

use std::io::Write;
use std::process::Command;

use image::RgbaImage;
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

/// Run OCR on `img`. Prefers `tesseract` when installed; on macOS falls back
/// to a short Vision framework Swift helper.
pub fn extract_text(img: &RgbaImage) -> Result<String, OcrError> {
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
    let text = platform_ocr(&path)?;
    let _ = std::fs::remove_file(&path);
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(OcrError::Message("no text recognized".into()));
    }
    Ok(text)
}

fn platform_ocr(path: &std::path::Path) -> Result<String, OcrError> {
    if which("tesseract") {
        let output = Command::new("tesseract")
            .args([path.to_string_lossy().as_ref(), "stdout", "-l", "eng+chi_sim"])
            .output()?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        // Retry English-only if chi_sim data is missing.
        let output = Command::new("tesseract")
            .args([path.to_string_lossy().as_ref(), "stdout", "-l", "eng"])
            .output()?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        return Err(OcrError::Message(format!(
            "tesseract failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    #[cfg(target_os = "macos")]
    {
        return macos_vision_ocr(path);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(OcrError::Message(
            "install tesseract (e.g. brew install tesseract tesseract-lang)".into(),
        ))
    }
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn macos_vision_ocr(path: &std::path::Path) -> Result<String, OcrError> {
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
