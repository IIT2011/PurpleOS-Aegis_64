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

pub const VIDEO_MAX_COLS: usize = 128;
pub const VIDEO_MAX_ROWS: usize = 64;

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
    pub text_buf: [u8; VIDEO_MAX_COLS * VIDEO_MAX_ROWS],
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
    text_buf: [0; 128 * 64],
});

pub fn has_boot_framebuffer(info: &MultibootInfo) -> bool {
    info.flags & MULTIBOOT_INFO_FRAMEBUFFER != 0
        && info.framebuffer_addr != 0
        && info.framebuffer_bpp == 32
        && info.framebuffer_width > 0
        && info.framebuffer_height > 0
        && info.framebuffer_pitch % 4 == 0
        && info.framebuffer_pitch / 4 >= info.framebuffer_width
}

pub fn init(mbi: Option<&MultibootInfo>) {
    let mut v = VIDEO.lock();

    if let Some(info) = mbi.filter(|info| has_boot_framebuffer(info)) {
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
    v.text_buf.fill(0);
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
    let pitch = v.pitch;
    let height = v.height;
    let scroll_pixels = FONT_HEIGHT * pitch;
    let total_pixels = height * pitch;

    unsafe {
        let fb_slice = core::slice::from_raw_parts_mut(v.fb_ptr, total_pixels);
        if total_pixels > scroll_pixels {
            fb_slice.copy_within(scroll_pixels..total_pixels, 0);
            fb_slice[(total_pixels - scroll_pixels)..total_pixels].fill(v.bg_color);
        }
    }

    let max_rows = (v.height / FONT_HEIGHT).min(VIDEO_MAX_ROWS);
    if max_rows > 1 {
        v.text_buf.copy_within(VIDEO_MAX_COLS..(max_rows * VIDEO_MAX_COLS), 0);
        v.text_buf[(max_rows - 1) * VIDEO_MAX_COLS..max_rows * VIDEO_MAX_COLS].fill(0);
    }
}

fn putc_inner(v: &mut VideoState, c: u8) {
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
                let cr = v.cursor_row;
                let cc = v.cursor_col;
                if cr < VIDEO_MAX_ROWS && cc < VIDEO_MAX_COLS {
                    v.text_buf[cr * VIDEO_MAX_COLS + cc] = 0;
                }
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
                scroll_up(v);
                v.cursor_row = max_rows.saturating_sub(1);
            }

            let cur_row = v.cursor_row;
            let cur_col = v.cursor_col;
            if cur_row < 64 && cur_col < 128 {
                v.text_buf[cur_row * 128 + cur_col] = c;
            }

            let x = cur_col * FONT_WIDTH;
            let y = cur_row * FONT_HEIGHT;
            let fb_slice = unsafe { core::slice::from_raw_parts_mut(v.fb_ptr, v.pitch * v.height) };
            draw_char(fb_slice, v.pitch, v.height, x, y, c, v.fg_color, v.bg_color);
            v.cursor_col += 1;
        }
    }

    if v.cursor_row >= max_rows {
        scroll_up(v);
        v.cursor_row = max_rows.saturating_sub(1);
    }
}

pub fn putc(c: u8) {
    let mut v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    putc_inner(&mut v, c);
}

pub fn puts(s: &str) {
    let mut v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    for b in s.bytes() {
        putc_inner(&mut v, b);
    }
}

pub fn set_colors(fg: u32, bg: u32) {
    let mut v = VIDEO.lock();
    v.fg_color = fg;
    v.bg_color = bg;
}

pub fn get_colors() -> (u32, u32) {
    let v = VIDEO.lock();
    (v.fg_color, v.bg_color)
}

pub fn redraw_top_bar(title: &str) {
    let (width, bg) = {
        let v = VIDEO.lock();
        if !v.available || v.fb_ptr.is_null() {
            return;
        }
        (v.width, v.bg_color)
    };
    draw_rect(0, 0, width, 32, COLOR_PURPLE_BAR);
    set_cursor(2, 0);
    set_colors(COLOR_WHITE, COLOR_PURPLE_BAR);
    puts(title);
    set_cursor(0, 3);
    set_colors(COLOR_WHITE, bg);
}

pub fn set_cursor(col: usize, row: usize) {
    let mut v = VIDEO.lock();
    v.cursor_col = col;
    v.cursor_row = row;
}

pub fn get_cursor() -> (usize, usize) {
    let v = VIDEO.lock();
    (v.cursor_col, v.cursor_row)
}

pub fn clear_to_eol() {
    let v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    let max_cols = v.width / FONT_WIDTH;
    let y = v.cursor_row * FONT_HEIGHT;
    let pitch = v.pitch;
    let bg = v.bg_color;
    for col in v.cursor_col..max_cols {
        let x = col * FONT_WIDTH;
        unsafe {
            for row in 0..FONT_HEIGHT {
                let offset = (y + row) * pitch + x;
                for px in 0..FONT_WIDTH {
                    *v.fb_ptr.add(offset + px) = bg;
                }
            }
        }
    }
}

pub fn get_width() -> usize {
    VIDEO.lock().width
}

pub fn get_height() -> usize {
    VIDEO.lock().height
}

pub fn get_fb_info() -> Option<(usize, usize, usize, usize, u64)> {
    let v = VIDEO.lock();
    if v.available && !v.fb_ptr.is_null() {
        Some((v.width, v.height, v.pitch, v.bpp, v.fb_ptr as u64))
    } else {
        None
    }
}

pub fn blit_rect(src: &[u32], dst_x: usize, dst_y: usize, w: usize, h: usize, src_pitch: usize) -> usize {
    let v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() || w == 0 || h == 0 || src_pitch == 0 {
        return 0;
    }
    if dst_x >= v.width || dst_y >= v.height {
        return 0;
    }
    let copy_w = w.min(v.width - dst_x);
    let copy_h = h.min(v.height - dst_y);

    let mut pixels_blitted = 0usize;
    unsafe {
        for row in 0..copy_h {
            let src_offset = match row.checked_mul(src_pitch) {
                Some(off) => off,
                None => break,
            };
            if match src_offset.checked_add(copy_w) {
                Some(end) => end > src.len(),
                None => true,
            } {
                break;
            }
            let dst_offset = (dst_y + row) * v.pitch + dst_x;
            core::ptr::copy_nonoverlapping(
                src.as_ptr().add(src_offset),
                v.fb_ptr.add(dst_offset),
                copy_w,
            );
            pixels_blitted = pixels_blitted.saturating_add(copy_w);
        }
    }
    pixels_blitted
}

pub fn draw_line(mut x0: usize, mut y0: usize, x1: usize, y1: usize, color: u32) {
    let v = VIDEO.lock();
    if !v.available || v.fb_ptr.is_null() {
        return;
    }
    let pitch = v.pitch;
    let width = v.width;
    let height = v.height;

    let dx = if x1 >= x0 { (x1 - x0) as isize } else { -((x0 - x1) as isize) };
    let dy = if y1 >= y0 { -((y1 - y0) as isize) } else { (y0 - y1) as isize };
    let sx: isize = if x0 < x1 { 1 } else { -1 };
    let sy: isize = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    unsafe {
        loop {
            if x0 < width && y0 < height {
                *v.fb_ptr.add(y0 * pitch + x0) = color;
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 = match (x0 as isize).checked_add(sx) {
                    Some(nx) if nx >= 0 => nx as usize,
                    _ => break,
                };
            }
            if e2 <= dx {
                err += dx;
                y0 = match (y0 as isize).checked_add(sy) {
                    Some(ny) if ny >= 0 => ny as usize,
                    _ => break,
                };
            }
        }
    }
}
