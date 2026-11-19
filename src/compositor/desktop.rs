//! Native Purple Desktop UI Shell Subsystem for Aegis64.
//!
//! Provides the refined Purple Bar (top floating panel) and Purple Dock
//! (bottom floating shelf with Looking Glass-inspired floor reflection).
//! Focuses exclusively on the desktop shell bars and visual refinement.

use super::blend::{interpolate_color, scale_alpha};
use super::cursor::Cursor;
use super::surface::{Rect, Surface};
use crate::drivers::font::{FONT_HEIGHT, FONT_WIDTH};

pub const COLOR_GLASS_BASE: u32 = 0xB8120724; // Sleek frosted acrylic glass
pub const COLOR_GLASS_BORDER: u32 = 0x48A855F7; // Translucent neon violet border
pub const COLOR_GLASS_HIGHLIGHT: u32 = 0x3CFFFFFF; // Top specular sheen line
pub const COLOR_AMETHYST_ACCENT: u32 = 0xFFA855F7; // Primary vibrant amethyst
pub const COLOR_NEON_VIOLET: u32 = 0xFFC084FC; // Highlight / active indicator
pub const COLOR_CYAN_ACCENT: u32 = 0xFF38BDF8; // Electric cyan
pub const COLOR_TEXT_PRIMARY: u32 = 0xFFF8FAFC;
pub const COLOR_TEXT_MUTED: u32 = 0xFF94A3B8;

pub const DOCK_NUM_ICONS: usize = 5;
pub const DOCK_ICON_SIZE: usize = 38;
pub const DOCK_ICON_SPACING: usize = 14;
pub const DOCK_PADDING_H: usize = 18;
pub const DOCK_HEIGHT: usize = 56;
pub const BAR_HEIGHT: usize = 34;
pub const ANIMATION_HZ: u64 = 60;
pub const DOCK_LIFT: u8 = 8;
const EDGE_MARGIN: usize = 16;
const BAR_REST_Y: i32 = 10;
const BAR_ENTRANCE_TICKS: u64 = ANIMATION_HZ / 3;
const LOGO_SIZE: usize = 26;
const LOGO_INSET: i32 = 8;
const TRAY_WIDTH: usize = 116;
const REFLECTION_HEIGHT: usize = 22;
const DOCK_WIDTH: usize = DOCK_PADDING_H * 2
    + DOCK_NUM_ICONS * DOCK_ICON_SIZE
    + (DOCK_NUM_ICONS - 1) * DOCK_ICON_SPACING;

fn bar_rect(width: usize, tick: u64) -> Rect {
    let margin = EDGE_MARGIN.min(width / 10);
    let remaining = BAR_ENTRANCE_TICKS.saturating_sub(tick);
    let offset = remaining.pow(2) * BAR_REST_Y as u64 / BAR_ENTRANCE_TICKS.pow(2);
    Rect::new(margin as i32, BAR_REST_Y - offset as i32, width - margin * 2, BAR_HEIGHT)
}

fn logo_rect(bar: Rect) -> Rect {
    Rect::new(bar.x + LOGO_INSET, bar.y + (bar.h - LOGO_SIZE) as i32 / 2, LOGO_SIZE, LOGO_SIZE)
}

fn dock_rect(width: usize, height: usize) -> Option<Rect> {
    if width < DOCK_WIDTH + EDGE_MARGIN * 2 || height < BAR_HEIGHT + DOCK_HEIGHT + REFLECTION_HEIGHT + EDGE_MARGIN * 2 {
        return None;
    }
    Some(Rect::new(
        ((width - DOCK_WIDTH) / 2) as i32,
        (height - DOCK_HEIGHT - REFLECTION_HEIGHT) as i32,
        DOCK_WIDTH,
        DOCK_HEIGHT,
    ))
}

fn dock_icon(dock: Rect, index: usize, lift: u8) -> Rect {
    let x = DOCK_PADDING_H + index * (DOCK_ICON_SIZE + DOCK_ICON_SPACING);
    let y = (dock.h - DOCK_ICON_SIZE) / 2;
    Rect::new(dock.x + x as i32, dock.y + y as i32 - lift as i32, DOCK_ICON_SIZE, DOCK_ICON_SIZE)
}

/// Areas that can change when only desktop chrome is visible.
pub fn animated_regions(width: usize, height: usize) -> [Rect; 2] {
    let bar = bar_rect(width, 0);
    let top = Rect::new(bar.x, 0, bar.w, BAR_HEIGHT + BAR_REST_Y as usize);
    let bottom = dock_rect(width, height).map_or(Rect::new(0, 0, 0, 0), |dock| {
        let margin = 2;
        Rect::new(
            dock.x - margin, dock.y - margin,
            dock.w + margin as usize * 2, dock.h + REFLECTION_HEIGHT + margin as usize,
        )
    });
    [top, bottom]
}

pub fn update_dock_lift(
    width: usize, height: usize, cursor: &Cursor, lift: &mut [u8; DOCK_NUM_ICONS], elapsed: u8,
) {
    let Some(dock) = dock_rect(width, height) else { return };
    for (index, offset) in lift.iter_mut().enumerate() {
        let base = dock_icon(dock, index, 0);
        let hover_area = Rect::new(base.x, base.y - DOCK_LIFT as i32, base.w, base.h + DOCK_LIFT as usize);
        *offset = if cursor.visible && hover_area.contains(cursor.x, cursor.y) {
            offset.saturating_add(elapsed).min(DOCK_LIFT)
        } else {
            offset.saturating_sub(elapsed)
        };
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    None,
    PurpleLogoClick,
    DockIconClick(usize),
}

#[inline(always)]
fn isqrt(val: usize) -> usize {
    let mut x = 0;
    while (x + 1) * (x + 1) <= val {
        x += 1;
    }
    x
}

/// Render an anti-aliased rounded rectangle filled with translucent glass and bordered with a subtle highlight.
pub fn render_rounded_glass_rect(
    target: &mut Surface,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    radius: usize,
    fill_color: u32,
    border_color: u32,
) {
    if !target.is_valid() || w == 0 || h == 0 {
        return;
    }

    let r = radius.min(w / 2).min(h / 2);
    let r_sq = r * r;
    let top_fill = interpolate_color(fill_color, 0xB8AD85D2, 1, 3);
    let bottom_fill = interpolate_color(fill_color, 0xDC080413, 1, 3);

    for row in 0..h {
        let py = y + row as i32;
        if py < 0 || py as usize >= target.height {
            continue;
        }

        let edge_distance = row.min(h - 1 - row);
        let dy = r.saturating_sub(edge_distance + 1);
        let cut = r.saturating_sub(isqrt(r_sq - dy * dy));
        let fill = interpolate_color(top_fill, bottom_fill, row, h - 1);
        let rim = interpolate_color(COLOR_GLASS_HIGHLIGHT, border_color, row, h - 1);
        let start = cut.max(x.saturating_neg().max(0) as usize);
        let end = (w - cut).min((target.width as i32 - x).max(0) as usize);
        for col in start..end {
            let px = (x + col as i32) as usize;
            let on_side = col == cut || col + 1 == w - cut;
            let body = if on_side && dy != 0 { scale_alpha(fill, 156) } else { fill };
            target.blend_pixel(px, py as usize, body);
            if on_side || edge_distance == 0 {
                target.blend_pixel(px, py as usize, rim);
            }
        }
    }
}

/// Inverted alpha-decaying reflection of the dock onto the wallpaper floor (Looking Glass heritage)
fn render_dock_reflection(target: &mut Surface, dock_rect: Rect) {
    let dock_bottom = dock_rect.bottom();
    let reflection_h = REFLECTION_HEIGHT.min(target.height.saturating_sub(dock_bottom as usize));

    for row in 0..reflection_h {
        let src_y = dock_bottom - 1 - (row * dock_rect.h / reflection_h) as i32;
        let dst_y = dock_bottom + row as i32;
        let remaining = reflection_h - row;
        let opacity = (105 * remaining * remaining / (reflection_h * reflection_h)) as u8;
        let inset = row * dock_rect.w / (reflection_h * 16);
        let width = dock_rect.w - 2 * inset;
        for col in 0..width {
            let src_x = dock_rect.x as usize + col * dock_rect.w / width;
            let dst_x = dock_rect.x as usize + inset + col;
            let source = target.get_pixel(src_x, src_y as usize);
            target.blend_pixel(dst_x, dst_y as usize, scale_alpha(source, opacity));
        }
    }
}

/// Render the refined Purple Bar across the top of the display with slick animations
fn render_purple_bar(target: &mut Surface, anim_tick: u64) {
    if !target.is_valid() || target.width < LOGO_SIZE + EDGE_MARGIN * 2 || target.height < BAR_HEIGHT + BAR_REST_Y as usize {
        return;
    }

    let bar = bar_rect(target.width, anim_tick);
    let (bar_x, bar_y, bar_w, bar_h) = (bar.x, bar.y, bar.w, bar.h);

    // 2. Frosted glass panel with specular top edge and subtle border
    render_rounded_glass_rect(
        target,
        bar_x,
        bar_y,
        bar_w,
        bar_h,
        12,
        COLOR_GLASS_BASE,
        COLOR_GLASS_BORDER,
    );

    // 3. Slick Animation: Specular Horizon Light Ray (Gleam Shimmer Sweep)
    let shimmer_period = (bar_w as u64 + 200).max(1);
    let shimmer_center = bar_x - 100 + (anim_tick.wrapping_mul(6) % shimmer_period) as i32;
    let shimmer_radius = 45;

    let sheen_x1 = (bar_x + 12).max(shimmer_center - shimmer_radius);
    let sheen_x2 = (bar_x + bar_w as i32 - 12).min(shimmer_center + shimmer_radius);
    if sheen_x1 < sheen_x2 && bar_y >= 0 && (bar_y as usize) < target.height {
        for px in sheen_x1..sheen_x2 {
            let dist = (px - shimmer_center).abs();
            let alpha = (((shimmer_radius - dist) * 0x90) / shimmer_radius) as u8;
            target.blend_pixel(px as usize, bar_y as usize, (alpha as u32) << 24 | 0x00FFFFFF);
        }
    }

    // 4. Left Region: Glowing Stylish Purple 'P' Brand Emblem with Breathing Amethyst Aura
    let logo = logo_rect(bar);
    let (btn_x, btn_y, btn_size) = (logo.x, logo.y, logo.w);

    // Breathing aura pulse: alpha oscillates smoothly between 0x60 and 0xD0 (frequency ~0.5Hz)
    let half_cycle = ANIMATION_HZ;
    let phase = (anim_tick % (half_cycle * 2)).abs_diff(half_cycle);
    let pulse = phase * phase * (3 * half_cycle - 2 * phase) / half_cycle.pow(2);
    let aura_alpha = 0x60 + (pulse * 0x70 / half_cycle) as u32;
    let aura_border_color = (aura_alpha << 24) | 0x00C084FC; // Neon Violet pulsing border
    let aura_base_color = 0xEE220C3A;

    render_rounded_glass_rect(
        target,
        btn_x,
        btn_y,
        btn_size,
        btn_size,
        7,
        aura_base_color,
        aura_border_color,
    );

    // Blit the Anti-Aliased Stylish 'Amethyst Prism P' Logo (18x18 ARGB)
    let logo_x = btn_x + (btn_size as i32 - 18) / 2;
    let logo_y = btn_y + (btn_size as i32 - 18) / 2;
    let logo_alpha = (232 + pulse * 23 / half_cycle) as u32;
    super::icons::render_logo_argb(target, logo_x, logo_y, &super::icons::STYLISH_P_LOGO, logo_alpha);

    // 5. Center Region: Clean Centered Digital Clock Capsule (No green dot)
    let time = crate::drivers::rtc::get_time();
    let dow = day_of_week(time.year, time.month, time.day);
    let mon = month_name(time.month);

    let mut clock_buf = [0u8; 32];
    let mut len = 0;

    for b in dow.bytes() { clock_buf[len] = b; len += 1; }
    clock_buf[len] = b' '; len += 1;
    for b in mon.bytes() { clock_buf[len] = b; len += 1; }
    clock_buf[len] = b' '; len += 1;

    if time.day >= 10 {
        clock_buf[len] = b'0' + (time.day / 10); len += 1;
    }
    clock_buf[len] = b'0' + (time.day % 10); len += 1;
    clock_buf[len] = b' '; len += 1;
    clock_buf[len] = b' '; len += 1;

    let colon_idx = len + 2;
    clock_buf[len] = b'0' + (time.hour / 10) % 10; len += 1;
    clock_buf[len] = b'0' + (time.hour % 10); len += 1;
    clock_buf[len] = b':'; len += 1;
    clock_buf[len] = b'0' + (time.minute / 10) % 10; len += 1;
    clock_buf[len] = b'0' + (time.minute % 10); len += 1;

    let text_w = len * FONT_WIDTH;
    let pill_pad = EDGE_MARGIN;
    let pill_w = text_w + pill_pad * 2;
    let pill_h = FONT_HEIGHT + 8;
    let pill_x = bar_x + (bar_w as i32 - pill_w as i32) / 2;
    let pill_y = bar_y + (bar_h - pill_h) as i32 / 2;
    let tray_x = bar.right() - TRAY_WIDTH as i32 - LOGO_INSET;
    let show_tray = tray_x >= pill_x + pill_w as i32 + LOGO_INSET;

    if pill_x >= logo.right() + LOGO_INSET {
        render_rounded_glass_rect(
            target, pill_x, pill_y, pill_w, pill_h, pill_h / 2, 0x90110722, 0x40A855F7,
        );
        let clock_text_x = pill_x as usize + pill_pad;
        let clock_text_y = pill_y as usize + (pill_h - FONT_HEIGHT) / 2;
        if let Ok(full_text) = core::str::from_utf8(&clock_buf[..len]) {
            let blink = (anim_tick / (ANIMATION_HZ / 2)) % 2 != 0;
            if blink {
                target.draw_text_bold(clock_text_x, clock_text_y, &full_text[..colon_idx], COLOR_TEXT_PRIMARY, 0, true);
                target.draw_text_bold(clock_text_x + (colon_idx + 1) * FONT_WIDTH, clock_text_y, &full_text[colon_idx + 1..], COLOR_TEXT_PRIMARY, 0, true);
            } else {
                target.draw_text_bold(clock_text_x, clock_text_y, full_text, COLOR_TEXT_PRIMARY, 0, true);
            }
        }
    }

    if show_tray {
        // These glyphs are shell affordances. Their colors do not report hardware state.
        render_rounded_glass_rect(target, tray_x, pill_y, TRAY_WIDTH, pill_h, pill_h / 2, 0x80110722, 0x389333EA);
        let icon_y = pill_y + 4;
        use super::icons::{render_icon_mask, ICON_BATTERY_SHELL, ICON_SLIDERS, ICON_VOLUME_FULL, ICON_WIFI_FULL};
        render_icon_mask(target, tray_x + 10, icon_y, &ICON_SLIDERS, COLOR_NEON_VIOLET);
        render_icon_mask(target, tray_x + 36, icon_y, &ICON_WIFI_FULL, COLOR_TEXT_MUTED);
        render_icon_mask(target, tray_x + 60, icon_y, &ICON_VOLUME_FULL, COLOR_TEXT_MUTED);
        render_icon_mask(target, tray_x + 86, pill_y + 6, &ICON_BATTERY_SHELL, COLOR_TEXT_MUTED);
    }
}

/// Render the refined Purple Dock at the bottom of the screen
fn render_purple_dock(target: &mut Surface, lift: &[u8; DOCK_NUM_ICONS]) {
    if !target.is_valid() {
        return;
    }
    let Some(dock) = dock_rect(target.width, target.height) else { return };
    render_rounded_glass_rect(
        target, dock.x, dock.y, dock.w, dock.h, 22, COLOR_GLASS_BASE, COLOR_GLASS_BORDER,
    );
    for (i, offset) in lift.iter().copied().enumerate() {
        let icon = dock_icon(dock, i, offset);
        let (ix, iy) = (icon.x, icon.y);
        if offset != 0 {
            let glow = scale_alpha(COLOR_NEON_VIOLET, offset * 12);
            render_rounded_glass_rect(target, ix - 2, iy - 2, icon.w + 4, icon.h + 4, 12, scale_alpha(glow, 64), glow);
        }

        match i {
            0 => {
                // Card 0: Terminal Icon (Deep violet card with electric cyan prompt & amethyst cursor)
                render_rounded_glass_rect(target, ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE, 10, 0xEE160A2B, 0x808B5CF6);
                // Chevron '>'
                target.draw_line(ix + 9, iy + 13, ix + 15, iy + 19, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 9, iy + 14, ix + 15, iy + 20, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 15, iy + 19, ix + 9, iy + 25, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 15, iy + 20, ix + 9, iy + 26, COLOR_CYAN_ACCENT);
                // Underscore '_'
                target.draw_line(ix + 19, iy + 25, ix + 27, iy + 25, COLOR_NEON_VIOLET);
                target.draw_line(ix + 19, iy + 26, ix + 27, iy + 26, COLOR_NEON_VIOLET);
            }
            1 => {
                // Card 1: Files Icon (Purple folder)
                render_rounded_glass_rect(target, ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE, 10, 0xEE240E44, 0x80A855F7);
                // Folder tab
                target.fill_rect(Rect::new(ix + 8, iy + 11, 9, 3), 0xFFE9D5FF);
                target.fill_rect(Rect::new(ix + 8, iy + 14, 22, 13), 0xFFA855F7);
                target.draw_line(ix + 8, iy + 14, ix + 29, iy + 14, 0xFFD8B4FE);
            }
            2 => {
                // Card 2: System Monitor Icon (Pulse waveform / ECG)
                render_rounded_glass_rect(target, ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE, 10, 0xEE100D2A, 0x806366F1);
                // Waveform
                target.draw_line(ix + 6, iy + 19, ix + 11, iy + 19, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 11, iy + 19, ix + 14, iy + 9, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 14, iy + 9, ix + 18, iy + 28, COLOR_NEON_VIOLET);
                target.draw_line(ix + 18, iy + 28, ix + 22, iy + 13, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 22, iy + 13, ix + 25, iy + 22, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 25, iy + 22, ix + 31, iy + 19, COLOR_CYAN_ACCENT);
            }
            3 => {
                // Card 3: Code Studio Icon (Brackets </>)
                render_rounded_glass_rect(target, ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE, 10, 0xEE1A0C35, 0x80C084FC);
                // '<'
                target.draw_line(ix + 10, iy + 19, ix + 14, iy + 14, COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 10, iy + 19, ix + 14, iy + 24, COLOR_AMETHYST_ACCENT);
                // '/'
                target.draw_line(ix + 17, iy + 25, ix + 20, iy + 13, 0xFFCBD5E1);
                // '>'
                target.draw_line(ix + 27, iy + 19, ix + 23, iy + 14, COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 27, iy + 19, ix + 23, iy + 24, COLOR_AMETHYST_ACCENT);
            }
            4 => {
                // Card 4: Settings Icon (Precision gear)
                render_rounded_glass_rect(target, ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE, 10, 0xEE1C0E38, 0x809333EA);
                let cx = ix + 19;
                let cy = iy + 19;
                target.fill_rect(Rect::new(cx - 6, cy - 6, 13, 13), COLOR_NEON_VIOLET);
                target.draw_line(cx - 8, cy, cx + 8, cy, COLOR_NEON_VIOLET);
                target.draw_line(cx, cy - 8, cx, cy + 8, COLOR_NEON_VIOLET);
                target.fill_rect(Rect::new(cx - 2, cy - 2, 5, 5), 0xFF1C0E38);
            }
            _ => {}
        }
    }

    // 4. Inverted mirror reflection pass below dock (Looking Glass heritage)
    render_dock_reflection(target, dock);
}

/// Render the complete desktop shell overlay in correct scene pass order
pub fn render_desktop_shell(target: &mut Surface, tick: u64, lift: &[u8; DOCK_NUM_ICONS]) {
    render_purple_bar(target, tick);
    render_purple_dock(target, lift);
}

/// Decode pointer input using the geometry shown in the last presented frame.
pub fn hit_test(
    x: i32, y: i32, width: usize, height: usize, tick: u64, lift: &[u8; DOCK_NUM_ICONS],
) -> DesktopAction {
    if width >= LOGO_SIZE + EDGE_MARGIN * 2 && height >= BAR_HEIGHT + BAR_REST_Y as usize
        && logo_rect(bar_rect(width, tick)).contains(x, y)
    {
        return DesktopAction::PurpleLogoClick;
    }
    if let Some(dock) = dock_rect(width, height) {
        for (index, offset) in lift.iter().copied().enumerate() {
            if dock_icon(dock, index, offset).contains(x, y) {
                return DesktopAction::DockIconClick(index);
            }
        }
    }

    DesktopAction::None
}

/// Compute 3-letter day of week for Gregorian date (Sakamoto algorithm)
fn day_of_week(year: u16, month: u8, day: u8) -> &'static str {
    let m = month as i32;
    let mut y = year as i32;
    let d = day as i32;
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    let idx = if m >= 1 && m <= 12 { (m - 1) as usize } else { 0 };
    let dow = ((y + y / 4 - y / 100 + y / 400 + t[idx] + d) % 7 + 7) % 7;
    match dow {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

/// Convert 1-based month integer to 3-letter month abbreviation
fn month_name(month: u8) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "---",
    }
}
