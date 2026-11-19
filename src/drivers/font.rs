//! Systemwide Font Engine and Manager for PurpleOS / Aegis64.
//!
//! Provides typography rendering, dynamic font face selection (Inter default,
//! Classic VGA fallback), weights (Regular, Bold), and subpixel anti-aliased
//! alpha blending.

use core::sync::atomic::{AtomicU8, Ordering};
use super::font8x16::FONT8X16;
use super::inter_font::{
    FONT_INTER_REGULAR_8X16, FONT_INTER_BOLD_8X16,
    FONT_INTER_REGULAR_ALPHA, FONT_INTER_BOLD_ALPHA,
};

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
            let alpha_table = match weight {
                FontWeight::Regular => &FONT_INTER_REGULAR_ALPHA[c as usize],
                FontWeight::Bold => &FONT_INTER_BOLD_ALPHA[c as usize],
            };

            for row in 0..16 {
                let py = y + row;
                let fb_row_offset = py * fb_width + x;
                for col in 0..8 {
                    let a = alpha_table[row][col];
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
