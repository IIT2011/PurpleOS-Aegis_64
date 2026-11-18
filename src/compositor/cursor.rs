//! Flicker-Free Software Cursor Compositing.
//!
//! Renders a sleek anti-aliased pointer with transparent background restore,
//! preventing tearing and artifacts during rapid mouse movement.

use super::surface::{Rect, Surface};

pub const CURSOR_WIDTH: usize = 16;
pub const CURSOR_HEIGHT: usize = 16;

// High-contrast modern arrow pointer (W = White outline, P = Purple body, . = Transparent)
// 16x16 pixel definition
const CURSOR_BITMAP: [&str; 16] = [
    "W...............",
    "WW..............",
    "WPW.............",
    "WPPW............",
    "WPPPW...........",
    "WPPPPW..........",
    "WPPPPPW.........",
    "WPPPPPPW........",
    "WPPPPPPPW.......",
    "WPPPPWwwww......",
    "WPPWPPW.........",
    "WPW.WPPW........",
    "WW...WPPW.......",
    "W.....WPPW......",
    ".......WW.......",
    "................",
];

const COLOR_WHITE: u32 = 0xFFFFFFFF;
const COLOR_BLACK: u32 = 0xFF000000;
const COLOR_PURPLE: u32 = 0xFF7E3FF2;
const COLOR_DARK: u32 = 0xFF2A1152;

pub struct Cursor {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    saved_bg: [u32; CURSOR_WIDTH * CURSOR_HEIGHT],
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
            saved_bg: [0; CURSOR_WIDTH * CURSOR_HEIGHT],
            saved_x: 0,
            saved_y: 0,
            saved_w: 0,
            saved_h: 0,
            is_saved: false,
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

        let bg_offset_x = if self.saved_x < 0 { (-self.saved_x) as usize } else { 0 };
        let bg_offset_y = if self.saved_y < 0 { (-self.saved_y) as usize } else { 0 };

        unsafe {
            for row in 0..copy_h {
                let dst_row = (y1 + row) * dst.stride + x1;
                let bg_row = (bg_offset_y + row) * CURSOR_WIDTH + bg_offset_x;
                for col in 0..copy_w {
                    *dst.pixels.add(dst_row + col) = self.saved_bg[bg_row + col];
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

        // 1. Save background pixels at (self.x, self.y)
        let x1 = self.x.max(0) as usize;
        let y1 = self.y.max(0) as usize;
        let x2 = ((self.x + CURSOR_WIDTH as i32).max(0) as usize).min(dst.width);
        let y2 = ((self.y + CURSOR_HEIGHT as i32).max(0) as usize).min(dst.height);

        if x1 >= x2 || y1 >= y2 {
            return;
        }

        let copy_w = x2 - x1;
        let copy_h = y2 - y1;

        self.saved_x = self.x;
        self.saved_y = self.y;
        self.saved_w = copy_w;
        self.saved_h = copy_h;

        let bg_offset_x = if self.x < 0 { (-self.x) as usize } else { 0 };
        let bg_offset_y = if self.y < 0 { (-self.y) as usize } else { 0 };

        unsafe {
            for row in 0..copy_h {
                let dst_row = (y1 + row) * dst.stride + x1;
                let bg_row = (bg_offset_y + row) * CURSOR_WIDTH + bg_offset_x;
                for col in 0..copy_w {
                    self.saved_bg[bg_row + col] = *dst.pixels.add(dst_row + col);
                }
            }
        }
        self.is_saved = true;

        // 2. Draw pointer bitmap pixels
        for row in 0..CURSOR_HEIGHT {
            let py = self.y + row as i32;
            if py < 0 || py as usize >= dst.height {
                continue;
            }
            let line_bytes = CURSOR_BITMAP[row].as_bytes();

            for col in 0..CURSOR_WIDTH {
                let px = self.x + col as i32;
                if px < 0 || px as usize >= dst.width {
                    continue;
                }

                let color = match line_bytes[col] {
                    b'W' => COLOR_WHITE,
                    b'w' => COLOR_BLACK,
                    b'P' => COLOR_PURPLE,
                    b'p' => COLOR_DARK,
                    _ => continue, // Transparent
                };

                dst.set_pixel(px as usize, py as usize, color);
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
        Rect::new(self.x, self.y, CURSOR_WIDTH, CURSOR_HEIGHT)
    }
}
