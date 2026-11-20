//! Flicker-Free Software Cursor Compositing.
//!
//! Renders a sleek anti-aliased pointer with transparent background restore,
//! preventing tearing and artifacts during rapid mouse movement.

use super::surface::{Rect, Surface};

const SIZE: usize = 24;
const ARROW: &[u8; SIZE * SIZE * 4] = include_bytes!("../../assets/cursors/zen/default.rgba");
const GRAB: &[u8; SIZE * SIZE * 4] = include_bytes!("../../assets/cursors/zen/dnd-move.rgba");

pub struct Cursor {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub grabbing: bool,
    pub monochrome: bool,
    saved_bg: [u32; SIZE * SIZE],
    saved_x: i32,
    saved_y: i32,
    saved_w: usize,
    saved_h: usize,
    is_saved: bool,
}

impl Cursor {
    pub const fn new() -> Self {
        Self {
            x: 100,
            y: 100,
            visible: true,
            grabbing: false,
            monochrome: false,
            saved_bg: [0; SIZE * SIZE],
            saved_x: 0,
            saved_y: 0,
            saved_w: 0,
            saved_h: 0,
            is_saved: false,
        }
    }

    pub fn bounds(&self) -> Rect {
        let (hot_x, hot_y) = if self.grabbing { (12, 12) } else { (1, 1) };
        Rect::new(self.x - hot_x, self.y - hot_y, SIZE, SIZE)
    }

    pub fn saved_bounds(&self) -> Rect {
        if self.is_saved {
            Rect::new(self.saved_x, self.saved_y, self.saved_w, self.saved_h)
        } else {
            Rect::new(0, 0, 0, 0)
        }
    }

    /// Restore previously saved background underneath cursor
    pub fn restore(&mut self, dst: &mut Surface) {
        if !self.is_saved || !dst.is_valid() || self.saved_w == 0 || self.saved_h == 0 {
            return;
        }

        let x1 = self.saved_x.max(0) as usize;
        let y1 = self.saved_y.max(0) as usize;
        let x2 = ((self.saved_x + self.saved_w as i32).max(0) as usize).min(dst.width);
        let y2 = ((self.saved_y + self.saved_h as i32).max(0) as usize).min(dst.height);

        let copy_w = x2.saturating_sub(x1);
        let copy_h = y2.saturating_sub(y1);

        unsafe {
            for row in 0..copy_h {
                let dst_row = (y1 + row) * dst.stride + x1;
                for col in 0..copy_w {
                    *dst.pixels.add(dst_row + col) = self.saved_bg[row * SIZE + col];
                }
            }
        }

        self.is_saved = false;
    }

    /// Save background and draw cursor at current position
    pub fn draw(&mut self, dst: &mut Surface) {
        if !self.visible || !dst.is_valid() {
            return;
        }

        let bounds = self.bounds();
        let x1 = bounds.x.max(0) as usize;
        let y1 = bounds.y.max(0) as usize;
        let x2 = (bounds.right().max(0) as usize).min(dst.width);
        let y2 = (bounds.bottom().max(0) as usize).min(dst.height);
        if x1 >= x2 || y1 >= y2 {
            return;
        }

        self.saved_x = x1 as i32;
        self.saved_y = y1 as i32;
        self.saved_w = x2 - x1;
        self.saved_h = y2 - y1;
        self.is_saved = true;

        let sprite = if self.grabbing { GRAB } else { ARROW };
        for y in y1..y2 {
            for x in x1..x2 {
                let pixel = unsafe { *dst.pixels.add(y * dst.stride + x) };
                self.saved_bg[(y - y1) * SIZE + x - x1] = pixel;
                let index =
                    ((y as i32 - bounds.y) as usize * SIZE + (x as i32 - bounds.x) as usize) * 4;
                let [r, g, b, a] = sprite[index..index + 4].try_into().unwrap();
                if a != 0 {
                    let color = u32::from_be_bytes([a, r, g, b]);
                    let color = if self.monochrome {
                        (u32::from(a) << 24) | super::blend::gray_level(color) * 0x0001_0101
                    } else {
                        color
                    };
                    dst.blend_pixel(x, y, color);
                }
            }
        }
    }

    /// Update coordinates, restore background, and redraw cursor without tearing
    pub fn move_to(&mut self, dst: &mut Surface, new_x: i32, new_y: i32) {
        if self.x == new_x && self.y == new_y {
            return;
        }
        self.restore(dst);
        self.x = new_x;
        self.y = new_y;
        self.draw(dst);
    }

    /// Return bounding rect of the cursor for damage invalidation
    pub fn rect(&self) -> Rect {
        self.bounds()
    }
}
