//! Pure no_std ARGB8888 2D graphics engine for Purple Files.
//! Uses integer-only math (no float/SSE/AVX) with true straight-alpha Porter-Duff
//! compositing so Live Aero backdrop glass (`0x110C`) renders cleanly.

use crate::font;

#[inline(always)]
pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[inline(always)]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    argb(0xFF, r, g, b)
}

#[inline(always)]
pub const fn with_alpha(c: u32, a: u8) -> u32 {
    (c & 0x00FF_FFFF) | ((a as u32) << 24)
}

#[inline(always)]
pub fn lerp_color(c0: u32, c1: u32, num: i32, den: i32) -> u32 {
    if den <= 0 || num <= 0 {
        return c0;
    }
    if num >= den {
        return c1;
    }
    let a0 = ((c0 >> 24) & 0xFF) as i32;
    let r0 = ((c0 >> 16) & 0xFF) as i32;
    let g0 = ((c0 >> 8) & 0xFF) as i32;
    let b0 = (c0 & 0xFF) as i32;

    let a1 = ((c1 >> 24) & 0xFF) as i32;
    let r1 = ((c1 >> 16) & 0xFF) as i32;
    let g1 = ((c1 >> 8) & 0xFF) as i32;
    let b1 = (c1 & 0xFF) as i32;

    let a = (a0 + (a1 - a0) * num / den) as u8;
    let r = (r0 + (r1 - r0) * num / den) as u8;
    let g = (g0 + (g1 - g0) * num / den) as u8;
    let b = (b0 + (b1 - b0) * num / den) as u8;
    argb(a, r, g, b)
}

#[inline(always)]
pub fn set_pixel(buf: &mut [u32], bw: i32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= bw {
        return;
    }
    let idx = (y as usize) * (bw as usize) + (x as usize);
    if idx < buf.len() {
        buf[idx] = color;
    }
}

#[inline(always)]
pub fn blend_pixel(buf: &mut [u32], bw: i32, x: i32, y: i32, src: u32) {
    if x < 0 || y < 0 || x >= bw {
        return;
    }
    let idx = (y as usize) * (bw as usize) + (x as usize);
    if idx >= buf.len() {
        return;
    }
    let sa = (src >> 24) & 0xFF;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        buf[idx] = src;
        return;
    }
    let dst = buf[idx];
    let da = (dst >> 24) & 0xFF;
    if da == 0 {
        buf[idx] = src;
        return;
    }

    let inv_sa = 255 - sa;
    let sr = (src >> 16) & 0xFF;
    let sg = (src >> 8) & 0xFF;
    let sb = src & 0xFF;

    let dr = (dst >> 16) & 0xFF;
    let dg = (dst >> 8) & 0xFF;
    let db = dst & 0xFF;

    if da == 255 {
        let r = (sr * sa + dr * inv_sa + 127) / 255;
        let g = (sg * sa + dg * inv_sa + 127) / 255;
        let b = (sb * sa + db * inv_sa + 127) / 255;
        buf[idx] = (0xFF << 24) | (r << 16) | (g << 8) | b;
    } else {
        let out_a = sa + (da * inv_sa + 127) / 255;
        if out_a == 0 {
            buf[idx] = 0;
            return;
        }
        let w_src = sa * 255;
        let w_dst = da * inv_sa;
        let denom = out_a * 255;
        let r = (sr * w_src + dr * w_dst + denom / 2) / denom;
        let g = (sg * w_src + dg * w_dst + denom / 2) / denom;
        let b = (sb * w_src + db * w_dst + denom / 2) / denom;
        buf[idx] = (out_a << 24) | (r << 16) | (g << 8) | b;
    }
}

pub fn clear(buf: &mut [u32], color: u32) {
    for px in buf.iter_mut() {
        *px = color;
    }
}

pub fn fill_rect(buf: &mut [u32], bw: i32, bh: i32, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(bw);
    let y1 = (y + h).min(bh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for py in y0..y1 {
        for px in x0..x1 {
            blend_pixel(buf, bw, px, py, color);
        }
    }
}

pub fn fill_rect_raw(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: u32,
) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(bw);
    let y1 = (y + h).min(bh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for py in y0..y1 {
        let row_start = (py as usize) * (bw as usize);
        for px in x0..x1 {
            buf[row_start + (px as usize)] = color;
        }
    }
}

/// Anti-aliased rounded rectangle using integer SDF coverage in the corners.
pub fn fill_rounded_rect(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    color: u32,
) {
    fill_rounded_rect_vgrad(buf, bw, bh, x, y, w, h, r, color, color, false);
}

pub fn fill_rounded_rect_raw(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    color: u32,
) {
    fill_rounded_rect_vgrad(buf, bw, bh, x, y, w, h, r, color, color, true);
}

pub fn fill_rounded_rect_vgrad(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    top_color: u32,
    bot_color: u32,
    raw_replace: bool,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = radius.min(w / 2).min(h / 2).max(0);
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(bw);
    let y1 = (y + h).min(bh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let r_outer_sq = r * r;
    let r_inner_sq = if r > 1 { (r - 1) * (r - 1) } else { 0 };
    let span = (r_outer_sq - r_inner_sq).max(1);

    for py in y0..y1 {
        let row_color = if top_color == bot_color {
            top_color
        } else {
            lerp_color(top_color, bot_color, py - y, h - 1)
        };
        let base_a = ((row_color >> 24) & 0xFF) as i32;

        let dy_corner = if py < y + r {
            (y + r) - py
        } else if py >= y + h - r {
            py - (y + h - r - 1)
        } else {
            0
        };

        for px in x0..x1 {
            let dx_corner = if px < x + r {
                (x + r) - px
            } else if px >= x + w - r {
                px - (x + w - r - 1)
            } else {
                0
            };

            if dx_corner == 0 || dy_corner == 0 || r == 0 {
                if raw_replace {
                    set_pixel(buf, bw, px, py, row_color);
                } else {
                    blend_pixel(buf, bw, px, py, row_color);
                }
            } else {
                let d2 = dx_corner * dx_corner + dy_corner * dy_corner;
                if d2 <= r_inner_sq {
                    if raw_replace {
                        set_pixel(buf, bw, px, py, row_color);
                    } else {
                        blend_pixel(buf, bw, px, py, row_color);
                    }
                } else if d2 < r_outer_sq {
                    let cov = (r_outer_sq - d2) * 255 / span;
                    let a = ((base_a * cov) / 255) as u8;
                    let c = with_alpha(row_color, a);
                    blend_pixel(buf, bw, px, py, c);
                }
            }
        }
    }
}

pub fn stroke_rounded_rect(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    color: u32,
) {
    if w <= 1 || h <= 1 {
        return;
    }
    let r = radius.min(w / 2).min(h / 2).max(0);
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(bw);
    let y1 = (y + h).min(bh);

    // Top & bottom straight edges
    for px in (x + r).max(0)..(x + w - r).min(bw) {
        if y >= 0 && y < bh {
            blend_pixel(buf, bw, px, y, color);
        }
        if y + h - 1 >= 0 && y + h - 1 < bh {
            blend_pixel(buf, bw, px, y + h - 1, color);
        }
    }
    // Left & right straight edges
    for py in (y + r).max(0)..(y + h - r).min(bh) {
        if x >= 0 && x < bw {
            blend_pixel(buf, bw, x, py, color);
        }
        if x + w - 1 >= 0 && x + w - 1 < bw {
            blend_pixel(buf, bw, x + w - 1, py, color);
        }
    }
    if r == 0 {
        return;
    }

    let r_outer = r * r;
    let r_inner = (r - 1).max(0) * (r - 1).max(0);
    for py in y0..y1 {
        let dy = if py < y + r {
            (y + r) - py
        } else if py >= y + h - r {
            py - (y + h - r - 1)
        } else {
            continue;
        };
        for px in x0..x1 {
            let dx = if px < x + r {
                (x + r) - px
            } else if px >= x + w - r {
                px - (x + w - r - 1)
            } else {
                continue;
            };
            let d2 = dx * dx + dy * dy;
            if d2 <= r_outer && d2 >= r_inner {
                blend_pixel(buf, bw, px, py, color);
            }
        }
    }
}

/// Anti-aliased filled circle centered at `(cx, cy)` with radius `r`.
pub fn fill_circle(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, r: i32, color: u32) {
    if r <= 0 {
        return;
    }
    let base_a = ((color >> 24) & 0xFF) as i32;
    let r2_outer = (r * 2 + 1) * (r * 2 + 1);
    let r2_inner = (r * 2 - 1) * (r * 2 - 1);
    let span = (r2_outer - r2_inner).max(1);

    let x0 = (cx - r - 1).max(0);
    let y0 = (cy - r - 1).max(0);
    let x1 = (cx + r + 2).min(bw);
    let y1 = (cy + r + 2).min(bh);

    for py in y0..y1 {
        let dy2 = (py - cy) * 2;
        for px in x0..x1 {
            let dx2 = (px - cx) * 2;
            let d2 = dx2 * dx2 + dy2 * dy2;
            if d2 <= r2_inner {
                blend_pixel(buf, bw, px, py, color);
            } else if d2 < r2_outer {
                let cov = (r2_outer - d2) * 255 / span;
                let a = ((base_a * cov) / 255) as u8;
                blend_pixel(buf, bw, px, py, with_alpha(color, a));
            }
        }
    }
}

pub fn draw_line_h(buf: &mut [u32], bw: i32, bh: i32, x: i32, y: i32, w: i32, color: u32) {
    if y < 0 || y >= bh || w <= 0 {
        return;
    }
    let x0 = x.max(0);
    let x1 = (x + w).min(bw);
    for px in x0..x1 {
        blend_pixel(buf, bw, px, y, color);
    }
}

pub fn draw_line_v(buf: &mut [u32], bw: i32, bh: i32, x: i32, y: i32, h: i32, color: u32) {
    if x < 0 || x >= bw || h <= 0 {
        return;
    }
    let y0 = y.max(0);
    let y1 = (y + h).min(bh);
    for py in y0..y1 {
        blend_pixel(buf, bw, x, py, color);
    }
}

// ============================================================================
// Purple Files Custom Icons & Window Controls
// ============================================================================

/// Draws the right-side macOS-style circular window controls:
/// [Minimize (amber)] [Maximize/Restore (green)] [Close (red)]
pub fn draw_mac_window_buttons_right(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    min_cx: i32,
    max_cx: i32,
    cls_cx: i32,
    cy: i32,
    hovered_btn: u8, // 0=none, 1=min, 2=max, 3=close
    is_maximized: bool,
) {
    let r = 7;

    // 1. Minimize (amber #FEBC2E)
    let min_col = if hovered_btn == 1 {
        rgb(0xFF, 0xD2, 0x57)
    } else {
        rgb(0xFE, 0xBC, 0x2E)
    };
    fill_circle(buf, bw, bh, min_cx, cy, r, min_col);
    if hovered_btn == 1 {
        draw_line_h(buf, bw, bh, min_cx - 3, cy, 7, rgb(0x5C, 0x3D, 0x00));
        draw_line_h(buf, bw, bh, min_cx - 3, cy + 1, 7, rgb(0x5C, 0x3D, 0x00));
    }

    // 2. Maximize / Restore (emerald #28C840)
    let max_col = if hovered_btn == 2 {
        rgb(0x42, 0xDF, 0x5B)
    } else {
        rgb(0x28, 0xC8, 0x40)
    };
    fill_circle(buf, bw, bh, max_cx, cy, r, max_col);
    if hovered_btn == 2 {
        let g = rgb(0x09, 0x47, 0x13);
        if is_maximized {
            // Restore: overlapping mini-squares
            draw_line_h(buf, bw, bh, max_cx - 3, cy - 2, 4, g);
            draw_line_h(buf, bw, bh, max_cx - 3, cy + 2, 4, g);
            draw_line_v(buf, bw, bh, max_cx - 3, cy - 2, 5, g);
            draw_line_v(buf, bw, bh, max_cx + 1, cy - 2, 5, g);
            draw_line_h(buf, bw, bh, max_cx - 1, cy - 3, 4, g);
            draw_line_v(buf, bw, bh, max_cx + 3, cy - 3, 4, g);
        } else {
            // Maximize: '+' / expand glyph
            draw_line_h(buf, bw, bh, max_cx - 3, cy, 7, g);
            draw_line_v(buf, bw, bh, max_cx, cy - 3, 7, g);
        }
    }

    // 3. Close (coral-red #FF5F57)
    let cls_col = if hovered_btn == 3 {
        rgb(0xFF, 0x7A, 0x73)
    } else {
        rgb(0xFF, 0x5F, 0x57)
    };
    fill_circle(buf, bw, bh, cls_cx, cy, r, cls_col);
    if hovered_btn == 3 {
        let g = rgb(0x5C, 0x0A, 0x06);
        for d in -3..=3 {
            blend_pixel(buf, bw, cls_cx + d, cy + d, g);
            blend_pixel(buf, bw, cls_cx + d, cy - d, g);
        }
    }
}

/// Stylized Purple Files logo badge for the headerbar.
pub fn draw_purple_files_logo(buf: &mut [u32], bw: i32, bh: i32, x: i32, y: i32) {
    // 22x18 glossy purple folder icon
    fill_rounded_rect(buf, bw, bh, x, y + 1, 9, 5, 2, rgb(0x93, 0x33, 0xEA));
    fill_rounded_rect_vgrad(
        buf,
        bw,
        bh,
        x,
        y + 3,
        22,
        15,
        4,
        rgb(0xA8, 0x55, 0xF7),
        rgb(0x6D, 0x28, 0xD9),
        false,
    );
    draw_line_h(buf, bw, bh, x + 2, y + 4, 18, argb(0x60, 0xFF, 0xFF, 0xFF));
}

/// Large 3D-shaded folder icon for Grid View or compact 16x14 for List View.
pub fn draw_folder_icon(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    size: i32,
    selected: bool,
) {
    if size <= 20 {
        // Compact 16x14 list icon
        fill_rounded_rect(buf, bw, bh, x, y + 1, 7, 4, 1, rgb(0x8B, 0x5C, 0xF6));
        fill_rounded_rect_vgrad(
            buf,
            bw,
            bh,
            x,
            y + 3,
            16,
            11,
            2,
            rgb(0xA7, 0x8B, 0xFA),
            rgb(0x7C, 0x3A, 0xED),
            false,
        );
        return;
    }

    // Grid View rich folder (size ~ 48)
    let w = size;
    let h = (size * 3) / 4;
    let top_y = y + (size - h) / 2;

    // Back tab
    fill_rounded_rect(
        buf,
        bw,
        bh,
        x + 2,
        top_y,
        w * 42 / 100,
        h / 3,
        4,
        rgb(0x7C, 0x3A, 0xED),
    );
    // Back plate
    fill_rounded_rect(
        buf,
        bw,
        bh,
        x + 2,
        top_y + 4,
        w - 4,
        h - 6,
        6,
        rgb(0x6D, 0x28, 0xD9),
    );
    // Subtle paper sheet peek
    fill_rounded_rect(
        buf,
        bw,
        bh,
        x + 6,
        top_y + 7,
        w - 12,
        8,
        2,
        rgb(0xED, 0xE9, 0xFE),
    );
    // Front gradient flap
    let top_c = if selected {
        rgb(0xC0, 0x84, 0xFC)
    } else {
        rgb(0xA8, 0x55, 0xF7)
    };
    let bot_c = rgb(0x6D, 0x28, 0xD9);
    fill_rounded_rect_vgrad(buf, bw, bh, x, top_y + 10, w, h - 10, 6, top_c, bot_c, false);
    // Glossy rim
    draw_line_h(
        buf,
        bw,
        bh,
        x + 4,
        top_y + 11,
        w - 8,
        argb(0x70, 0xFF, 0xFF, 0xFF),
    );
}

/// Rich file icon with accent badge and extension label.
pub fn draw_file_icon(
    buf: &mut [u32],
    bw: i32,
    bh: i32,
    x: i32,
    y: i32,
    size: i32,
    accent: u32,
    ext: &[u8],
) {
    if size <= 20 {
        // Compact 14x16 list icon
        fill_rounded_rect(buf, bw, bh, x + 1, y, 12, 15, 2, rgb(0x3B, 0x33, 0x55));
        stroke_rounded_rect(buf, bw, bh, x + 1, y, 12, 15, 2, argb(0x80, 0xC4, 0xB5, 0xFD));
        fill_rect(buf, bw, bh, x + 3, y + 10, 8, 3, accent);
        return;
    }

    let w = (size * 3) / 4;
    let h = size - 4;
    let fx = x + (size - w) / 2;
    let fy = y + 2;

    // Card body
    fill_rounded_rect_vgrad(
        buf,
        bw,
        bh,
        fx,
        fy,
        w,
        h,
        5,
        rgb(0x2E, 0x27, 0x47),
        rgb(0x20, 0x1B, 0x33),
        false,
    );
    stroke_rounded_rect(buf, bw, bh, fx, fy, w, h, 5, argb(0x66, 0xA7, 0x8B, 0xFA));

    // Folded corner accent
    fill_rounded_rect(buf, bw, bh, fx + w - 10, fy + 2, 8, 8, 2, argb(0x45, 0xFF, 0xFF, 0xFF));

    // Subtle text lines
    draw_line_h(buf, bw, bh, fx + 6, fy + 12, w - 18, argb(0x40, 0xE2, 0xE8, 0xF0));
    draw_line_h(buf, bw, bh, fx + 6, fy + 16, w - 14, argb(0x30, 0xE2, 0xE8, 0xF0));

    // Extension pill badge at bottom
    if !ext.is_empty() {
        let badge_h = 15;
        let badge_y = fy + h - badge_h - 4;
        fill_rounded_rect(buf, bw, bh, fx + 4, badge_y, w - 8, badge_h, 3, accent);
        let max_c = ((w - 10) / font::FONT_W).max(1) as usize;
        let show = ext.len().min(max_c).min(4);
        let tw = (show as i32) * font::FONT_W;
        let tx = fx + (w - tw) / 2;
        font::draw_str(buf, bw, bh, tx, badge_y + 1, &ext[..show], rgb(0xFF, 0xFF, 0xFF));
    }
}

pub fn draw_arrow_left(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, col: u32) {
    draw_line_h(buf, bw, bh, cx - 4, cy, 9, col);
    for d in 1..=4 {
        blend_pixel(buf, bw, cx - 4 + d, cy - d, col);
        blend_pixel(buf, bw, cx - 4 + d, cy + d, col);
    }
}

pub fn draw_arrow_right(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, col: u32) {
    draw_line_h(buf, bw, bh, cx - 4, cy, 9, col);
    for d in 1..=4 {
        blend_pixel(buf, bw, cx + 4 - d, cy - d, col);
        blend_pixel(buf, bw, cx + 4 - d, cy + d, col);
    }
}

pub fn draw_arrow_up(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, col: u32) {
    draw_line_v(buf, bw, bh, cx, cy - 4, 9, col);
    for d in 1..=4 {
        blend_pixel(buf, bw, cx - d, cy - 4 + d, col);
        blend_pixel(buf, bw, cx + d, cy - 4 + d, col);
    }
}

pub fn draw_grid_glyph(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, col: u32) {
    fill_rect(buf, bw, bh, cx - 5, cy - 5, 4, 4, col);
    fill_rect(buf, bw, bh, cx + 1, cy - 5, 4, 4, col);
    fill_rect(buf, bw, bh, cx - 5, cy + 1, 4, 4, col);
    fill_rect(buf, bw, bh, cx + 1, cy + 1, 4, 4, col);
}

pub fn draw_list_glyph(buf: &mut [u32], bw: i32, bh: i32, cx: i32, cy: i32, col: u32) {
    fill_rect(buf, bw, bh, cx - 5, cy - 4, 10, 2, col);
    fill_rect(buf, bw, bh, cx - 5, cy - 1, 10, 2, col);
    fill_rect(buf, bw, bh, cx - 5, cy + 2, 10, 2, col);
}
