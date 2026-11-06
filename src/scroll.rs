//! Scroll / long capture — stitch successive fullscreen frames vertically.

use image::{RgbaImage, imageops};

use crate::capture::{CaptureError, CaptureMode};
use crate::chrome::HideGuard;

/// In-progress scroll capture session.
#[derive(Debug, Clone)]
pub struct ScrollSession {
    frames: Vec<RgbaImage>,
}

impl ScrollSession {
    /// Take the first fullscreen frame.
    pub fn begin() -> Result<Self, CaptureError> {
        let _hidden = HideGuard::acquire();
        let frame = capture_fullscreen_raw()?;
        Ok(Self {
            frames: vec![frame],
        })
    }

    /// Append another fullscreen frame (caller scrolls between steps).
    pub fn add_step(&mut self) -> Result<(), CaptureError> {
        let _hidden = HideGuard::acquire();
        let frame = capture_fullscreen_raw()?;
        self.frames.push(frame);
        Ok(())
    }

    pub fn steps(&self) -> usize {
        self.frames.len()
    }

    /// Stitch frames top-to-bottom with overlap detection.
    pub fn finish(self) -> Result<RgbaImage, CaptureError> {
        if self.frames.is_empty() {
            return Err(CaptureError::Message("no scroll frames".into()));
        }
        let mut out = self.frames[0].clone();
        for next in self.frames.into_iter().skip(1) {
            out = stitch_vertical(&out, &next);
        }
        Ok(out)
    }
}

fn capture_fullscreen_raw() -> Result<RgbaImage, CaptureError> {
    // Bypass capture::capture to avoid nested HideGuard / Scroll rejection.
    match CaptureMode::Fullscreen {
        CaptureMode::Fullscreen => {
            let path = {
                let mut path = std::env::temp_dir();
                path.push(format!(
                    "mira-scroll-{}-{}.png",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                ));
                path
            };
            crate::capture::platform_capture_fullscreen_for_scroll(&path)?;
            let img = image::open(&path)
                .map_err(|e| CaptureError::Image(e.to_string()))?
                .to_rgba8();
            let _ = std::fs::remove_file(&path);
            Ok(img)
        }
        _ => unreachable!(),
    }
}

/// Find best overlap and append `next` below `top`.
pub fn stitch_vertical(top: &RgbaImage, next: &RgbaImage) -> RgbaImage {
    let width = top.width().min(next.width());
    let overlap = find_overlap(top, next, width);
    let new_h = top.height() + next.height().saturating_sub(overlap);
    let mut out = RgbaImage::new(width, new_h);
    imageops::overlay(&mut out, top, 0, 0);
    imageops::overlay(&mut out, next, 0, (top.height() - overlap) as i64);
    out
}

fn find_overlap(top: &RgbaImage, next: &RgbaImage, width: u32) -> u32 {
    let max_overlap = (top.height().min(next.height()) * 3 / 4).max(1);
    let sample = 8u32;
    let mut best = 0u32;
    let mut best_score = u64::MAX;
    let step = (max_overlap / 64).max(1);
    let mut o = 0u32;
    while o <= max_overlap {
        let mut score = 0u64;
        let mut n = 0u64;
        let y0 = top.height().saturating_sub(o);
        for dy in (0..o).step_by(sample as usize) {
            for x in (0..width).step_by(sample as usize) {
                let a = top.get_pixel(x, y0 + dy).0;
                let b = next.get_pixel(x, dy).0;
                let dr = a[0].abs_diff(b[0]) as u64;
                let dg = a[1].abs_diff(b[1]) as u64;
                let db = a[2].abs_diff(b[2]) as u64;
                score += dr + dg + db;
                n += 1;
            }
        }
        let avg = if n == 0 { u64::MAX } else { score / n };
        if avg < best_score {
            best_score = avg;
            best = o;
        }
        o += step;
    }
    // If frames barely overlap (score still high), append with a small default strip.
    if best_score > 40 {
        return (next.height() / 10).max(1).min(next.height() / 2);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn stitch_extends_height() {
        let a = RgbaImage::from_pixel(4, 4, Rgba([1, 0, 0, 255]));
        let mut b = RgbaImage::from_pixel(4, 4, Rgba([0, 1, 0, 255]));
        // Make top rows of b match bottom of a so overlap prefers matching.
        for x in 0..4 {
            for y in 0..2 {
                b.put_pixel(x, y, Rgba([1, 0, 0, 255]));
            }
        }
        let out = stitch_vertical(&a, &b);
        assert!(out.height() >= 4);
        assert!(out.height() <= 8);
        assert_eq!(out.width(), 4);
    }
}
