//! Surface and Geometry Primitives for Aegis64 Native Compositor.
//!
//! Provides off-screen pixel buffers, 2D clipping rectangles, damage region
//! tracking, and high-performance pixel operations without external dependencies.

use crate::drivers::dma::{alloc_coherent, free_coherent};
use crate::drivers::font8x16::FONT8X16;

/// 2D Axis-Aligned Bounding Box (Rectangle)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    #[inline(always)]
    pub const fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    #[inline(always)]
    pub const fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }

    /// Check if this rectangle contains point (px, py)
    #[inline(always)]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Check if two rectangles overlap
    #[inline(always)]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Calculate the intersection bounding box of two rectangles
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());

        if x1 < x2 && y1 < y2 {
            Some(Rect {
                x: x1,
                y: y1,
                w: (x2 - x1) as usize,
                h: (y2 - y1) as usize,
            })
        } else {
            None
        }
    }

    /// Calculate the minimal bounding box enclosing both rectangles
    pub fn union(&self, other: &Rect) -> Rect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = self.right().max(other.right());
        let y2 = self.bottom().max(other.bottom());

        Rect {
            x: x1,
            y: y1,
            w: (x2 - x1) as usize,
            h: (y2 - y1) as usize,
        }
    }
}

/// Tracks damaged (dirty) screen regions to minimize blitting overhead
#[derive(Copy, Clone, Debug)]
pub struct DamageTracker {
    pub dirty: Option<Rect>,
}

impl DamageTracker {
    pub const fn new() -> Self {
        Self { dirty: None }
    }

    pub fn mark_dirty(&mut self, rect: Rect) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        match self.dirty {
            Some(existing) => self.dirty = Some(existing.union(&rect)),
            None => self.dirty = Some(rect),
        }
    }

    pub fn reset(&mut self) {
        self.dirty = None;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.is_some()
    }
}

/// ARGB8888 Pixel Surface (Off-Screen Framebuffer or Mapped Display)
pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub stride: usize, // Pixels per horizontal scanline
    pub pixels: *mut u32,
    pub is_allocated: bool,
    pub byte_size: usize,
}

unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

impl Surface {
    /// Allocate a new offscreen surface using coherent physical memory
    pub fn new(width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let byte_size = width.saturating_mul(height).saturating_mul(core::mem::size_of::<u32>());
        let (ptr, _) = unsafe { alloc_coherent(byte_size)? };

        Some(Self {
            width,
            height,
            stride: width,
            pixels: ptr as *mut u32,
            is_allocated: true,
            byte_size,
        })
    }

    /// Wrap an existing unowned memory region (e.g., hardware linear framebuffer)
    pub const fn from_raw(width: usize, height: usize, stride: usize, pixels: *mut u32) -> Self {
        Self {
            width,
            height,
            stride,
            pixels,
            is_allocated: false,
            byte_size: 0,
        }
    }

    /// Empty uninitialized surface handle
    pub const fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            stride: 0,
            pixels: core::ptr::null_mut(),
            is_allocated: false,
            byte_size: 0,
        }
    }

    /// Deallocate physical memory if this surface owns its buffer
    pub fn destroy(&mut self) {
        if self.is_allocated && !self.pixels.is_null() && self.byte_size > 0 {
            unsafe {
                free_coherent(self.pixels as *mut u8, self.byte_size);
            }
            self.pixels = core::ptr::null_mut();
            self.is_allocated = false;
            self.byte_size = 0;
            self.width = 0;
            self.height = 0;
            self.stride = 0;
        }
    }

    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        !self.pixels.is_null() && self.width > 0 && self.height > 0
    }

    /// Clear entire surface with a solid color
    pub fn clear(&mut self, color: u32) {
        if !self.is_valid() {
            return;
        }
        let total_pixels = self.stride * self.height;
        unsafe {
            for i in 0..total_pixels {
                *self.pixels.add(i) = color;
            }
        }
    }

    /// Read pixel at coordinate (x, y)
    #[inline(always)]
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height && !self.pixels.is_null() {
            unsafe { *self.pixels.add(y * self.stride + x) }
        } else {
            0
        }
    }

    /// Set pixel at coordinate (x, y) with boundary checking
    #[inline(always)]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height && !self.pixels.is_null() {
            unsafe {
                *self.pixels.add(y * self.stride + x) = color;
            }
        }
    }

    /// Blend pixel at coordinate (x, y) using SourceOver alpha blending
    #[inline(always)]
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height && !self.pixels.is_null() {
            unsafe {
                let offset = y * self.stride + x;
                let dst = *self.pixels.add(offset);
                *self.pixels.add(offset) = super::blend::blend_pixels(color, dst);
            }
        }
    }

    /// Fill a rectangle with a solid color, clipped to surface boundaries
    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        if !self.is_valid() || rect.w == 0 || rect.h == 0 {
            return;
        }

        let x1 = rect.x.max(0) as usize;
        let y1 = rect.y.max(0) as usize;
        let x2 = (rect.right().max(0) as usize).min(self.width);
        let y2 = (rect.bottom().max(0) as usize).min(self.height);

        if x1 >= x2 || y1 >= y2 {
            return;
        }

        let fill_w = x2 - x1;
        unsafe {
            for py in y1..y2 {
                let row_ptr = self.pixels.add(py * self.stride + x1);
                for px in 0..fill_w {
                    *row_ptr.add(px) = color;
                }
            }
        }
    }

    /// Draw a line using Bresenham's algorithm
    pub fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && (x0 as usize) < self.width && y0 >= 0 && (y0 as usize) < self.height {
                self.set_pixel(x0 as usize, y0 as usize, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Draw a single ASCII character onto the surface
    pub fn draw_char(&mut self, x: usize, y: usize, c: u8, fg: u32, bg: u32, transparent_bg: bool) {
        if !self.is_valid() || x + 8 > self.width || y + 16 > self.height {
            return;
        }

        let glyph = FONT8X16[c as usize];
        for row in 0..16 {
            let bits = glyph[row];
            let py = y + row;
            let offset = py * self.stride + x;
            for col in 0..8 {
                let is_fg = (bits & (0x80 >> col)) != 0;
                if is_fg {
                    unsafe { *self.pixels.add(offset + col) = fg; }
                } else if !transparent_bg {
                    unsafe { *self.pixels.add(offset + col) = bg; }
                }
            }
        }
    }

    /// Draw a text string onto the surface
    pub fn draw_text(&mut self, mut x: usize, y: usize, text: &str, fg: u32, bg: u32, transparent_bg: bool) {
        for b in text.bytes() {
            if b == b'\n' {
                break;
            }
            if x + 8 > self.width {
                break;
            }
            self.draw_char(x, y, b, fg, bg, transparent_bg);
            x += 8;
        }
    }

    /// Blit a rectangular region from src to this surface with bounds clipping
    pub fn blit(&mut self, src: &Surface, src_rect: Rect, dst_x: i32, dst_y: i32, alpha: bool) {
        if !self.is_valid() || !src.is_valid() || src_rect.w == 0 || src_rect.h == 0 {
            return;
        }

        // Clamp src_rect inside source surface bounds
        let s_x = src_rect.x.max(0) as usize;
        let s_y = src_rect.y.max(0) as usize;
        let s_r = (src_rect.right().max(0) as usize).min(src.width);
        let s_b = (src_rect.bottom().max(0) as usize).min(src.height);

        if s_x >= s_r || s_y >= s_b {
            return;
        }

        let copy_w = s_r - s_x;
        let copy_h = s_b - s_y;

        // Clip destination coordinates
        let d_x1 = dst_x.max(0) as usize;
        let d_y1 = dst_y.max(0) as usize;
        let d_x2 = ((dst_x + copy_w as i32).max(0) as usize).min(self.width);
        let d_y2 = ((dst_y + copy_h as i32).max(0) as usize).min(self.height);

        if d_x1 >= d_x2 || d_y1 >= d_y2 {
            return;
        }

        let actual_w = d_x2 - d_x1;
        let actual_h = d_y2 - d_y1;

        let src_start_x = if dst_x < 0 { s_x + (-dst_x as usize) } else { s_x };
        let src_start_y = if dst_y < 0 { s_y + (-dst_y as usize) } else { s_y };

        unsafe {
            for row in 0..actual_h {
                let src_row = (src_start_y + row) * src.stride + src_start_x;
                let dst_row = (d_y1 + row) * self.stride + d_x1;

                if alpha {
                    for col in 0..actual_w {
                        let s_px = *src.pixels.add(src_row + col);
                        let d_px = *self.pixels.add(dst_row + col);
                        *self.pixels.add(dst_row + col) = super::blend::blend_pixels(s_px, d_px);
                    }
                } else {
                    core::ptr::copy_nonoverlapping(
                        src.pixels.add(src_row),
                        self.pixels.add(dst_row),
                        actual_w,
                    );
                }
            }
        }
    }
}
