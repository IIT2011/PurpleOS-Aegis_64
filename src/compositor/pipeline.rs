//! Compositing Pipeline and Scene Graph Renderer for Aegis64.
//!
//! Orchestrates the multi-pass desktop rendering pipeline:
//! 1. Wallpaper background & watermark pass
//! 2. System status bar & dock pass
//! 3. Window z-order sorted frame & client pass
//! 4. Flicker-free hardware/software cursor overlay pass
//! 5. Zero-copy hardware VBE page flip / LFB presentation pass

use super::blend::render_vertical_gradient;
use super::cursor::Cursor;
use super::surface::{Rect, Surface};
use super::window::Window;

pub const COLOR_WALLPAPER_TOP: u32 = 0xFF120722;
pub const COLOR_WALLPAPER_BOT: u32 = 0xFF280E46;

pub const COLOR_BAR_BG: u32 = 0xFF1C0A33;
pub const COLOR_BAR_BORDER: u32 = 0xFF4A1F7A;
pub const COLOR_TEXT_WHITE: u32 = 0xFFFFFFFF;
pub const COLOR_TEXT_ACCENT: u32 = 0xFFC084FC;
pub const COLOR_TEXT_DIM: u32 = 0xFF9CA3AF;
pub const COLOR_TAB_ACTIVE: u32 = 0xFF5D289D;
pub const COLOR_TAB_INACTIVE: u32 = 0xFF281342;

pub struct CompositorPipeline;

impl CompositorPipeline {
    /// Render the entire desktop scene onto the target back-buffer surface
    pub fn render_scene(
        target: &mut Surface,
        windows: &[Option<Window>],
        cursor: &mut Cursor,
        fps_counter: u64,
    ) {
        if !target.is_valid() {
            return;
        }

        let w = target.width;
        let h = target.height;

        // Pass 1: Render Wallpaper Gradient
        render_vertical_gradient(
            target,
            Rect::new(0, 0, w, h),
            COLOR_WALLPAPER_TOP,
            COLOR_WALLPAPER_BOT,
        );

        // Subtle geometric grid dots on desktop background
        for gy in (40..h.saturating_sub(40)).step_by(32) {
            for gx in (20..w.saturating_sub(20)).step_by(32) {
                target.set_pixel(gx, gy, 0x334C1D95);
            }
        }

        // Pass 2: Render Top System Menu Bar (Height: 26px)
        Self::render_top_bar(target, fps_counter);

        // Pass 3: Render Bottom Taskbar / Dock (Height: 28px)
        Self::render_bottom_dock(target, windows);

        // Pass 4: Render Windows in Z-Order (ascending: lowest Z first, highest on top)
        Self::render_windows(target, windows);

        // Pass 5: Render Cursor on Top
        cursor.draw(target);
    }

    /// Render top menu bar with OS brand, active status, and clock
    fn render_top_bar(target: &mut Surface, fps_counter: u64) {
        let w = target.width;
        let bar_rect = Rect::new(0, 0, w, 26);
        target.fill_rect(bar_rect, COLOR_BAR_BG);
        target.draw_line(0, 25, w as i32 - 1, 25, COLOR_BAR_BORDER);

        // Left branding
        target.draw_text(10, 5, "PurpleOS", COLOR_TEXT_ACCENT, 0, true);
        target.draw_text(80, 5, "Aegis64 Native Compositor", COLOR_TEXT_WHITE, 0, true);

        // Center status badge
        target.draw_text(340, 5, "Pure Rust x86_64", COLOR_TEXT_DIM, 0, true);

        // Right system statistics
        let rtc_time = crate::drivers::rtc::get_time();
        let mut time_str = [b'0'; 8]; // "HH:MM:SS"
        time_str[0] = b'0' + ((rtc_time.hour / 10) % 10) as u8;
        time_str[1] = b'0' + (rtc_time.hour % 10) as u8;
        time_str[2] = b':';
        time_str[3] = b'0' + ((rtc_time.minute / 10) % 10) as u8;
        time_str[4] = b'0' + (rtc_time.minute % 10) as u8;
        time_str[5] = b':';
        time_str[6] = b'0' + ((rtc_time.second / 10) % 10) as u8;
        time_str[7] = b'0' + (rtc_time.second % 10) as u8;

        if let Ok(s) = core::str::from_utf8(&time_str) {
            let tx = w.saturating_sub(80);
            target.draw_text(tx, 5, s, COLOR_TEXT_WHITE, 0, true);
        }

        // FPS indicator
        let fx = w.saturating_sub(160);
        let mut fps_buf = [0u8; 16];
        let fps_txt = format_u64_str(fps_counter, &mut fps_buf);
        target.draw_text(fx, 5, "Frame:", COLOR_TEXT_DIM, 0, true);
        target.draw_text(fx + 52, 5, fps_txt, COLOR_TEXT_ACCENT, 0, true);
    }

    /// Render bottom taskbar with open window tabs
    fn render_bottom_dock(target: &mut Surface, windows: &[Option<Window>]) {
        let w = target.width;
        let h = target.height;
        if h < 32 {
            return;
        }

        let dock_y = (h - 28) as i32;
        let dock_rect = Rect::new(0, dock_y, w, 28);
        target.fill_rect(dock_rect, COLOR_BAR_BG);
        target.draw_line(0, dock_y, w as i32 - 1, dock_y, COLOR_BAR_BORDER);

        // Application Menu Button on Left
        let menu_rect = Rect::new(6, dock_y + 3, 76, 22);
        target.fill_rect(menu_rect, COLOR_TAB_ACTIVE);
        target.draw_text(16, (dock_y + 6) as usize, "PURPLE", COLOR_TEXT_WHITE, 0, true);

        // Window Tabs in Dock
        let mut tab_x = 90i32;
        for win_opt in windows.iter() {
            if let Some(win) = win_opt {
                if !win.is_visible {
                    continue;
                }
                let tab_w = 120usize;
                let tab_rect = Rect::new(tab_x, dock_y + 3, tab_w, 22);
                let tab_bg = if win.is_focused {
                    COLOR_TAB_ACTIVE
                } else {
                    COLOR_TAB_INACTIVE
                };
                target.fill_rect(tab_rect, tab_bg);
                target.draw_text(
                    (tab_x + 8) as usize,
                    (dock_y + 6) as usize,
                    win.title_str(),
                    COLOR_TEXT_WHITE,
                    0,
                    true,
                );
                tab_x += tab_w as i32 + 6;
            }
        }
    }

    /// Sort and render visible windows in ascending Z-order
    fn render_windows(target: &mut Surface, windows: &[Option<Window>]) {
        // Collect active window indices
        let mut indices: [usize; 16] = [0; 16];
        let mut count = 0;

        for (i, win_opt) in windows.iter().enumerate() {
            if let Some(win) = win_opt {
                if win.is_visible && !win.is_minimized {
                    indices[count] = i;
                    count += 1;
                }
            }
        }

        // Sort indices by window.z_order ascending (insertion sort for small n)
        for i in 1..count {
            let mut j = i;
            while j > 0 {
                let idx_a = indices[j - 1];
                let idx_b = indices[j];
                let z_a = windows[idx_a].as_ref().map(|w| w.z_order).unwrap_or(0);
                let z_b = windows[idx_b].as_ref().map(|w| w.z_order).unwrap_or(0);
                if z_a > z_b {
                    indices.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        // Render each window from lowest to highest
        for &idx in &indices[..count] {
            if let Some(win) = &windows[idx] {
                win.render_decorations(target);
                win.render_content(target);
            }
        }
    }
}

fn format_u64_str<'a>(mut val: u64, buf: &'a mut [u8; 16]) -> &'a str {
    if val == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut i = 0;
    let mut temp = [0u8; 16];
    while val > 0 && i < 16 {
        temp[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    core::str::from_utf8(&buf[..i]).unwrap_or("")
}
