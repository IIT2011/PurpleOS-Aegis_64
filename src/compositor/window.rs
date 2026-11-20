//! Window Management, Frame Decoration, and Hit-Testing Engine.
//!
//! Handles window lifecycle, Z-ordering, window drag/move/resize,
//! titlebar gradients, window control buttons, and client surface rendering.

use super::blend::{frost_backdrop, render_drop_shadow, scale_alpha};
use super::surface::{Rect, Surface};

pub const MAX_WINDOWS: usize = 16;
pub const TITLEBAR_HEIGHT: usize = 32;
pub const BORDER_WIDTH: usize = 1;
pub const SHADOW_MARGIN: usize = 12;

// Modern PurpleOS Dark Desktop Theme Palette
pub const COLOR_TITLE_ACTIVE_TOP: u32 = 0xFF5D289D;
pub const COLOR_TITLE_ACTIVE_BOT: u32 = 0xFF351259;
pub const COLOR_TITLE_INACTIVE_TOP: u32 = 0xFF332E3F;
pub const COLOR_TITLE_INACTIVE_BOT: u32 = 0xFF1D1926;

pub const COLOR_BORDER_ACTIVE: u32 = 0xFF9B6BDB;
pub const COLOR_BORDER_INACTIVE: u32 = 0xFF51445E;

pub const COLOR_CLIENT_BG: u32 = 0xFF100B18;
pub const COLOR_TEXT_WHITE: u32 = 0xFFFFFFFF;
pub const COLOR_TEXT_MUTED: u32 = 0xFFB3A7C7;

pub const COLOR_BTN_CLOSE: u32 = 0xFFFF4757;
pub const COLOR_BTN_MIN: u32 = 0xFFFFA502;
pub const COLOR_BTN_MAX: u32 = 0xFF2ED573;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowHitArea {
    None,
    Titlebar,
    CloseButton,
    MinimizeButton,
    MaximizeButton,
    Client(usize, usize),
    ResizeBorder(ResizeEdge),
}

const RESIZE_GRAB: i32 = 7;
pub const MIN_CLIENT_WIDTH: usize = 160;
pub const MIN_CLIENT_HEIGHT: usize = 96;

const CONTROL_DIAMETER: usize = 14;
const CONTROL_GAP: i32 = 6;
const CONTROL_INSET: i32 = 10;
const CONTROLS: [(WindowHitArea, u32); 3] = [
    (WindowHitArea::CloseButton, COLOR_BTN_CLOSE),
    (WindowHitArea::MaximizeButton, COLOR_BTN_MAX),
    (WindowHitArea::MinimizeButton, COLOR_BTN_MIN),
];

fn draw_control(target: &mut Surface, bounds: Rect, color: u32) {
    const SAMPLES: i32 = 4;
    let radius = CONTROL_DIAMETER as i32 * SAMPLES;
    for y in 0..CONTROL_DIAMETER as i32 {
        for x in 0..CONTROL_DIAMETER as i32 {
            let (px, py) = (bounds.x + x, bounds.y + y);
            if px < 0 || py < 0 {
                continue;
            }
            let mut hits = 0;
            for sub_y in 0..SAMPLES {
                for sub_x in 0..SAMPLES {
                    let dx = 2 * (x * SAMPLES + sub_x) + 1 - radius;
                    let dy = 2 * (y * SAMPLES + sub_y) + 1 - radius;
                    hits += i32::from(dx * dx + dy * dy <= radius * radius);
                }
            }
            if hits > 0 {
                target.blend_pixel(
                    px as usize,
                    py as usize,
                    scale_alpha(color, (hits * 255 / (SAMPLES * SAMPLES)) as u8),
                );
            }
        }
    }
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
    /// Native clients can opt into backdrop blur and per-pixel alpha.
    pub is_glass: bool,
    pub is_visible: bool,
    pub surface: Surface,
    pub(crate) owner: Option<u32>,
    pub(crate) events: super::client::ClientEvents,

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
            is_glass: false,
            is_visible: true,
            surface,
            owner: None,
            events: super::client::ClientEvents::new(),
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

    /// Replace the backing surface while retaining the overlapping client pixels.
    pub fn resize_surface(&mut self, width: usize, height: usize) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        if width == self.client_w && height == self.client_h {
            return true;
        }
        let Some(mut surface) = Surface::new(width, height) else {
            return false;
        };
        surface.clear(if self.is_glass { 0 } else { COLOR_CLIENT_BG });
        surface.blit(
            &self.surface,
            Rect::new(0, 0, width.min(self.client_w), height.min(self.client_h)),
            0,
            0,
            false,
        );
        self.surface.destroy();
        self.surface = surface;
        self.client_w = width;
        self.client_h = height;
        true
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

    fn control_bounds(&self, index: usize) -> Rect {
        let stride = CONTROL_DIAMETER as i32 + CONTROL_GAP;
        Rect::new(
            self.total_rect().right()
                - CONTROL_INSET
                - CONTROL_DIAMETER as i32
                - index as i32 * stride,
            self.y + BORDER_WIDTH as i32 + (TITLEBAR_HEIGHT - CONTROL_DIAMETER) as i32 / 2,
            CONTROL_DIAMETER,
            CONTROL_DIAMETER,
        )
    }

    /// Determine which part of the window was clicked or grabbed for resizing.
    pub fn hit_test(&self, mx: i32, my: i32) -> WindowHitArea {
        if !self.is_visible || self.is_minimized {
            return WindowHitArea::None;
        }
        let total = self.total_rect();
        if !total.contains(mx, my) {
            return WindowHitArea::None;
        }

        let left = mx - total.x < RESIZE_GRAB;
        let right = total.right() - mx <= RESIZE_GRAB;
        let top = my - total.y < RESIZE_GRAB;
        let bottom = total.bottom() - my <= RESIZE_GRAB;
        if top && left {
            return WindowHitArea::ResizeBorder(ResizeEdge::NorthWest);
        }
        if top && right {
            return WindowHitArea::ResizeBorder(ResizeEdge::NorthEast);
        }
        if bottom && left {
            return WindowHitArea::ResizeBorder(ResizeEdge::SouthWest);
        }
        if bottom && right {
            return WindowHitArea::ResizeBorder(ResizeEdge::SouthEast);
        }
        if left {
            return WindowHitArea::ResizeBorder(ResizeEdge::West);
        }
        if right {
            return WindowHitArea::ResizeBorder(ResizeEdge::East);
        }
        if top {
            return WindowHitArea::ResizeBorder(ResizeEdge::North);
        }
        if bottom {
            return WindowHitArea::ResizeBorder(ResizeEdge::South);
        }

        let title_bottom = self.y + (TITLEBAR_HEIGHT + BORDER_WIDTH) as i32;
        if my < title_bottom {
            for (index, (action, _)) in CONTROLS.iter().enumerate() {
                let bounds = self.control_bounds(index);
                if bounds.contains(mx, my) {
                    let dx = 2 * (mx - bounds.x) + 1 - CONTROL_DIAMETER as i32;
                    let dy = 2 * (my - bounds.y) + 1 - CONTROL_DIAMETER as i32;
                    if dx * dx + dy * dy <= (CONTROL_DIAMETER * CONTROL_DIAMETER) as i32 {
                        return *action;
                    }
                }
            }
            return WindowHitArea::Titlebar;
        }

        let client = self.client_rect();
        if client.contains(mx, my) {
            return WindowHitArea::Client((mx - client.x) as usize, (my - client.y) as usize);
        }
        WindowHitArea::None
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
        for edge in [
            Rect::new(tot.x, tot.y, tot.w, BORDER_WIDTH),
            Rect::new(
                tot.x,
                tot.bottom() - BORDER_WIDTH as i32,
                tot.w,
                BORDER_WIDTH,
            ),
            Rect::new(tot.x, tot.y, BORDER_WIDTH, tot.h),
            Rect::new(
                tot.right() - BORDER_WIDTH as i32,
                tot.y,
                BORDER_WIDTH,
                tot.h,
            ),
        ] {
            dst.fill_rect(edge, border_color);
        }

        // 3. Titlebar Gradient
        let title_rect = Rect::new(
            self.x + BORDER_WIDTH as i32,
            self.y + BORDER_WIDTH as i32,
            self.client_w,
            TITLEBAR_HEIGHT,
        );

        let tint = if self.is_focused {
            0xA83F2164
        } else {
            0xA7272033
        };
        frost_backdrop(dst, title_rect, 0);
        super::desktop::render_rounded_glass_rect(
            dst,
            title_rect.x,
            title_rect.y,
            title_rect.w,
            title_rect.h,
            7,
            tint,
            border_color,
        );

        // 4. Titlebar Text
        let text_color = if self.is_focused {
            COLOR_TEXT_WHITE
        } else {
            COLOR_TEXT_MUTED
        };
        use crate::drivers::font::{self, FontWeight};
        let text_x = (title_rect.x + 10).max(0) as usize;
        let text_y = (title_rect.y
            + (TITLEBAR_HEIGHT.saturating_sub(font::DESKTOP_HEIGHT) / 2) as i32)
            .max(0) as usize;
        let right = (self.control_bounds(CONTROLS.len() - 1).x - CONTROL_GAP).max(0) as usize;
        dst.draw_text_styled(
            text_x,
            text_y,
            self.title_str(),
            text_color,
            0,
            true,
            right,
            FontWeight::Bold,
        );

        for (index, (_, color)) in CONTROLS.iter().enumerate() {
            draw_control(dst, self.control_bounds(index), *color);
        }
    }

    /// Blit window client area surface onto destination framebuffer
    pub fn render_content(&self, dst: &mut Surface) {
        if !self.is_visible || self.is_minimized || !dst.is_valid() || !self.surface.is_valid() {
            return;
        }

        let dst_x = self.x + BORDER_WIDTH as i32;
        let dst_y = self.y + (TITLEBAR_HEIGHT + BORDER_WIDTH) as i32;
        let src_rect = Rect::new(0, 0, self.client_w, self.client_h);
        if self.is_glass {
            let area = Rect::new(dst_x, dst_y, self.client_w, self.client_h);
            frost_backdrop(dst, area, 16);
            let screen = Rect::new(0, 0, dst.width, dst.height);
            if let Some(visible) = area.intersection(&screen) {
                for y in visible.y as usize..visible.bottom() as usize {
                    for x in visible.x as usize..visible.right() as usize {
                        dst.blend_pixel(x, y, 0x76130C1F);
                    }
                }
            }
        }
        dst.blit(&self.surface, src_rect, dst_x, dst_y, self.is_glass);
    }
}
