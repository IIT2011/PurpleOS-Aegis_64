//! High-Performance Porter-Duff Alpha Blending and Gradient Engine.
//!
//! Implements SourceOver composite equations, smooth vertical linear gradients,
//! soft alpha window drop shadows, and window border highlights for Aegis64.

use super::surface::{Rect, Surface};

/// Fast SourceOver Porter-Duff alpha blending:
/// `out = src * alpha + dst * (1 - alpha)`
#[inline(always)]
pub fn blend_pixels(src: u32, dst: u32) -> u32 {
    let src_a = (src >> 24) & 0xFF;
    if src_a == 255 {
        return src;
    }
    if src_a == 0 {
        return dst;
    }

    let inv_a = 255 - src_a;
    let src_r = (src >> 16) & 0xFF;
    let src_g = (src >> 8) & 0xFF;
    let src_b = src & 0xFF;

    let dst_r = (dst >> 16) & 0xFF;
    let dst_g = (dst >> 8) & 0xFF;
    let dst_b = dst & 0xFF;
    let dst_a = (dst >> 24) & 0xFF;

    let out_r = (src_r * src_a + dst_r * inv_a) / 255;
    let out_g = (src_g * src_a + dst_g * inv_a) / 255;
    let out_b = (src_b * src_a + dst_b * inv_a) / 255;
    let out_a = src_a + (dst_a * inv_a) / 255;

    (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b
}

/// Linear color interpolation between c1 (factor = 0) and c2 (factor = max)
#[inline(always)]
pub fn interpolate_color(c1: u32, c2: u32, factor: usize, max: usize) -> u32 {
    if max == 0 {
        return c1;
    }
    let t = factor.min(max) as u32;
    let inv = (max - factor.min(max)) as u32;
    let max_u32 = max as u32;

    let r1 = (c1 >> 16) & 0xFF;
    let g1 = (c1 >> 8) & 0xFF;
    let b1 = c1 & 0xFF;
    let a1 = (c1 >> 24) & 0xFF;

    let r2 = (c2 >> 16) & 0xFF;
    let g2 = (c2 >> 8) & 0xFF;
    let b2 = c2 & 0xFF;
    let a2 = (c2 >> 24) & 0xFF;

    let r = (r1 * inv + r2 * t) / max_u32;
    let g = (g1 * inv + g2 * t) / max_u32;
    let b = (b1 * inv + b2 * t) / max_u32;
    let a = (a1 * inv + a2 * t) / max_u32;

    (a << 24) | (r << 16) | (g << 8) | b
}

/// Scale existing pixel's alpha channel
#[inline(always)]
pub fn scale_alpha(color: u32, alpha_scale: u8) -> u32 {
    let a = ((color >> 24) & 0xFF) * (alpha_scale as u32) / 255;
    (a << 24) | (color & 0x00FFFFFF)
}

/// Render a smooth vertical color gradient across a rectangular area
pub fn render_vertical_gradient(dst: &mut Surface, rect: Rect, top_color: u32, bottom_color: u32) {
    if !dst.is_valid() || rect.w == 0 || rect.h == 0 {
        return;
    }

    let y1 = rect.y.max(0) as usize;
    let y2 = (rect.bottom().max(0) as usize).min(dst.height);
    let x1 = rect.x.max(0) as usize;
    let x2 = (rect.right().max(0) as usize).min(dst.width);

    if y1 >= y2 || x1 >= x2 {
        return;
    }

    let total_h = rect.h.max(1);
    let width = x2 - x1;

    for y in y1..y2 {
        let factor = (y as i32 - rect.y).max(0) as usize;
        let color = interpolate_color(top_color, bottom_color, factor, total_h);

        unsafe {
            let row_ptr = dst.pixels.add(y * dst.stride + x1);
            for x in 0..width {
                *row_ptr.add(x) = color;
            }
        }
    }
}

/// Render a soft alpha drop shadow around a window
pub fn render_drop_shadow(dst: &mut Surface, window_rect: Rect, shadow_margin: usize) {
    if !dst.is_valid() || shadow_margin == 0 {
        return;
    }

    let m = shadow_margin as i32;
    // Outer bounding box including shadows
    let outer = Rect::new(
        window_rect.x - m,
        window_rect.y - m,
        window_rect.w + (shadow_margin * 2),
        window_rect.h + (shadow_margin * 2),
    );

    let y_start = outer.y.max(0) as usize;
    let y_end = (outer.bottom().max(0) as usize).min(dst.height);
    let x_start = outer.x.max(0) as usize;
    let x_end = (outer.right().max(0) as usize).min(dst.width);

    if y_start >= y_end || x_start >= x_end {
        return;
    }

    // Shadow color is deep black with falloff alpha
    for y in y_start..y_end {
        let py = y as i32;
        for x in x_start..x_end {
            let px = x as i32;

            // If inside the window itself, skip (window will be drawn over it)
            if px >= window_rect.x
                && px < window_rect.right()
                && py >= window_rect.y
                && py < window_rect.bottom()
            {
                continue;
            }

            // Calculate distance to closest window edge
            let dx = if px < window_rect.x {
                window_rect.x - px
            } else if px >= window_rect.right() {
                px - window_rect.right() + 1
            } else {
                0
            };

            let dy = if py < window_rect.y {
                window_rect.y - py
            } else if py >= window_rect.bottom() {
                py - window_rect.bottom() + 1
            } else {
                0
            };

            // Chebyshev / Euclidean hybrid metric
            let dist = (dx.max(dy) as usize).min(shadow_margin);
            let falloff = shadow_margin - dist;
            // Max shadow alpha at edge is ~0x60 (96), fading to 0
            let alpha = ((falloff * 85) / shadow_margin) as u32;

            if alpha > 0 {
                let shadow_color = (alpha << 24) | 0x000000;
                dst.blend_pixel(x, y, shadow_color);
            }
        }
    }
}
pub fn gray_level(color: u32) -> u32 {
    let red = (color >> 16) & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = color & 0xff;
    (red * 77 + green * 150 + blue * 29 + 128) >> 8
}

/// Blur the already composited pixels beneath a glass panel. The scratch grid
/// is bounded independently of screen resolution and captured before drawing.
pub fn frost_backdrop(target: &mut Surface, area: Rect, radius: usize) {
    const GRID_W: usize = 80;
    const GRID_H: usize = 36;
    const MIN_STEP: usize = 6;
    static BACKDROP: crate::sync::Spinlock<[u32; GRID_W * GRID_H]> =
        crate::sync::Spinlock::new([0; GRID_W * GRID_H]);

    let screen = Rect::new(0, 0, target.width, target.height);
    let Some(visible) = area.intersection(&screen) else {
        return;
    };
    if !target.is_valid() || visible.w == 0 || visible.h == 0 {
        return;
    }
    let step = visible
        .w
        .div_ceil(GRID_W - 1)
        .max(visible.h.div_ceil(GRID_H - 1))
        .max(MIN_STEP);
    let cols = (visible.w - 1).div_ceil(step) + 1;
    let rows = (visible.h - 1).div_ceil(step) + 1;
    let left = visible.x as usize;
    let top = visible.y as usize;
    let right = visible.right() as usize;
    let bottom = visible.bottom() as usize;
    let mut backdrop = BACKDROP.lock();
    for row in 0..rows {
        let y = (top + row * step).min(bottom - 1);
        for col in 0..cols {
            let x = (left + col * step).min(right - 1);
            let delta = step / 2;
            let point = |px: usize, py: usize| {
                target.get_pixel(px.min(target.width - 1), py.min(target.height - 1))
            };
            let h = interpolate_color(point(x.saturating_sub(delta), y), point(x + delta, y), 1, 2);
            let v = interpolate_color(point(x, y.saturating_sub(delta)), point(x, y + delta), 1, 2);
            backdrop[row * GRID_W + col] = interpolate_color(h, v, 1, 2);
        }
    }
    let radius = radius.min(area.w / 2).min(area.h / 2);
    for y in top..bottom {
        let row = (y - top) / step;
        let next_row = (row + 1).min(rows - 1);
        let fy = (y - top) % step;
        let rel_y = (y as i32 - area.y) as usize;
        let dy = radius.saturating_sub(rel_y.min(area.h - 1 - rel_y) + 1);
        for x in left..right {
            let rel_x = (x as i32 - area.x) as usize;
            let dx = radius.saturating_sub(rel_x.min(area.w - 1 - rel_x) + 1);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let col = (x - left) / step;
            let next_col = (col + 1).min(cols - 1);
            let fx = (x - left) % step;
            let a = interpolate_color(
                backdrop[row * GRID_W + col],
                backdrop[row * GRID_W + next_col],
                fx,
                step,
            );
            let b = interpolate_color(
                backdrop[next_row * GRID_W + col],
                backdrop[next_row * GRID_W + next_col],
                fx,
                step,
            );
            target.set_pixel(x, y, interpolate_color(a, b, fy, step));
        }
    }
}
