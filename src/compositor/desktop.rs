//! Native Purple Desktop UI Shell Subsystem for Aegis64.
//!
//! Provides the refined Purple Bar (top floating panel) and Purple Dock
//! (bottom floating shelf with Looking Glass-inspired floor reflection).
//! Focuses exclusively on the desktop shell bars and visual refinement.

use super::blend::scale_alpha;
use super::surface::{Rect, Surface};
use super::window::Window;

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
pub const DOCK_ICON_SPACING: usize = 16;
pub const DOCK_PADDING_H: usize = 18;
pub const DOCK_HEIGHT: usize = 56;
pub const BAR_HEIGHT: usize = 34;

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

    for row in 0..h {
        let py = y + row as i32;
        if py < 0 || py as usize >= target.height {
            continue;
        }

        let (dx_left, dx_right) = if row < r {
            let dy = r - 1 - row;
            let dy_sq = dy * dy;
            let span = if r_sq >= dy_sq { isqrt(r_sq - dy_sq) } else { 0 };
            let cut = r.saturating_sub(span);
            (cut, cut)
        } else if row >= h - r {
            let dy = row - (h - r);
            let dy_sq = dy * dy;
            let span = if r_sq >= dy_sq { isqrt(r_sq - dy_sq) } else { 0 };
            let cut = r.saturating_sub(span);
            (cut, cut)
        } else {
            (0, 0)
        };

        let start_x = dx_left;
        let end_x = w.saturating_sub(dx_right);
        if start_x >= end_x {
            continue;
        }

        for col in start_x..end_x {
            let px = x + col as i32;
            if px < 0 || px as usize >= target.width {
                continue;
            }

            let is_border = row == 0 || row == h - 1 || col == start_x || col == end_x - 1
                || (row < r && col == start_x + 1)
                || (row >= h - r && col == start_x + 1);

            let color = if is_border {
                border_color
            } else if row == 1 {
                COLOR_GLASS_HIGHLIGHT
            } else {
                fill_color
            };

            target.blend_pixel(px as usize, py as usize, color);
        }
    }
}

/// Inverted alpha-decaying reflection of the dock onto the wallpaper floor (Looking Glass heritage)
pub fn render_dock_reflection(target: &mut Surface, dock_rect: Rect) {
    let reflection_h = 16.min(target.height.saturating_sub(dock_rect.bottom() as usize));
    let dock_bottom = dock_rect.bottom();

    for row in 0..reflection_h {
        let dst_y = dock_bottom + row as i32;
        let src_y = dock_bottom - 1 - (row as i32 * 2);
        if src_y < dock_rect.y || dst_y < 0 || dst_y as usize >= target.height {
            continue;
        }

        let alpha_factor = (0x34 as usize).saturating_sub(row * 3) as u8;
        if alpha_factor == 0 {
            break;
        }

        for col in 0..dock_rect.w {
            let px = dock_rect.x + col as i32;
            if px < 0 || px as usize >= target.width {
                continue;
            }

            let src_px = target.get_pixel(px as usize, src_y as usize);
            if (src_px >> 24) != 0 {
                let faded_px = scale_alpha(src_px, alpha_factor);
                target.blend_pixel(px as usize, dst_y as usize, faded_px);
            }
        }
    }
}

/// Render the refined Purple Bar across the top of the display
pub fn render_purple_bar(target: &mut Surface) {
    if !target.is_valid() || target.width < 120 || target.height < 100 {
        return;
    }

    let margin = 16.min(target.width / 10);
    let bar_x = margin as i32;
    let bar_y = 10;
    let bar_w = target.width - margin * 2;
    let bar_h = BAR_HEIGHT;

    // 1. Frosted glass panel with specular top edge and subtle border
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

    // 2. Left Region: Glowing Purple 'P' Logo Button
    let btn_x = bar_x + 8;
    let btn_y = bar_y + 4;
    render_rounded_glass_rect(
        target,
        btn_x,
        btn_y,
        26,
        26,
        7,
        0xDD2E104E,
        0xFF9333EA,
    );

    // Vector Stylized 'P' Glyph
    // Vertical stem (2px wide)
    for py in (btn_y + 5)..(btn_y + 21) {
        target.set_pixel((btn_x + 8) as usize, py as usize, 0xFFFFFFFF);
        target.set_pixel((btn_x + 9) as usize, py as usize, 0xFFFFFFFF);
    }
    // Top loop horizontal bars (2px thick)
    for px in (btn_x + 8)..(btn_x + 16) {
        target.set_pixel(px as usize, (btn_y + 5) as usize, 0xFFFFFFFF);
        target.set_pixel(px as usize, (btn_y + 6) as usize, 0xFFFFFFFF);
        target.set_pixel(px as usize, (btn_y + 12) as usize, 0xFFFFFFFF);
        target.set_pixel(px as usize, (btn_y + 13) as usize, 0xFFFFFFFF);
    }
    // Loop right vertical bar
    for py in (btn_y + 6)..(btn_y + 13) {
        target.set_pixel((btn_x + 16) as usize, py as usize, 0xFFFFFFFF);
        target.set_pixel((btn_x + 17) as usize, py as usize, 0xFFFFFFFF);
    }

    // 3. Center Region: Dynamic digital clock from CMOS RTC
    let time = crate::drivers::rtc::get_time();
    let mut clock_buf = [0u8; 16];
    clock_buf[0] = b'0' + (time.hour / 10) % 10;
    clock_buf[1] = b'0' + time.hour % 10;
    clock_buf[2] = b':';
    clock_buf[3] = b'0' + (time.minute / 10) % 10;
    clock_buf[4] = b'0' + time.minute % 10;
    clock_buf[5] = b' ';
    clock_buf[6] = b'U';
    clock_buf[7] = b'T';
    clock_buf[8] = b'C';
    let clock_str = core::str::from_utf8(&clock_buf[..9]).unwrap_or("18:40 UTC");

    let clock_x = (bar_x + (bar_w as i32 - 9 * 8) / 2).max(bar_x + 40);
    let clock_y = bar_y + (bar_h as i32 - 16) / 2;
    target.draw_text(clock_x as usize, clock_y as usize, clock_str, COLOR_TEXT_PRIMARY, 0, true);

    // 4. Right Region: Minimalist System Tray Icons
    let tray_x = bar_x + bar_w as i32 - 84;
    let tray_y = bar_y + 10;

    // Wi-Fi Icon (3-tier curved signal arcs in cyan)
    target.draw_line(tray_x + 2, tray_y + 3, tray_x + 12, tray_y + 3, COLOR_CYAN_ACCENT);
    target.draw_line(tray_x + 4, tray_y + 7, tray_x + 10, tray_y + 7, COLOR_CYAN_ACCENT);
    target.set_pixel((tray_x + 7) as usize, (tray_y + 11) as usize, COLOR_CYAN_ACCENT);
    target.set_pixel((tray_x + 6) as usize, (tray_y + 11) as usize, COLOR_CYAN_ACCENT);

    // Audio/Volume Icon (Speaker cone + sound arcs)
    let spk_x = tray_x + 24;
    target.draw_line(spk_x + 1, tray_y + 5, spk_x + 4, tray_y + 5, 0xFFCBD5E1);
    target.draw_line(spk_x + 1, tray_y + 9, spk_x + 4, tray_y + 9, 0xFFCBD5E1);
    target.draw_line(spk_x + 1, tray_y + 5, spk_x + 1, tray_y + 9, 0xFFCBD5E1);
    target.draw_line(spk_x + 4, tray_y + 5, spk_x + 8, tray_y + 2, 0xFFCBD5E1);
    target.draw_line(spk_x + 4, tray_y + 9, spk_x + 8, tray_y + 12, 0xFFCBD5E1);
    target.draw_line(spk_x + 8, tray_y + 2, spk_x + 8, tray_y + 12, 0xFFCBD5E1);
    target.draw_line(spk_x + 11, tray_y + 4, spk_x + 13, tray_y + 7, COLOR_AMETHYST_ACCENT);
    target.draw_line(spk_x + 13, tray_y + 7, spk_x + 11, tray_y + 10, COLOR_AMETHYST_ACCENT);

    // Battery/Power Icon (Sleek emerald battery pill)
    let bat_x = tray_x + 50;
    target.fill_rect(Rect::new(bat_x, tray_y + 2, 18, 11), 0xFF475569);
    target.fill_rect(Rect::new(bat_x + 1, tray_y + 3, 16, 9), 0xFF0F172A);
    target.fill_rect(Rect::new(bat_x + 3, tray_y + 5, 12, 5), 0xFF10B981); // Emerald charge
    target.fill_rect(Rect::new(bat_x + 18, tray_y + 5, 2, 5), 0xFF64748B); // Terminal pip
}

/// Render the refined Purple Dock at the bottom of the screen
pub fn render_purple_dock(target: &mut Surface, _windows: &[Option<Window>]) {
    if !target.is_valid() || target.width < 320 || target.height < 140 {
        return;
    }

    let dock_w = 288;
    let dock_h = DOCK_HEIGHT;
    let dock_x = (target.width as i32 - dock_w as i32) / 2;
    let dock_y = target.height as i32 - dock_h as i32 - 16;

    // 1. Frosted glass dock shelf with smooth 22px rounded caps
    render_rounded_glass_rect(
        target,
        dock_x,
        dock_y,
        dock_w,
        dock_h,
        22,
        COLOR_GLASS_BASE,
        COLOR_GLASS_BORDER,
    );

    // 2. Render 5 dock launcher cards with high-fidelity vector glyphs
    for i in 0..DOCK_NUM_ICONS {
        let ix = dock_x + 18 + (i as i32 * 52);
        let iy = dock_y + 9;

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
    render_dock_reflection(target, Rect::new(dock_x, dock_y, dock_w, dock_h));
}

/// Render the complete desktop shell overlay in correct scene pass order
pub fn render_desktop_shell(target: &mut Surface, windows: &[Option<Window>]) {
    render_purple_bar(target);
    render_purple_dock(target, windows);
}

/// Hit test pointer coordinate against the desktop shell UI
pub fn hit_test(x: i32, y: i32, screen_w: usize, screen_h: usize) -> DesktopAction {
    // 1. Check Purple Bar 'P' Button
    let margin = 16.min(screen_w / 10);
    let bar_x = margin as i32;
    let bar_y = 10;
    let btn_rect = Rect::new(bar_x + 8, bar_y + 4, 26, 26);
    if btn_rect.contains(x, y) {
        return DesktopAction::PurpleLogoClick;
    }

    // 2. Check Purple Dock Icons
    let dock_w = 288;
    let dock_x = (screen_w as i32 - dock_w as i32) / 2;
    let dock_y = screen_h as i32 - 56 - 16;
    let dock_rect = Rect::new(dock_x, dock_y, dock_w, 56);

    if dock_rect.contains(x, y) {
        for i in 0..DOCK_NUM_ICONS {
            let ix = dock_x + 18 + (i as i32 * 52);
            let iy = dock_y + 9;
            let icon_rect = Rect::new(ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE);
            if icon_rect.contains(x, y) {
                return DesktopAction::DockIconClick(i);
            }
        }
    }

    DesktopAction::None
}
