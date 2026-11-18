//! Native Purple Desktop UI Shell Subsystem for Aegis64.
//!
//! Provides the top floating Aegis Bar, bottom floating Purple Flow Dock
//! with Looking Glass-inspired mirror reflection, running application indicators,
//! and resolution-independent layout calculation.

use super::blend::scale_alpha;
use super::surface::{Rect, Surface};
use super::window::Window;

pub const COLOR_GLASS_BASE: u32 = 0xCC160B2A; // 80% opacity frosted glass
pub const COLOR_GLASS_BORDER: u32 = 0x44C084FC; // translucent neon violet border
pub const COLOR_GLASS_HIGHLIGHT: u32 = 0x33FFFFFF; // top specular line
pub const COLOR_AMETHYST_ACCENT: u32 = 0xFFA855F7; // primary vibrant amethyst
pub const COLOR_NEON_VIOLET: u32 = 0xFFC084FC; // active indicator dot
pub const COLOR_CYAN_ACCENT: u32 = 0xFF38BDF8; // telemetry / prompt
pub const COLOR_TEXT_PRIMARY: u32 = 0xFFF8FAFC;
pub const COLOR_TEXT_MUTED: u32 = 0xFF94A3B8;

pub const DOCK_NUM_ICONS: usize = 5;
pub const DOCK_ICON_SIZE: usize = 36;
pub const DOCK_ICON_SPACING: usize = 16;
pub const DOCK_PADDING_H: usize = 18;
pub const DOCK_HEIGHT: usize = 54;
pub const BAR_HEIGHT: usize = 32;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    None,
    ToggleLauncher,
    LaunchTerminal,
    LaunchFiles,
    LaunchSystemMonitor,
    LaunchCodeStudio,
    LaunchSettings,
}

#[inline(always)]
fn isqrt(val: usize) -> usize {
    let mut x = 0;
    while (x + 1) * (x + 1) <= val {
        x += 1;
    }
    x
}

/// Render a rounded rectangle filled with translucent glass and bordered with a subtle highlight.
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
    let reflection_h = 14.min(target.height.saturating_sub(dock_rect.bottom() as usize));
    let dock_bottom = dock_rect.bottom();

    for row in 0..reflection_h {
        let dst_y = dock_bottom + row as i32;
        let src_y = dock_bottom - 1 - (row as i32 * 2);
        if src_y < dock_rect.y || dst_y < 0 || dst_y as usize >= target.height {
            continue;
        }

        let alpha_factor = (0x38 as usize).saturating_sub(row * 4) as u8;
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

/// Render the floating Aegis Bar across the top of the screen
pub fn render_aegis_bar(target: &mut Surface) {
    if !target.is_valid() || target.width < 120 || target.height < 100 {
        return;
    }

    let margin = 16.min(target.width / 10);
    let bar_x = margin as i32;
    let bar_y = 10;
    let bar_w = target.width - margin * 2;
    let bar_h = BAR_HEIGHT;

    // 1. Frosted glass panel
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

    // 2. Left Region: Glowing Purple 'P' crest button
    let btn_x = bar_x + 8;
    let btn_y = bar_y + 4;
    render_rounded_glass_rect(
        target,
        btn_x,
        btn_y,
        24,
        24,
        6,
        0xEE3B155B,
        COLOR_AMETHYST_ACCENT,
    );

    // Draw stylized vector 'P' glyph
    for py in (btn_y + 5)..(btn_y + 19) {
        target.set_pixel((btn_x + 8) as usize, py as usize, 0xFFFFFFFF);
        target.set_pixel((btn_x + 9) as usize, py as usize, 0xFFFFFFFF);
    }
    for px in (btn_x + 8)..(btn_x + 15) {
        target.set_pixel(px as usize, (btn_y + 5) as usize, 0xFFFFFFFF);
        target.set_pixel(px as usize, (btn_y + 12) as usize, 0xFFFFFFFF);
    }
    for py in (btn_y + 6)..(btn_y + 12) {
        target.set_pixel((btn_x + 15) as usize, py as usize, 0xFFFFFFFF);
    }

    // 3. Center Region: Dynamic digital clock from RTC
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
    let tray_x = bar_x + bar_w as i32 - 76;
    let tray_y = bar_y + 9;

    // Wi-Fi Icon (3-tier signal waves)
    target.draw_line(tray_x + 2, tray_y + 3, tray_x + 12, tray_y + 3, COLOR_CYAN_ACCENT);
    target.draw_line(tray_x + 4, tray_y + 7, tray_x + 10, tray_y + 7, COLOR_CYAN_ACCENT);
    target.set_pixel((tray_x + 7) as usize, (tray_y + 11) as usize, COLOR_CYAN_ACCENT);

    // Audio/Volume Icon (Speaker cone + sound wave)
    let spk_x = tray_x + 22;
    target.draw_line(spk_x + 1, tray_y + 5, spk_x + 4, tray_y + 5, 0xFFCBD5E1);
    target.draw_line(spk_x + 1, tray_y + 9, spk_x + 4, tray_y + 9, 0xFFCBD5E1);
    target.draw_line(spk_x + 1, tray_y + 5, spk_x + 1, tray_y + 9, 0xFFCBD5E1);
    target.draw_line(spk_x + 4, tray_y + 5, spk_x + 8, tray_y + 2, 0xFFCBD5E1);
    target.draw_line(spk_x + 4, tray_y + 9, spk_x + 8, tray_y + 12, 0xFFCBD5E1);
    target.draw_line(spk_x + 8, tray_y + 2, spk_x + 8, tray_y + 12, 0xFFCBD5E1);
    target.draw_line(spk_x + 11, tray_y + 4, spk_x + 13, tray_y + 7, 0xFF94A3B8);
    target.draw_line(spk_x + 13, tray_y + 7, spk_x + 11, tray_y + 10, 0xFF94A3B8);

    // Battery/Power Icon
    let bat_x = tray_x + 46;
    target.fill_rect(Rect::new(bat_x, tray_y + 2, 16, 10), 0xFF334155);
    target.fill_rect(Rect::new(bat_x + 1, tray_y + 3, 14, 8), 0xFF0F172A);
    target.fill_rect(Rect::new(bat_x + 3, tray_y + 5, 10, 4), 0xFF4ADE80); // Green charge
    target.fill_rect(Rect::new(bat_x + 16, tray_y + 5, 2, 4), 0xFF64748B); // Terminal pip
}

/// Render the floating Purple Flow Dock and active app indicators
pub fn render_flow_dock(target: &mut Surface, windows: &[Option<Window>]) {
    if !target.is_valid() || target.width < 320 || target.height < 140 {
        return;
    }

    let dock_w = 280;
    let dock_h = DOCK_HEIGHT;
    let dock_x = (target.width as i32 - dock_w as i32) / 2;
    let dock_y = target.height as i32 - dock_h as i32 - 18;

    // 1. Frosted glass dock shelf
    render_rounded_glass_rect(
        target,
        dock_x,
        dock_y,
        dock_w,
        dock_h,
        20,
        COLOR_GLASS_BASE,
        COLOR_GLASS_BORDER,
    );

    // 2. Active window presence detection
    let mut term_active = false;
    let mut files_active = false;
    let mut sysmon_active = false;
    let mut code_active = false;
    let mut about_active = false;

    for win_opt in windows.iter() {
        if let Some(win) = win_opt {
            let title = win.title_str();
            if title.contains("Terminal") {
                term_active = true;
            }
            if title.contains("Files") {
                files_active = true;
            }
            if title.contains("Monitor") || title.contains("System") {
                sysmon_active = true;
            }
            if title.contains("Code") || title.contains("Studio") {
                code_active = true;
            }
            if title.contains("About") || title.contains("Purple") {
                about_active = true;
            }
        }
    }

    let is_active = [
        term_active,
        files_active,
        sysmon_active,
        code_active,
        about_active,
    ];

    // 3. Render 5 dock icons with custom vector glyphs
    for i in 0..DOCK_NUM_ICONS {
        let ix = dock_x + 18 + (i as i32 * 52);
        let iy = dock_y + 9;

        // Icon backing card
        render_rounded_glass_rect(
            target,
            ix,
            iy,
            DOCK_ICON_SIZE,
            DOCK_ICON_SIZE,
            10,
            0xEE1E0C38,
            COLOR_AMETHYST_ACCENT,
        );

        match i {
            0 => {
                // Aegis Terminal Icon (Prompt chevron & underscore)
                target.draw_line(ix + 9, iy + 12, ix + 15, iy + 18, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 15, iy + 18, ix + 9, iy + 24, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 18, iy + 24, ix + 26, iy + 24, COLOR_NEON_VIOLET);
            }
            1 => {
                // Purple Files Icon (Folder tab & body)
                target.draw_line(ix + 8, iy + 11, ix + 16, iy + 11, 0xFFE9D5FF);
                target.draw_line(ix + 16, iy + 11, ix + 19, iy + 14, 0xFFE9D5FF);
                target.fill_rect(Rect::new(ix + 8, iy + 14, 20, 12), COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 8, iy + 14, ix + 27, iy + 14, 0xFFF3E8FF);
            }
            2 => {
                // System Monitor Icon (Pulse waveform / EKG)
                target.draw_line(ix + 6, iy + 18, ix + 11, iy + 18, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 11, iy + 18, ix + 14, iy + 10, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 14, iy + 10, ix + 18, iy + 26, COLOR_NEON_VIOLET);
                target.draw_line(ix + 18, iy + 26, ix + 22, iy + 14, COLOR_CYAN_ACCENT);
                target.draw_line(ix + 22, iy + 14, ix + 29, iy + 18, COLOR_CYAN_ACCENT);
            }
            3 => {
                // Code Studio Icon (Code brackets </>)
                target.draw_line(ix + 10, iy + 18, ix + 14, iy + 13, COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 10, iy + 18, ix + 14, iy + 23, COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 16, iy + 24, ix + 20, iy + 12, 0xFFCBD5E1);
                target.draw_line(ix + 26, iy + 18, ix + 22, iy + 13, COLOR_AMETHYST_ACCENT);
                target.draw_line(ix + 26, iy + 18, ix + 22, iy + 23, COLOR_AMETHYST_ACCENT);
            }
            4 => {
                // Settings Icon (Precision gear glyph)
                let cx = ix + 18;
                let cy = iy + 18;
                target.fill_rect(Rect::new(cx - 5, cy - 5, 11, 11), COLOR_NEON_VIOLET);
                target.draw_line(cx - 7, cy, cx + 7, cy, COLOR_NEON_VIOLET);
                target.draw_line(cx, cy - 7, cx, cy + 7, COLOR_NEON_VIOLET);
                target.fill_rect(Rect::new(cx - 2, cy - 2, 5, 5), 0xFF1E0C38);
            }
            _ => {}
        }

        // Active Application Dot Indicator
        if is_active[i] {
            let dot_x = ix + (DOCK_ICON_SIZE as i32 - 6) / 2;
            let dot_y = dock_y + dock_h as i32 - 7;
            target.fill_rect(Rect::new(dot_x, dot_y, 6, 3), COLOR_NEON_VIOLET);
        }
    }

    // 4. Inverted mirror reflection pass below dock
    render_dock_reflection(target, Rect::new(dock_x, dock_y, dock_w, dock_h));
}

/// Render the complete desktop shell overlay in correct scene pass order
pub fn render_desktop_shell(target: &mut Surface, windows: &[Option<Window>]) {
    render_aegis_bar(target);
    render_flow_dock(target, windows);
}

/// Hit test pointer coordinate against the desktop shell UI
pub fn hit_test(x: i32, y: i32, screen_w: usize, screen_h: usize) -> DesktopAction {
    // 1. Check Aegis Bar 'P' Button
    let margin = 16.min(screen_w / 10);
    let bar_x = margin as i32;
    let bar_y = 10;
    let btn_rect = Rect::new(bar_x + 8, bar_y + 4, 24, 24);
    if btn_rect.contains(x, y) {
        return DesktopAction::ToggleLauncher;
    }

    // 2. Check Flow Dock Icons
    let dock_w = 280;
    let dock_x = (screen_w as i32 - dock_w as i32) / 2;
    let dock_y = screen_h as i32 - 54 - 18;
    let dock_rect = Rect::new(dock_x, dock_y, dock_w, 54);

    if dock_rect.contains(x, y) {
        for i in 0..DOCK_NUM_ICONS {
            let ix = dock_x + 18 + (i as i32 * 52);
            let iy = dock_y + 9;
            let icon_rect = Rect::new(ix, iy, DOCK_ICON_SIZE, DOCK_ICON_SIZE);
            if icon_rect.contains(x, y) {
                return match i {
                    0 => DesktopAction::LaunchTerminal,
                    1 => DesktopAction::LaunchFiles,
                    2 => DesktopAction::LaunchSystemMonitor,
                    3 => DesktopAction::LaunchCodeStudio,
                    4 => DesktopAction::LaunchSettings,
                    _ => DesktopAction::None,
                };
            }
        }
    }

    DesktopAction::None
}
