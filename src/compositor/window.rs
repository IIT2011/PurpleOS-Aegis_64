//! Window Management, Frame Decoration, and Hit-Testing Engine.
//!
//! Handles window lifecycle, Z-ordering, window drag/move/resize,
//! titlebar gradients, window control buttons, and client surface rendering.

use super::blend::{render_drop_shadow, render_vertical_gradient};
use super::surface::{Rect, Surface};

pub const MAX_WINDOWS: usize = 16;
pub const TITLEBAR_HEIGHT: usize = 28;
pub const BORDER_WIDTH: usize = 2;
pub const SHADOW_MARGIN: usize = 8;

// Modern PurpleOS Dark Desktop Theme Palette
pub const COLOR_TITLE_ACTIVE_TOP: u32 = 0xFF5D289D;
pub const COLOR_TITLE_ACTIVE_BOT: u32 = 0xFF351259;
pub const COLOR_TITLE_INACTIVE_TOP: u32 = 0xFF332E3F;
pub const COLOR_TITLE_INACTIVE_BOT: u32 = 0xFF1D1926;

pub const COLOR_BORDER_ACTIVE: u32 = 0xFF7E3FF2;
pub const COLOR_BORDER_INACTIVE: u32 = 0xFF453A54;

pub const COLOR_CLIENT_BG: u32 = 0xFF160E21;
pub const COLOR_TEXT_WHITE: u32 = 0xFFFFFFFF;
pub const COLOR_TEXT_MUTED: u32 = 0xFFB3A7C7;

pub const COLOR_BTN_CLOSE: u32 = 0xFFFF4757;
pub const COLOR_BTN_MIN: u32 = 0xFFFFA502;
pub const COLOR_BTN_MAX: u32 = 0xFF2ED573;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowHitArea {
    None,
    Titlebar,
    CloseButton,
    MinimizeButton,
    MaximizeButton,
    Client(usize, usize),
    ResizeBorder,
}

pub struct Window {
    pub id: usize,
    pub title: [u8; 64],
    pub title_len: usize,
    pub x: i32,
    pub y: i32,
    pub client_w: usize,
    pub client_h: usize,
    pub z_order: usize,
    pub is_focused: bool,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub is_visible: bool,
    pub surface: Surface,

    // Saved geometry for maximize toggle
    pub prev_x: i32,
    pub prev_y: i32,
    pub prev_w: usize,
    pub prev_h: usize,
}

impl Window {
    pub fn new(
        id: usize,
        title: &str,
        x: i32,
        y: i32,
        client_w: usize,
        client_h: usize,
    ) -> Option<Self> {
        let mut surface = Surface::new(client_w, client_h)?;
        surface.clear(COLOR_CLIENT_BG);

        let mut win = Self {
            id,
            title: [0; 64],
            title_len: 0,
            x,
            y,
            client_w,
            client_h,
            z_order: 0,
            is_focused: true,
            is_minimized: false,
            is_maximized: false,
            is_visible: true,
            surface,
            prev_x: x,
            prev_y: y,
            prev_w: client_w,
            prev_h: client_h,
        };

        win.set_title(title);
        Some(win)
    }

    pub fn destroy(&mut self) {
        self.surface.destroy();
        self.is_visible = false;
    }

    pub fn set_title(&mut self, title: &str) {
        let bytes = title.as_bytes();
        let len = bytes.len().min(63);
        self.title[..len].copy_from_slice(&bytes[..len]);
        self.title_len = len;
    }

    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("Window")
    }

    #[inline(always)]
    pub fn total_w(&self) -> usize {
        self.client_w + (BORDER_WIDTH * 2)
    }

    #[inline(always)]
    pub fn total_h(&self) -> usize {
        self.client_h + TITLEBAR_HEIGHT + (BORDER_WIDTH * 2)
    }

    pub fn total_rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.total_w(), self.total_h())
    }

    pub fn client_rect(&self) -> Rect {
        Rect::new(
            self.x + BORDER_WIDTH as i32,
            self.y + (TITLEBAR_HEIGHT + BORDER_WIDTH) as i32,
            self.client_w,
            self.client_h,
        )
    }

    /// Full invalidation rectangle including outer drop shadows
    pub fn damage_rect(&self) -> Rect {
        let m = SHADOW_MARGIN as i32;
        Rect::new(
            self.x - m,
            self.y - m,
            self.total_w() + (SHADOW_MARGIN * 2),
            self.total_h() + (SHADOW_MARGIN * 2),
        )
    }

    /// Determine which part of the window was clicked
    pub fn hit_test(&self, mx: i32, my: i32) -> WindowHitArea {
        if !self.is_visible || self.is_minimized {
            return WindowHitArea::None;
        }

        let tot = self.total_rect();
        if !tot.contains(mx, my) {
            return WindowHitArea::None;
        }

        let rel_x = mx - self.x;
        let rel_y = my - self.y;

        // Titlebar region
        if rel_y >= 0 && rel_y < (TITLEBAR_HEIGHT + BORDER_WIDTH) as i32 {
            let tot_w = self.total_w() as i32;

            // Close button (rightmost: tot_w - 24 .. tot_w - 6)
            if rel_x >= (tot_w - 24) && rel_x < (tot_w - 6) && rel_y >= 6 && rel_y < 22 {
                return WindowHitArea::CloseButton;
            }

            // Maximize button (tot_w - 44 .. tot_w - 26)
            if rel_x >= (tot_w - 44) && rel_x < (tot_w - 26) && rel_y >= 6 && rel_y < 22 {
                return WindowHitArea::MaximizeButton;
            }

            // Minimize button (tot_w - 64 .. tot_w - 46)
            if rel_x >= (tot_w - 64) && rel_x < (tot_w - 46) && rel_y >= 6 && rel_y < 22 {
                return WindowHitArea::MinimizeButton;
            }

            return WindowHitArea::Titlebar;
        }

        // Client area region
        let c_rect = self.client_rect();
        if c_rect.contains(mx, my) {
            let cx = (mx - c_rect.x) as usize;
            let cy = (my - c_rect.y) as usize;
            return WindowHitArea::Client(cx, cy);
        }

        WindowHitArea::ResizeBorder
    }

    /// Render window drop shadow, borders, titlebar, and control buttons
    pub fn render_decorations(&self, dst: &mut Surface) {
        if !self.is_visible || self.is_minimized || !dst.is_valid() {
            return;
        }

        let tot = self.total_rect();

        // 1. Soft Alpha Drop Shadow
        render_drop_shadow(dst, tot, SHADOW_MARGIN);

        // 2. Window Frame Border
        let border_color = if self.is_focused {
            COLOR_BORDER_ACTIVE
        } else {
            COLOR_BORDER_INACTIVE
        };
        dst.fill_rect(tot, border_color);

        // 3. Titlebar Gradient
        let title_rect = Rect::new(
            self.x + BORDER_WIDTH as i32,
            self.y + BORDER_WIDTH as i32,
            self.client_w,
            TITLEBAR_HEIGHT,
        );

        let (t_top, t_bot) = if self.is_focused {
            (COLOR_TITLE_ACTIVE_TOP, COLOR_TITLE_ACTIVE_BOT)
        } else {
            (COLOR_TITLE_INACTIVE_TOP, COLOR_TITLE_INACTIVE_BOT)
        };
        render_vertical_gradient(dst, title_rect, t_top, t_bot);

        // 4. Titlebar Text
        let text_color = if self.is_focused {
            COLOR_TEXT_WHITE
        } else {
            COLOR_TEXT_MUTED
        };
        let text_x = (self.x + BORDER_WIDTH as i32 + 10).max(0) as usize;
        let text_y = (self.y + BORDER_WIDTH as i32 + 6).max(0) as usize;
        dst.draw_text(text_x, text_y, self.title_str(), text_color, 0, true);

        // 5. Modern Window Control Buttons (Close, Maximize, Minimize)
        let tot_w = self.total_w() as i32;

        // Close Button [✕] (Red)
        let close_rect = Rect::new(self.x + tot_w - 24, self.y + 6, 16, 16);
        dst.fill_rect(close_rect, COLOR_BTN_CLOSE);
        let cx = (close_rect.x + 4).max(0) as usize;
        let cy = (close_rect.y).max(0) as usize;
        dst.draw_char(cx, cy, b'x', COLOR_TEXT_WHITE, 0, true);

        // Maximize Button [□] (Green)
        let max_rect = Rect::new(self.x + tot_w - 44, self.y + 6, 16, 16);
        dst.fill_rect(max_rect, COLOR_BTN_MAX);
        let mx = (max_rect.x + 4).max(0) as usize;
        let my = (max_rect.y).max(0) as usize;
        dst.draw_char(mx, my, b'+', COLOR_TEXT_WHITE, 0, true);

        // Minimize Button [—] (Yellow)
        let min_rect = Rect::new(self.x + tot_w - 64, self.y + 6, 16, 16);
        dst.fill_rect(min_rect, COLOR_BTN_MIN);
        let lx = (min_rect.x + 4).max(0) as usize;
        let ly = (min_rect.y).max(0) as usize;
        dst.draw_char(lx, ly, b'-', COLOR_TEXT_WHITE, 0, true);
    }

    /// Blit window client area surface onto destination framebuffer
    pub fn render_content(&self, dst: &mut Surface) {
        if !self.is_visible || self.is_minimized || !dst.is_valid() || !self.surface.is_valid() {
            return;
        }

        let dst_x = self.x + BORDER_WIDTH as i32;
        let dst_y = self.y + (TITLEBAR_HEIGHT + BORDER_WIDTH) as i32;
        let src_rect = Rect::new(0, 0, self.client_w, self.client_h);

        dst.blit(&self.surface, src_rect, dst_x, dst_y, false);
    }
}
