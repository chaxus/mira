//! Annotation tools aligned with Lark's screenshot toolbar (bitmap ops).

use image::{Rgba, RgbaImage};

/// Active tool while editing a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotateTool {
    Rect,
    Ellipse,
    #[default]
    Arrow,
    Pen,
    Text,
    Tag,
    Mosaic,
    Watermark,
}

impl AnnotateTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Arrow => "arrow",
            Self::Pen => "pen",
            Self::Text => "text",
            Self::Tag => "tag",
            Self::Mosaic => "mosaic",
            Self::Watermark => "watermark",
        }
    }
}

/// A committed annotation stroke / shape.
#[derive(Debug, Clone)]
pub enum Stroke {
    Rect {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: [u8; 4],
        thickness: u32,
    },
    Ellipse {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: [u8; 4],
        thickness: u32,
    },
    Arrow {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: [u8; 4],
        thickness: u32,
    },
    Pen {
        points: Vec<(i32, i32)>,
        color: [u8; 4],
        thickness: u32,
    },
    Mosaic {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        block: u32,
    },
}

/// Editable document: base frame + undoable strokes.
#[derive(Debug, Clone)]
pub struct Document {
    pub base: RgbaImage,
    pub strokes: Vec<Stroke>,
}

impl Document {
    pub fn new(base: RgbaImage) -> Self {
        Self {
            base,
            strokes: Vec::new(),
        }
    }

    pub fn width(&self) -> u32 {
        self.base.width()
    }

    pub fn height(&self) -> u32 {
        self.base.height()
    }

    pub fn push(&mut self, stroke: Stroke) {
        self.strokes.push(stroke);
    }

    pub fn undo(&mut self) -> bool {
        self.strokes.pop().is_some()
    }

    /// Flatten base + strokes into a new image.
    pub fn render(&self) -> RgbaImage {
        let mut out = self.base.clone();
        for stroke in &self.strokes {
            apply_stroke(&mut out, stroke);
        }
        out
    }
}

fn apply_stroke(img: &mut RgbaImage, stroke: &Stroke) {
    match stroke {
        Stroke::Rect {
            x0,
            y0,
            x1,
            y1,
            color,
            thickness,
        } => draw_rect_outline(img, *x0, *y0, *x1, *y1, *color, *thickness),
        Stroke::Ellipse {
            x0,
            y0,
            x1,
            y1,
            color,
            thickness,
        } => draw_ellipse_outline(img, *x0, *y0, *x1, *y1, *color, *thickness),
        Stroke::Arrow {
            x0,
            y0,
            x1,
            y1,
            color,
            thickness,
        } => {
            draw_line(img, *x0, *y0, *x1, *y1, *color, *thickness);
            draw_arrow_head(img, *x0, *y0, *x1, *y1, *color, *thickness);
        }
        Stroke::Pen {
            points,
            color,
            thickness,
        } => {
            for w in points.windows(2) {
                draw_line(img, w[0].0, w[0].1, w[1].0, w[1].1, *color, *thickness);
            }
        }
        Stroke::Mosaic {
            x0,
            y0,
            x1,
            y1,
            block,
        } => apply_mosaic(img, *x0, *y0, *x1, *y1, (*block).max(2)),
    }
}

fn put(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x < img.width() && y < img.height() {
        img.put_pixel(x, y, Rgba(color));
    }
}

fn draw_disk(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r2 {
                put(img, cx + dx, cy + dy, color);
            }
        }
    }
}

fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4], thickness: u32) {
    let radius = (thickness as i32 / 2).max(1);
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        draw_disk(img, x, y, radius, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_rect_outline(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    thickness: u32,
) {
    let (xa, xb) = (x0.min(x1), x0.max(x1));
    let (ya, yb) = (y0.min(y1), y0.max(y1));
    draw_line(img, xa, ya, xb, ya, color, thickness);
    draw_line(img, xb, ya, xb, yb, color, thickness);
    draw_line(img, xb, yb, xa, yb, color, thickness);
    draw_line(img, xa, yb, xa, ya, color, thickness);
}

fn draw_ellipse_outline(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    thickness: u32,
) {
    let (xa, xb) = (x0.min(x1), x0.max(x1));
    let (ya, yb) = (y0.min(y1), y0.max(y1));
    let rx = ((xb - xa) / 2).max(1);
    let ry = ((yb - ya) / 2).max(1);
    let cx = (xa + xb) / 2;
    let cy = (ya + yb) / 2;
    let steps = ((rx + ry) * 4).max(32);
    let mut prev = None;
    for i in 0..=steps {
        let t = std::f64::consts::TAU * (i as f64) / (steps as f64);
        let x = cx + (rx as f64 * t.cos()).round() as i32;
        let y = cy + (ry as f64 * t.sin()).round() as i32;
        if let Some((px, py)) = prev {
            draw_line(img, px, py, x, y, color, thickness);
        }
        prev = Some((x, y));
    }
}

fn draw_arrow_head(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    thickness: u32,
) {
    let angle = ((y1 - y0) as f64).atan2((x1 - x0) as f64);
    let len = 12.0 + thickness as f64 * 2.0;
    for delta in [-0.5_f64, 0.5_f64] {
        let a = angle + std::f64::consts::PI + delta;
        let hx = x1 + (len * a.cos()).round() as i32;
        let hy = y1 + (len * a.sin()).round() as i32;
        draw_line(img, x1, y1, hx, hy, color, thickness);
    }
}

fn apply_mosaic(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, block: u32) {
    let (xa, xb) = (x0.min(x1).max(0) as u32, x0.max(x1).max(0) as u32);
    let (ya, yb) = (y0.min(y1).max(0) as u32, y0.max(y1).max(0) as u32);
    let xb = xb.min(img.width().saturating_sub(1));
    let yb = yb.min(img.height().saturating_sub(1));
    let mut y = ya;
    while y <= yb {
        let mut x = xa;
        while x <= xb {
            let x2 = (x + block - 1).min(xb);
            let y2 = (y + block - 1).min(yb);
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            let mut a = 0u32;
            let mut n = 0u32;
            for py in y..=y2 {
                for px in x..=x2 {
                    let p = img.get_pixel(px, py).0;
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                    a += p[3] as u32;
                    n += 1;
                }
            }
            if n > 0 {
                let avg = Rgba([(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]);
                for py in y..=y2 {
                    for px in x..=x2 {
                        img.put_pixel(px, py, avg);
                    }
                }
            }
            x = x.saturating_add(block);
            if x == 0 {
                break;
            }
        }
        y = y.saturating_add(block);
        if y == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn rect_marks_pixels() {
        let mut doc = Document::new(blank(40, 40));
        doc.push(Stroke::Rect {
            x0: 5,
            y0: 5,
            x1: 20,
            y1: 15,
            color: [255, 0, 0, 255],
            thickness: 2,
        });
        let out = doc.render();
        assert_eq!(out.get_pixel(5, 5).0, [255, 0, 0, 255]);
        // Interior of the rect outline stays white.
        assert_eq!(out.get_pixel(10, 10).0, [255, 255, 255, 255]);
    }

    #[test]
    fn undo_removes_last_stroke() {
        let mut doc = Document::new(blank(10, 10));
        doc.push(Stroke::Pen {
            points: vec![(1, 1), (2, 2)],
            color: [0, 0, 0, 255],
            thickness: 1,
        });
        assert!(doc.undo());
        assert!(!doc.undo());
    }

    #[test]
    fn mosaic_averages_block() {
        let mut img = blank(8, 8);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
        apply_mosaic(&mut img, 0, 0, 1, 1, 2);
        let a = img.get_pixel(0, 0).0;
        let b = img.get_pixel(1, 0).0;
        assert_eq!(a, b);
    }
}
