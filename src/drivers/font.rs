//! Systemwide Font Engine and Manager for PurpleOS / Aegis64.
//!
//! Provides typography rendering, dynamic font face selection (Inter default,
//! Classic VGA fallback), weights (Regular, Bold), and subpixel anti-aliased
//! alpha blending.

use super::font8x16::FONT8X16;
use super::inter_font::{
    FONT_INTER_BOLD_8X16, FONT_INTER_BOLD_ALPHA, FONT_INTER_REGULAR_8X16, FONT_INTER_REGULAR_ALPHA,
};
use core::sync::atomic::{AtomicU8, Ordering};

#[path = "../../sdk/gui/inter.rs"]
pub mod inter;

pub const DESKTOP_HEIGHT: usize = inter::HEIGHT;
static INTER_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static mut DESKTOP_REGULAR: inter::Inter = inter::Inter::EMPTY;
static mut DESKTOP_BOLD: inter::Inter = inter::Inter::EMPTY;

/// Runs once on the boot CPU before the desktop or framebuffer console start.
pub fn init_desktop() {
    const REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter/Inter-Regular.ttf");
    const BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter/Inter-Bold.ttf");
    let ready = unsafe {
        (&mut *(&raw mut DESKTOP_REGULAR)).load(REGULAR).is_some()
            && (&mut *(&raw mut DESKTOP_BOLD)).load(BOLD).is_some()
    };
    INTER_READY.store(ready, Ordering::Release);
    if ready {
        crate::drivers::serial::println("Typeface: Inter Regular and Bold outlines loaded");
    }
}

pub fn desktop_face(weight: FontWeight) -> Option<&'static inter::Inter> {
    if get_active_font() != FontFace::Inter || !INTER_READY.load(Ordering::Acquire) {
        return None;
    }
    let face = match weight {
        FontWeight::Regular => &raw const DESKTOP_REGULAR,
        FontWeight::Bold => &raw const DESKTOP_BOLD,
    };
    Some(unsafe { &*face })
}

pub fn text_width(text: &str, weight: FontWeight) -> usize {
    desktop_face(weight).map_or_else(
        || text.chars().count() * FONT_WIDTH,
        |face| {
            text.chars()
                .map(|c| face.glyph(u8::try_from(c as u32).unwrap_or(b'?')).advance)
                .sum::<usize>()
                .div_ceil(inter::SUBPIXELS)
        },
    )
}

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 16;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FontFace {
    Inter,
    ClassicVga,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Bold,
}

pub static CURRENT_FONT: AtomicU8 = AtomicU8::new(0); // 0 = Inter, 1 = ClassicVga

#[inline(always)]
pub fn get_active_font() -> FontFace {
    match CURRENT_FONT.load(Ordering::Relaxed) {
        0 => FontFace::Inter,
        _ => FontFace::ClassicVga,
    }
}

pub fn set_active_font(face: FontFace) {
    let val = match face {
        FontFace::Inter => 0,
        FontFace::ClassicVga => 1,
    };
    CURRENT_FONT.store(val, Ordering::Relaxed);
}

#[inline(always)]
pub fn get_glyph_row(c: u8, row: usize, weight: FontWeight) -> u8 {
    let face = get_active_font();
    match face {
        FontFace::Inter => match weight {
            FontWeight::Regular => FONT_INTER_REGULAR_8X16[c as usize][row],
            FontWeight::Bold => FONT_INTER_BOLD_8X16[c as usize][row],
        },
        FontFace::ClassicVga => FONT8X16[c as usize][row],
    }
}

#[inline(always)]
pub fn get_glyph_alpha(c: u8, row: usize, col: usize, weight: FontWeight) -> u8 {
    if c.is_ascii_graphic() || c == b' ' {
        if let Some(face) = desktop_face(weight) {
            let glyph = face.glyph(c);
            let advance = glyph.advance.div_ceil(inter::SUBPIXELS).max(1);
            let projected = advance.min(FONT_WIDTH);
            let left = (FONT_WIDTH - projected) / 2;
            if col < left || col >= left + projected {
                return 0;
            }
            let source_x = ((2 * (col - left) + 1) * advance) / (2 * projected);
            let source_y = ((2 * row + 1) * DESKTOP_HEIGHT) / (2 * FONT_HEIGHT);
            let column = (source_x as isize - inter::Glyph::x_offset(0)) as usize;
            return glyph.coverage[source_y].get(column).copied().unwrap_or(0);
        }
    }
    let face = get_active_font();
    match face {
        FontFace::Inter => match weight {
            FontWeight::Regular => FONT_INTER_REGULAR_ALPHA[c as usize][row][col],
            FontWeight::Bold => FONT_INTER_BOLD_ALPHA[c as usize][row][col],
        },
        FontFace::ClassicVga => {
            let bits = FONT8X16[c as usize][row];
            if (bits & (0x80 >> col)) != 0 {
                255
            } else {
                0
            }
        }
    }
}

/// Anti-alias alpha blend foreground pixel onto destination pixel
#[inline(always)]
pub fn blend_alpha(fg: u32, bg: u32, alpha: u8) -> u32 {
    if alpha == 255 {
        return fg;
    }
    if alpha == 0 {
        return bg;
    }

    let a = alpha as u32;
    let inv_a = 255 - a;

    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;

    let bg_r = (bg >> 16) & 0xFF;
    let bg_g = (bg >> 8) & 0xFF;
    let bg_b = bg & 0xFF;

    let out_r = ((fg_r * a + bg_r * inv_a) + 127) / 255;
    let out_g = ((fg_g * a + bg_g * inv_a) + 127) / 255;
    let out_b = ((fg_b * a + bg_b * inv_a) + 127) / 255;

    0xFF000000 | (out_r << 16) | (out_g << 8) | out_b
}

/// Standard draw_char for kernel framebuffers with anti-aliasing
pub fn draw_char(
    fb: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    x: usize,
    y: usize,
    c: u8,
    fg: u32,
    bg: u32,
) {
    draw_char_antialiased(
        fb,
        fb_width,
        fb_height,
        x,
        y,
        c,
        fg,
        bg,
        bg == 0,
        FontWeight::Regular,
    );
}

/// High-fidelity anti-aliased character rasterizer with subpixel alpha blending
pub fn draw_char_antialiased(
    fb: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    x: usize,
    y: usize,
    c: u8,
    fg: u32,
    bg: u32,
    transparent_bg: bool,
    weight: FontWeight,
) {
    if x + FONT_WIDTH > fb_width || y + FONT_HEIGHT > fb_height {
        return;
    }

    let face = get_active_font();
    match face {
        FontFace::Inter => {
            for row in 0..16 {
                let py = y + row;
                let fb_row_offset = py * fb_width + x;
                for col in 0..8 {
                    let a = get_glyph_alpha(c, row, col, weight);
                    let px_idx = fb_row_offset + col;
                    if a > 0 {
                        let dst = if transparent_bg { fb[px_idx] } else { bg };
                        fb[px_idx] = blend_alpha(fg, dst, a);
                    } else if !transparent_bg {
                        fb[px_idx] = bg;
                    }
                }
            }
        }
        FontFace::ClassicVga => {
            let glyph = FONT8X16[c as usize];
            for row in 0..16 {
                let bits = glyph[row];
                let py = y + row;
                let fb_row_offset = py * fb_width + x;
                for col in 0..8 {
                    let is_fg = (bits & (0x80 >> col)) != 0;
                    let px_idx = fb_row_offset + col;
                    if is_fg {
                        fb[px_idx] = fg;
                    } else if !transparent_bg {
                        fb[px_idx] = bg;
                    }
                }
            }
        }
    }
}
