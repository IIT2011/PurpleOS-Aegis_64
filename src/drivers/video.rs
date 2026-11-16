use crate::arch::x86_64::boot::{MultibootInfo, MULTIBOOT_INFO_FRAMEBUFFER};
use crate::sync::Spinlock;
use super::font::{draw_char, FONT_HEIGHT, FONT_WIDTH};

pub const COLOR_PURPLE_BG: u32 = 0x1A0933;
pub const COLOR_PURPLE_BAR: u32 = 0x2D1152;
pub const COLOR_PURPLE_ACC: u32 = 0x7E3FF2;
pub const COLOR_WHITE: u32 = 0xFFFFFF;
pub const COLOR_GOLD: u32 = 0xFFD700;
pub const COLOR_GRAY: u32 = 0x888888;
pub const COLOR_RED: u32 = 0xFF3333;
pub const COLOR_GREEN: u32 = 0x33FF33;

pub struct VideoState {
    pub fb_ptr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub bpp: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub fg_color: u32,
    pub bg_color: u32,
    pub available: bool,
}

unsafe impl Send for VideoState {}

pub static VIDEO: Spinlock<VideoState> = Spinlock::new(VideoState {
    fb_ptr: core::ptr::null_mut(),
    width: 0,
    height: 0,
    pitch: 0,
    bpp: 0,
    cursor_col: 0,
    cursor_row: 0,
    fg_color: COLOR_WHITE,
    bg_color: COLOR_PURPLE_BG,
    available: false,
});

pub fn init(mbi: Option<&MultibootInfo>) {
    let mut v = VIDEO.lock();

    if let Some(info) = mbi {
        if (info.flags & MULTIBOOT_INFO_FRAMEBUFFER) != 0 && info.framebuffer_addr != 0 {
            v.fb_ptr = info.framebuffer_addr as *mut u32;
            v.width = info.framebuffer_width as usize;
            v.height = info.framebuffer_height as usize;
            v.pitch = (info.framebuffer_pitch / 4) as usize;
            v.bpp = info.framebuffer_bpp as usize;
            v.cursor_col = 0;
            v.cursor_row = 0;
            v.fg_color = COLOR_WHITE;
            v.bg_color = COLOR_PURPLE_BG;
            v.available = true;

            crate::drivers::serial::print("Video: Linear Framebuffer initialized: ");
            crate::drivers::serial::print_u64(v.width as u64);
            crate::drivers::serial::print("x");
            crate::drivers::serial::print_u64(v.height as u64);
            crate::drivers::serial::print("@");
            crate::drivers::serial::print_u64(v.bpp as u64);
            crate::drivers::serial::println("bpp");
            return;
        }
    }

    // Hardware GPU fallback: probe Bochs / Intel / VirtIO GPU directly
    if let Some((fb_ptr, width, height, pitch, bpp)) = crate::drivers::gpu::init_hardware_display() {
        v.fb_ptr = fb_ptr;
        v.width = width;
        v.height = height;
        v.pitch = pitch;
        v.bpp = bpp;
        v.cursor_col = 0;
        v.cursor_row = 0;
        v.fg_color = COLOR_WHITE;
        v.bg_color = COLOR_PURPLE_BG;
        v.available = true;

        crate::drivers::serial::print("Video: Hardware GPU linear framebuffer initialized: ");
        crate::drivers::serial::print_u64(v.width as u64);
        crate::drivers::serial::print("x");
        crate::drivers::serial::print_u64(v.height as u64);
        crate::drivers::serial::print("@");
        crate::drivers::serial::print_u64(v.bpp as u64);
        crate::drivers::serial::println("bpp (Bochs/QEMU DRM VBE Engine)");
        return;
    }

    crate::drivers::serial::println("Video: No linear framebuffer discovered in boot info.");
}

pub fn is_available() -> bool {
    VIDEO.lock().available
}

pub fn clear_screen(color: u32) {
    let mut v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    let total_pixels = v.pitch * v.height;
    unsafe {
        for i in 0..total_pixels {
            *v.fb_ptr.add(i) = color;
        }
    }
    v.cursor_col = 0;
    v.cursor_row = 0;
}

pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    let v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    let max_x = (x + w).min(v.width);
    let max_y = (y + h).min(v.height);

    unsafe {
        for py in y..max_y {
            let row_offset = py * v.pitch;
            for px in x..max_x {
                *v.fb_ptr.add(row_offset + px) = color;
            }
        }
    }
}

fn scroll_up(v: &mut VideoState) {
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    let line_height = FONT_HEIGHT;
    let pitch = v.pitch;
    let height = v.height;

    unsafe {
        // Shift lines up
        let src = v.fb_ptr.add(line_height * pitch);
        let dst = v.fb_ptr;
        let count = (height - line_height) * pitch;
        core::ptr::copy(src, dst, count);

        // Clear bottom row
        let bottom = v.fb_ptr.add((height - line_height) * pitch);
        let bottom_count = line_height * pitch;
        for i in 0..bottom_count {
            *bottom.add(i) = v.bg_color;
        }
    }
}

pub fn putc(c: u8) {
    let mut v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }

    let max_cols = v.width / FONT_WIDTH;
    let max_rows = v.height / FONT_HEIGHT;

    match c {
        b'\n' => {
            v.cursor_col = 0;
            v.cursor_row += 1;
        }
        b'\r' => {
            v.cursor_col = 0;
        }
        b'\x08' => { // Backspace
            if v.cursor_col > 0 {
                v.cursor_col -= 1;
                let x = v.cursor_col * FONT_WIDTH;
                let y = v.cursor_row * FONT_HEIGHT;
                let pitch = v.pitch;
                let bg = v.bg_color;
                unsafe {
                    for row in 0..FONT_HEIGHT {
                        let offset = (y + row) * pitch + x;
                        for col in 0..FONT_WIDTH {
                            *v.fb_ptr.add(offset + col) = bg;
                        }
                    }
                }
            }
        }
        _ => {
            if v.cursor_col >= max_cols {
                v.cursor_col = 0;
                v.cursor_row += 1;
            }
            if v.cursor_row >= max_rows {
                scroll_up(&mut v);
                v.cursor_row = max_rows.saturating_sub(1);
            }

            let x = v.cursor_col * FONT_WIDTH;
            let y = v.cursor_row * FONT_HEIGHT;
            let fb_slice = unsafe { core::slice::from_raw_parts_mut(v.fb_ptr, v.pitch * v.height) };
            draw_char(fb_slice, v.pitch, v.height, x, y, c, v.fg_color, v.bg_color);
            v.cursor_col += 1;
        }
    }

    if v.cursor_row >= max_rows {
        scroll_up(&mut v);
        v.cursor_row = max_rows.saturating_sub(1);
    }
}

pub fn puts(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

pub fn set_colors(fg: u32, bg: u32) {
    let mut v = VIDEO.lock();
    v.fg_color = fg;
    v.bg_color = bg;
}

pub fn set_cursor(col: usize, row: usize) {
    let mut v = VIDEO.lock();
    v.cursor_col = col;
    v.cursor_row = row;
}

pub fn get_width() -> usize {
    VIDEO.lock().width
}

pub fn get_height() -> usize {
    VIDEO.lock().height
}
