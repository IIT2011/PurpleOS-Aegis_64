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

/// Render the refined Purple Bar across the top of the display with slick animations
pub fn render_purple_bar(target: &mut Surface, frame_count: u64) {
    if !target.is_valid() || target.width < 120 || target.height < 100 {
        return;
    }

    let margin = 16.min(target.width / 10);
    let bar_x = margin as i32;
    let bar_w = target.width - margin * 2;
    let bar_h = BAR_HEIGHT;

    // 1. Slick Animation: Boot Entrance Slide-Down (Quadratic Deceleration Ease-Out)
    // Starts flush with the screen top (y = 0) and smoothly glides down into resting floating position (y = 10)
    let slide_offset = if frame_count < 20 {
        let rem = 20 - frame_count as i32;
        (rem * rem * 10) / 400
    } else {
        0
    };
    let bar_y = 10 - slide_offset;

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
    let shimmer_center = bar_x - 100 + ((frame_count * 6) % shimmer_period) as i32;
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

    // 4. Left Region: Glowing Purple 'P' Logo Button with Breathing Amethyst Aura
    let btn_x = bar_x + 8;
    let btn_y = bar_y + 4;

    // Breathing aura pulse: alpha oscillates smoothly between 0x60 and 0xD0 (frequency ~0.5Hz)
    let pulse_cycle = (frame_count % 60) as i32;
    let pulse_delta = if pulse_cycle < 30 { pulse_cycle } else { 60 - pulse_cycle }; // 0..30
    let aura_alpha = 0x60 + (pulse_delta * 0x70 / 30) as u32; // 0x60..0xD0
    let aura_border_color = (aura_alpha << 24) | 0x00C084FC; // Neon Violet pulsing border
    let aura_base_color = 0xEE220C3A;

    render_rounded_glass_rect(
        target,
        btn_x,
        btn_y,
        26,
        26,
        7,
        aura_base_color,
        aura_border_color,
    );

    // Vector Stylized 'P' Glyph (Emits subtle violet sheen when pulse peaks)
    let glyph_color = if pulse_delta > 22 { 0xFFF3E8FF } else { 0xFFFFFFFF };

    // Vertical stem (2px wide)
    for py in (btn_y + 5)..(btn_y + 21) {
        if py >= 0 && (py as usize) < target.height {
            target.set_pixel((btn_x + 8) as usize, py as usize, glyph_color);
            target.set_pixel((btn_x + 9) as usize, py as usize, glyph_color);
        }
    }
    // Top loop horizontal bars (2px thick)
    for px in (btn_x + 8)..(btn_x + 16) {
        if btn_y + 5 >= 0 && ((btn_y + 13) as usize) < target.height {
            target.set_pixel(px as usize, (btn_y + 5) as usize, glyph_color);
            target.set_pixel(px as usize, (btn_y + 6) as usize, glyph_color);
            target.set_pixel(px as usize, (btn_y + 12) as usize, glyph_color);
            target.set_pixel(px as usize, (btn_y + 13) as usize, glyph_color);
        }
    }
    // Loop right vertical bar
    for py in (btn_y + 6)..(btn_y + 13) {
        if py >= 0 && (py as usize) < target.height {
            target.set_pixel((btn_x + 16) as usize, py as usize, glyph_color);
            target.set_pixel((btn_x + 17) as usize, py as usize, glyph_color);
        }
    }

    // 5. Center Region: Dynamic Digital Clock & Kernel Heartbeat Capsule
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

    // Center capsule width: 142px, height: 24px
    let pill_w = 142;
    let pill_h = 24;
    let pill_x = (bar_x + (bar_w as i32 - pill_w as i32) / 2).max(bar_x + 40);
    let pill_y = bar_y + 5;

    render_rounded_glass_rect(
        target,
        pill_x,
        pill_y,
        pill_w,
        pill_h,
        12,
        0x90110722, // Frosted dark obsidian-violet glass
        0x40A855F7, // Subtle neon violet ring
    );

    // Heartbeat Activity Dot on left of time (Real ECG double-beat rhythm)
    let hb_cycle = (frame_count % 60) as usize;
    let is_systolic = hb_cycle < 4;
    let is_diastolic = hb_cycle >= 7 && hb_cycle < 11;
    let (dot_color, dot_radius) = if is_systolic {
        (0xFF34D399, 3) // Bright Emerald Flash with halo
    } else if is_diastolic {
        (0xFF38BDF8, 2) // Cyan Secondary Pulse
    } else {
        (0xFF059669, 2) // Resting Emerald
    };

    let dot_cx = pill_x + 16;
    let dot_cy = pill_y + 12;
    if is_systolic && dot_cy >= 3 && (dot_cy as usize + 3) < target.height {
        // Subtle outer pulse glow ring
        target.blend_pixel((dot_cx - 3) as usize, dot_cy as usize, 0x5034D399);
        target.blend_pixel((dot_cx + 3) as usize, dot_cy as usize, 0x5034D399);
        target.blend_pixel(dot_cx as usize, (dot_cy - 3) as usize, 0x5034D399);
        target.blend_pixel(dot_cx as usize, (dot_cy + 3) as usize, 0x5034D399);
    }
    target.fill_rect(
        Rect::new(dot_cx - dot_radius + 1, dot_cy - dot_radius + 1, (dot_radius * 2 - 1) as usize, (dot_radius * 2 - 1) as usize),
        dot_color,
    );

    // Render Clock text with pulsing fading colon
    let clock_text_x = (pill_x + 32) as usize;
    let clock_text_y = (pill_y + 4) as usize;

    let hr_str = core::str::from_utf8(&clock_buf[..2]).unwrap_or("12");
    target.draw_text_bold(clock_text_x, clock_text_y, hr_str, COLOR_TEXT_PRIMARY, 0, true);

    // Colon pulse: smoothly alternates alpha every half-second
    let colon_bright = ((frame_count / 15) % 2) == 0;
    let colon_color = if colon_bright { 0xFFFFFFFF } else { 0x50A855F7 };
    target.draw_text_bold(clock_text_x + 16, clock_text_y, ":", colon_color, 0, true);

    let min_str = core::str::from_utf8(&clock_buf[3..9]).unwrap_or("00 UTC");
    target.draw_text_bold(clock_text_x + 24, clock_text_y, min_str, COLOR_TEXT_PRIMARY, 0, true);

    // 6. Right Region: Modernized Status Tray with Scanning Wi-Fi, Resonant Audio, & Energy Flow
    let tray_w = 98;
    let tray_h = 24;
    let tray_x = bar_x + bar_w as i32 - tray_w as i32 - 8;
    let tray_y = bar_y + 5;

    render_rounded_glass_rect(
        target,
        tray_x,
        tray_y,
        tray_w,
        tray_h,
        12,
        0x80110722,
        0x389333EA,
    );

    // Widget 1: Wi-Fi Radar Sweep (3-tier concentric cyan arcs in outward cascade)
    let wifi_step = ((frame_count / 12) % 4) as usize;
    let wifi_x = tray_x + 12;
    let wifi_y = tray_y + 6;

    // Anchor dot
    target.set_pixel((wifi_x + 5) as usize, (wifi_y + 9) as usize, COLOR_CYAN_ACCENT);
    target.set_pixel((wifi_x + 6) as usize, (wifi_y + 9) as usize, COLOR_CYAN_ACCENT);

    // Arc 1 (inner)
    let arc1_color = if wifi_step >= 1 { COLOR_CYAN_ACCENT } else { 0x4038BDF8 };
    target.draw_line(wifi_x + 3, wifi_y + 6, wifi_x + 8, wifi_y + 6, arc1_color);

    // Arc 2 (middle)
    let arc2_color = if wifi_step >= 2 { COLOR_CYAN_ACCENT } else { 0x3038BDF8 };
    target.draw_line(wifi_x + 2, wifi_y + 4, wifi_x + 9, wifi_y + 4, arc2_color);

    // Arc 3 (outer)
    let arc3_color = if wifi_step >= 3 { COLOR_CYAN_ACCENT } else { 0x2038BDF8 };
    target.draw_line(wifi_x + 1, wifi_y + 2, wifi_x + 10, wifi_y + 2, arc3_color);

    // Widget 2: Audio Speaker with Acoustic Wave Resonance
    let spk_x = tray_x + 36;
    let spk_y = tray_y + 6;
    let spk_color = 0xFFCBD5E1;
    target.draw_line(spk_x + 1, spk_y + 4, spk_x + 4, spk_y + 4, spk_color);
    target.draw_line(spk_x + 1, spk_y + 8, spk_x + 4, spk_y + 8, spk_color);
    target.draw_line(spk_x + 1, spk_y + 4, spk_x + 1, spk_y + 8, spk_color);
    target.draw_line(spk_x + 4, spk_y + 4, spk_x + 8, spk_y + 1, spk_color);
    target.draw_line(spk_x + 4, spk_y + 8, spk_x + 8, spk_y + 11, spk_color);
    target.draw_line(spk_x + 8, spk_y + 1, spk_x + 8, spk_y + 11, spk_color);

    // Acoustic wave vibration animation
    let audio_vibe = ((frame_count / 10) % 2) == 0;
    let wave1_color = if audio_vibe { 0xFFC084FC } else { 0x90A855F7 };
    let wave2_color = if audio_vibe { 0x90A855F7 } else { 0xFFC084FC };
    target.draw_line(spk_x + 10, spk_y + 3, spk_x + 12, spk_y + 6, wave1_color);
    target.draw_line(spk_x + 12, spk_y + 6, spk_x + 10, spk_y + 9, wave1_color);
    target.draw_line(spk_x + 13, spk_y + 1, spk_x + 15, spk_y + 6, wave2_color);
    target.draw_line(spk_x + 15, spk_y + 6, spk_x + 13, spk_y + 11, wave2_color);

    // Widget 3: Battery Pill with Live Energy Sheen Flow
    let bat_x = tray_x + 64;
    let bat_y = tray_y + 7;
    target.fill_rect(Rect::new(bat_x, bat_y, 18, 10), 0xFF475569);
    target.fill_rect(Rect::new(bat_x + 1, bat_y + 1, 16, 8), 0xFF0F172A);
    target.fill_rect(Rect::new(bat_x + 3, bat_y + 3, 12, 4), 0xFF10B981); // Emerald base
    target.fill_rect(Rect::new(bat_x + 18, bat_y + 3, 2, 4), 0xFF64748B); // Terminal pip

    // Energy sheen flow: a 2px highlight sweeps across the 12px charge bar every 60 frames
    let energy_pos = ((frame_count / 3) % 20) as i32;
    if energy_pos >= 0 && energy_pos < 12 {
        let sx = bat_x + 3 + energy_pos;
        for sy in (bat_y + 3)..(bat_y + 7) {
            target.blend_pixel(sx as usize, sy as usize, 0x80FFFFFF);
        }
    }
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
pub fn render_desktop_shell(target: &mut Surface, windows: &[Option<Window>], frame_count: u64) {
    render_purple_bar(target, frame_count);
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
