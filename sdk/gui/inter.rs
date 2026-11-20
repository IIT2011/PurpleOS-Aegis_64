//! Shared Inter outline rasterizer for Purple Desktop and native GUI apps.

use ttf_parser::{Face, OutlineBuilder};

pub const HEIGHT: usize = 20;
pub const SUBPIXELS: usize = 64;
const WIDTH: usize = 24;
const LEFT_PAD: usize = 3;
const BASELINE: f32 = 16.0;
const FONT_PIXELS: f32 = 17.0;
const SAMPLE: usize = 4;
const CURVE_STEPS: usize = 10;
const MAX_EDGES: usize = 1024;

#[derive(Clone, Copy)]
pub struct Glyph {
    pub advance: usize,
    pub coverage: [[u8; WIDTH]; HEIGHT],
}

impl Glyph {
    const EMPTY: Self = Self {
        advance: 0,
        coverage: [[0; WIDTH]; HEIGHT],
    };

    pub fn x_offset(column: usize) -> isize {
        column as isize - LEFT_PAD as isize
    }
}

pub struct Inter {
    glyphs: [Glyph; 95],
}

impl Inter {
    pub const EMPTY: Self = Self {
        glyphs: [Glyph::EMPTY; 95],
    };

    pub fn load(&mut self, bytes: &[u8]) -> Option<()> {
        let face = Face::parse(bytes, 0).ok()?;
        let scale = FONT_PIXELS / f32::from(face.units_per_em());
        let mut outline = Outline::new(scale);
        for (byte, glyph) in (32u8..=126).zip(self.glyphs.iter_mut()) {
            let id = face.glyph_index(char::from(byte))?;
            let width = f32::from(face.glyph_hor_advance(id)?) * scale;
            glyph.advance = (width * SUBPIXELS as f32 + 0.5) as usize;
            outline.count = 0;
            outline.overflow = false;
            face.outline_glyph(id, &mut outline);
            if outline.overflow {
                return None;
            }
            outline.raster(&mut glyph.coverage);
        }
        Some(())
    }

    pub fn glyph(&self, byte: u8) -> &Glyph {
        let byte = if (32..=126).contains(&byte) {
            byte
        } else {
            b'?'
        };
        &self.glyphs[usize::from(byte - 32)]
    }

    pub fn measure(&self, text: &[u8]) -> usize {
        text.iter().map(|&byte| self.glyph(byte).advance).sum()
    }
}

#[derive(Clone, Copy)]
struct Point(f32, f32);

impl Point {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
        )
    }
}

struct Outline {
    edges: [(Point, Point); MAX_EDGES],
    count: usize,
    origin: Point,
    current: Point,
    scale: f32,
    overflow: bool,
}

impl Outline {
    fn new(scale: f32) -> Self {
        const ZERO: Point = Point(0.0, 0.0);
        Self {
            edges: [(ZERO, ZERO); MAX_EDGES],
            count: 0,
            origin: ZERO,
            current: ZERO,
            scale,
            overflow: false,
        }
    }

    fn point(&self, x: f32, y: f32) -> Point {
        Point(x * self.scale + LEFT_PAD as f32, BASELINE - y * self.scale)
    }

    fn line(&mut self, end: Point) {
        if self.current.1 != end.1 {
            if let Some(edge) = self.edges.get_mut(self.count) {
                *edge = (self.current, end);
                self.count += 1;
            } else {
                self.overflow = true;
            }
        }
        self.current = end;
    }

    fn raster(&self, coverage: &mut [[u8; WIDTH]; HEIGHT]) {
        for (row, pixels) in coverage.iter_mut().enumerate() {
            let mut hits = [0u8; WIDTH];
            for sample_y in 0..SAMPLE {
                let y = row as f32 + (sample_y as f32 + 0.5) / SAMPLE as f32;
                let mut changes = [0i16; SAMPLE * WIDTH];
                for &(start, end) in &self.edges[..self.count] {
                    let (top, bottom, direction) = if start.1 < end.1 {
                        (start, end, 1)
                    } else {
                        (end, start, -1)
                    };
                    if y < top.1 || y >= bottom.1 {
                        continue;
                    }
                    let x = top.0 + (y - top.1) * (bottom.0 - top.0) / (bottom.1 - top.1);
                    // Start winding at the first sample center to the right of this edge.
                    let index = ((x * SAMPLE as f32 + 0.5) as usize).min(changes.len());
                    if let Some(change) = changes.get_mut(index) {
                        *change += direction;
                    }
                }
                let mut winding = 0;
                for (index, change) in changes.into_iter().enumerate() {
                    winding += change;
                    if winding != 0 {
                        hits[index / SAMPLE] += 1;
                    }
                }
            }
            for (pixel, count) in pixels.iter_mut().zip(hits) {
                *pixel = (u16::from(count) * 255 / (SAMPLE * SAMPLE) as u16) as u8;
            }
        }
    }
}

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.origin = self.point(x, y);
        self.current = self.origin;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.line(self.point(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let start = self.current;
        let control = self.point(x1, y1);
        let end = self.point(x, y);
        for step in 1..=CURVE_STEPS {
            let t = step as f32 / CURVE_STEPS as f32;
            self.line(start.lerp(control, t).lerp(control.lerp(end, t), t));
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let start = self.current;
        let control1 = self.point(x1, y1);
        let control2 = self.point(x2, y2);
        let end = self.point(x, y);
        for step in 1..=CURVE_STEPS {
            let t = step as f32 / CURVE_STEPS as f32;
            let middle = control1.lerp(control2, t);
            self.line(
                start
                    .lerp(control1, t)
                    .lerp(middle, t)
                    .lerp(middle.lerp(control2.lerp(end, t), t), t),
            );
        }
    }

    fn close(&mut self) {
        self.line(self.origin);
    }
}
