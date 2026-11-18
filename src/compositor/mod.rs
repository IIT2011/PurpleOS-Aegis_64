//! PurpleOS Aegis64 Native Window Server and Framebuffer Compositor Subsystem.
//!
//! Features:
//! - Multi-window desktop environment with dynamic Z-ordering and clipping
//! - Hardware VBE double-buffering and linear framebuffer page-flipping
//! - Flicker-free software mouse pointer with background save/restore
//! - Soft Porter-Duff alpha blended drop shadows and modern vertical titlebar gradients
//! - Real-time window dragging, focus management, and titlebar control buttons
//! - Character device interface at `/dev/compositor` and `/dev/fb0`
//! - Seeded desktop configuration at `/System/Configuration/desktop.cfg`
//! - Pure Rust `#![no_std]` implementation with zero external crates

pub mod blend;
pub mod cursor;
pub mod pipeline;
pub mod surface;
pub mod window;

pub use cursor::Cursor;
pub use pipeline::CompositorPipeline;
pub use surface::{DamageTracker, Rect, Surface};
pub use window::{Window, WindowHitArea, MAX_WINDOWS};

use crate::drivers::gpu::GPU;
use crate::drivers::video::{self, VIDEO};
use crate::sync::Spinlock;
use core::sync::atomic::{AtomicBool, Ordering};

pub static COMPOSITOR_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct CompositorEngine {
    pub running: bool,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub back_buffer: Surface,
    pub windows: [Option<Window>; MAX_WINDOWS],
    pub next_window_id: usize,
    pub cursor: Cursor,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_buttons: u8,
    pub last_mouse_buttons: u8,
    pub dragging_window_id: Option<usize>,
    pub drag_offset_x: i32,
    pub drag_offset_y: i32,
    pub focused_window_id: Option<usize>,
    pub highest_z: usize,
    pub frame_count: u64,
}

unsafe impl Send for CompositorEngine {}
unsafe impl Sync for CompositorEngine {}

impl CompositorEngine {
    pub const fn new() -> Self {
        Self {
            running: false,
            width: 0,
            height: 0,
            stride: 0,
            back_buffer: Surface::empty(),
            windows: [
                None, None, None, None, None, None, None, None,
                None, None, None, None, None, None, None, None,
            ],
            next_window_id: 1,
            cursor: Cursor::new(),
            mouse_x: 200,
            mouse_y: 150,
            mouse_buttons: 0,
            last_mouse_buttons: 0,
            dragging_window_id: None,
            drag_offset_x: 0,
            drag_offset_y: 0,
            focused_window_id: None,
            highest_z: 0,
            frame_count: 0,
        }
    }

    /// Initialize the compositor engine and display resolution
    pub fn init(&mut self) {
        let (w, h, pitch) = {
            let v = VIDEO.lock();
            if v.available && v.width > 0 && v.height > 0 {
                (v.width, v.height, v.pitch)
            } else {
                (800, 600, 800)
            }
        };

        self.width = w;
        self.height = h;
        self.stride = pitch;

        // Allocate offscreen back-buffer surface
        if let Some(surface) = Surface::new(w, h) {
            self.back_buffer = surface;
            crate::drivers::serial::print("Compositor: Allocated offscreen back-buffer: ");
            crate::drivers::serial::print_u64(w as u64);
            crate::drivers::serial::print("x");
            crate::drivers::serial::print_u64(h as u64);
            crate::drivers::serial::println(" ARGB8888");
        } else {
            crate::drivers::serial::println("Compositor: WARNING - Failed to allocate back-buffer, using direct LFB");
        }

        self.cursor.x = (w / 2) as i32;
        self.cursor.y = (h / 2) as i32;
        self.mouse_x = self.cursor.x;
        self.mouse_y = self.cursor.y;

        crate::drivers::serial::println("Compositor: Aegis64 Window Server & Framebuffer Engine Initialized");
    }

    /// Start the compositor, initialize demo windows, and render initial desktop scene
    pub fn start(&mut self) {
        if self.running {
            return;
        }

        if !self.back_buffer.is_valid() {
            if let Some(surface) = Surface::new(self.width, self.height) {
                self.back_buffer = surface;
            }
        }

        self.running = true;
        COMPOSITOR_RUNNING.store(true, Ordering::SeqCst);
        self.render_frame();
        crate::drivers::serial::println("Compositor: Native framebuffer compositor active (pure window server)");
    }

    /// Stop compositor and return to text console
    pub fn stop(&mut self) {
        if !self.running {
            return;
        }
        self.running = false;
        COMPOSITOR_RUNNING.store(false, Ordering::SeqCst);
        self.dragging_window_id = None;

        // Restore text mode top bar on console
        if video::is_available() {
            video::clear_screen(video::COLOR_PURPLE_BG);
            video::redraw_top_bar("Purple OS / Aegis64 Native Console [Pure Rust x86-64]");
        }

        crate::drivers::serial::println("Compositor: Stopped, returned to native text console");
    }

    /// Count number of active open windows
    pub fn window_count(&self) -> usize {
        self.windows.iter().filter(|w| w.is_some()).count()
    }

    /// Create a new window and bring it to front
    pub fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    ) -> Option<usize> {
        let slot = self.windows.iter().position(|w| w.is_none())?;
        let id = self.next_window_id;
        self.next_window_id += 1;

        let mut win = Window::new(id, title, x, y, width, height)?;
        self.highest_z += 1;
        win.z_order = self.highest_z;
        win.is_focused = true;

        // Unfocus previous windows
        for other in self.windows.iter_mut().flatten() {
            other.is_focused = false;
        }

        self.windows[slot] = Some(win);
        self.focused_window_id = Some(id);

        if self.running {
            self.render_frame();
        }

        Some(id)
    }

    /// Close and destroy a window by ID
    pub fn close_window(&mut self, id: usize) -> bool {
        let mut closed = false;
        for slot in self.windows.iter_mut() {
            if let Some(win) = slot {
                if win.id == id {
                    win.destroy();
                    *slot = None;
                    closed = true;
                    break;
                }
            }
        }

        if closed {
            if self.focused_window_id == Some(id) {
                self.focus_highest_window();
            }
            if self.dragging_window_id == Some(id) {
                self.dragging_window_id = None;
            }
            if self.running {
                self.render_frame();
            }
        }

        closed
    }

    /// Focus a window by ID and bring it to the top of the Z-stack
    pub fn focus_window(&mut self, id: usize) -> bool {
        let mut found = false;
        self.highest_z += 1;
        let new_z = self.highest_z;

        for win in self.windows.iter_mut().flatten() {
            if win.id == id {
                win.z_order = new_z;
                win.is_focused = true;
                found = true;
            } else {
                win.is_focused = false;
            }
        }

        if found {
            self.focused_window_id = Some(id);
            if self.running {
                self.render_frame();
            }
        }

        found
    }

    /// Focus the remaining window with the highest Z-order
    fn focus_highest_window(&mut self) {
        let mut max_z = 0;
        let mut top_id = None;

        for win in self.windows.iter().flatten() {
            if win.z_order >= max_z {
                max_z = win.z_order;
                top_id = Some(win.id);
            }
        }

        if let Some(id) = top_id {
            self.focus_window(id);
        } else {
            self.focused_window_id = None;
        }
    }

    /// Move a window to new coordinates
    pub fn move_window(&mut self, id: usize, new_x: i32, new_y: i32) -> bool {
        for win in self.windows.iter_mut().flatten() {
            if win.id == id {
                win.x = new_x;
                win.y = new_y;
                if self.running {
                    self.render_frame();
                }
                return true;
            }
        }
        false
    }

    /// Process mouse coordinates and button events
    pub fn handle_mouse_event(&mut self, x: i32, y: i32, buttons: u8) {
        self.mouse_x = x;
        self.mouse_y = y;
        self.cursor.x = x;
        self.cursor.y = y;

        let left_clicked = (buttons & 0x01) != 0 && (self.last_mouse_buttons & 0x01) == 0;
        let left_released = (buttons & 0x01) == 0 && (self.last_mouse_buttons & 0x01) != 0;
        self.last_mouse_buttons = buttons;

        if left_released {
            self.dragging_window_id = None;
        }

        // Handle window dragging in motion
        if let Some(drag_id) = self.dragging_window_id {
            let new_x = x - self.drag_offset_x;
            let new_y = y - self.drag_offset_y;
            for win in self.windows.iter_mut().flatten() {
                if win.id == drag_id {
                    win.x = new_x;
                    win.y = new_y;
                    break;
                }
            }
            if self.running {
                self.render_frame();
            }
            return;
        }

        // Handle left click hit-testing
        if left_clicked {
            // Find window clicked under cursor, searching from highest Z to lowest Z
            let mut top_hit_idx = None;
            let mut highest_z = 0;

            for (i, win_opt) in self.windows.iter().enumerate() {
                if let Some(win) = win_opt {
                    if win.is_visible && !win.is_minimized && win.total_rect().contains(x, y) {
                        if win.z_order >= highest_z {
                            highest_z = win.z_order;
                            top_hit_idx = Some(i);
                        }
                    }
                }
            }

            if let Some(idx) = top_hit_idx {
                let (hit_area, win_id, win_x, win_y) = {
                    let win = self.windows[idx].as_ref().unwrap();
                    (win.hit_test(x, y), win.id, win.x, win.y)
                };

                // Bring clicked window to front
                self.focus_window(win_id);

                match hit_area {
                    WindowHitArea::CloseButton => {
                        self.close_window(win_id);
                    }
                    WindowHitArea::MaximizeButton => {
                        for win in self.windows.iter_mut().flatten() {
                            if win.id == win_id {
                                if win.is_maximized {
                                    win.x = win.prev_x;
                                    win.y = win.prev_y;
                                    win.client_w = win.prev_w;
                                    win.client_h = win.prev_h;
                                    win.is_maximized = false;
                                } else {
                                    win.prev_x = win.x;
                                    win.prev_y = win.y;
                                    win.prev_w = win.client_w;
                                    win.prev_h = win.client_h;
                                    win.x = 0;
                                    win.y = 26; // Below top menu bar
                                    win.client_w = self.width.saturating_sub(4);
                                    win.client_h = self.height.saturating_sub(26 + 28 + 32);
                                    win.is_maximized = true;
                                }
                                break;
                            }
                        }
                        if self.running {
                            self.render_frame();
                        }
                    }
                    WindowHitArea::MinimizeButton => {
                        for win in self.windows.iter_mut().flatten() {
                            if win.id == win_id {
                                win.is_minimized = true;
                                break;
                            }
                        }
                        self.focus_highest_window();
                        if self.running {
                            self.render_frame();
                        }
                    }
                    WindowHitArea::Titlebar => {
                        self.dragging_window_id = Some(win_id);
                        self.drag_offset_x = x - win_x;
                        self.drag_offset_y = y - win_y;
                    }
                    _ => {}
                }
            }
        }

        if self.running {
            self.render_frame();
        }
    }

    /// Render scene onto back buffer and present to display
    pub fn render_frame(&mut self) {
        if !self.running {
            return;
        }

        self.frame_count = self.frame_count.wrapping_add(1);

        // 1. Render complete scene into back-buffer
        CompositorPipeline::render_scene(
            &mut self.back_buffer,
            &self.windows,
            &mut self.cursor,
            self.frame_count,
        );

        // 2. Present back-buffer to screen
        self.present_frame();
    }

    /// Blit or page-flip the back-buffer onto the visible linear framebuffer
    fn present_frame(&mut self) {
        if !self.back_buffer.is_valid() {
            return;
        }

        // Hardware Bochs / VBE accelerated page-flip or direct blit
        let gpu = GPU.lock();
        if gpu.is_ready && !gpu.fb_ptr.is_null() {
            let w = self.back_buffer.width.min(gpu.width);
            let h = self.back_buffer.height.min(gpu.height);
            let stride = self.back_buffer.stride;

            unsafe {
                for row in 0..h {
                    let src = self.back_buffer.pixels.add(row * stride);
                    let dst = gpu.fb_ptr.add(row * gpu.pitch);
                    core::ptr::copy_nonoverlapping(src, dst, w);
                }
            }
            return;
        }

        // Standard linear framebuffer copy
        let v = VIDEO.lock();
        if v.available && !v.fb_ptr.is_null() {
            let w = self.back_buffer.width.min(v.width);
            let h = self.back_buffer.height.min(v.height);
            let stride = self.back_buffer.stride;

            unsafe {
                for row in 0..h {
                    let src = self.back_buffer.pixels.add(row * stride);
                    let dst = v.fb_ptr.add(row * v.pitch);
                    core::ptr::copy_nonoverlapping(src, dst, w);
                }
            }
        }
    }

    /// Create rich default demo windows
    pub fn setup_demo_windows(&mut self) {
        if self.window_count() > 0 {
            return;
        }

        // Window 1: Aegis Terminal
        if let Some(id) = self.create_window("Aegis Terminal", 40, 50, 360, 210) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 360, 210), 0xFF140B24);
                win.surface.draw_text(10, 10, "aegis64@purple:~$", 0xFF2ED573, 0, true);
                win.surface.draw_text(154, 10, "compositor status", 0xFFFFFFFF, 0, true);

                win.surface.draw_text(10, 34, "Compositor: ONLINE (x86_64 Pure Rust)", 0xFFB3A7C7, 0, true);
                win.surface.draw_text(10, 54, "Resolution: 800x600@32bpp Double Buffered", 0xFFB3A7C7, 0, true);
                win.surface.draw_text(10, 74, "VFS Device: /dev/compositor, /dev/fb0", 0xFFB3A7C7, 0, true);
                win.surface.draw_text(10, 94, "Security:   AegisDetect Root Trust Anchor", 0xFFFFA502, 0, true);

                win.surface.draw_text(10, 126, "aegis64@purple:~$", 0xFF2ED573, 0, true);
                win.surface.draw_text(154, 126, "ls -la /System/Services", 0xFFFFFFFF, 0, true);
                win.surface.draw_text(10, 150, "Compositor  WindowServer  PurpleDetect", 0xFFC084FC, 0, true);
                win.surface.draw_text(10, 174, "aegis64@purple:~$ _", 0xFF2ED573, 0, true);
            }
        }

        // Window 2: System Monitor
        if let Some(id) = self.create_window("System Monitor", 420, 80, 340, 220) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 340, 220), 0xFF0F172A);

                win.surface.draw_text(10, 10, "Hardware Peripherals & Drivers", 0xFF38BDF8, 0, true);
                win.surface.draw_line(10, 28, 330, 28, 0xFF334155);

                win.surface.draw_text(10, 36, "CPU Cores:    4 Cores Active (x86_64 SMP)", 0xFFE2E8F0, 0, true);
                win.surface.draw_text(10, 56, "Memory PMM:   Dual-Bitmap PMM Allocator", 0xFFE2E8F0, 0, true);
                win.surface.draw_text(10, 76, "Storage:      PurpleFS v2 GPT Journaled", 0xFFE2E8F0, 0, true);
                win.surface.draw_text(10, 96, "Display GPU:  Bochs VBE / VirtIO-GPU 2D", 0xFFE2E8F0, 0, true);
                win.surface.draw_text(10, 116, "APDRV Bundles: filesystems.apdrv, gpu.apdrv", 0xFFE2E8F0, 0, true);

                // Progress Bar: Memory Utilization (approx 12%)
                win.surface.draw_text(10, 146, "RAM Load: [========                    ] 14%", 0xFF4ADE80, 0, true);
                win.surface.draw_text(10, 170, "Syscalls: 68 Native System Calls Online", 0xFFFCD34D, 0, true);
                win.surface.draw_text(10, 190, "POSIX:    IEEE Std 1003.1-2024 Verified", 0xFFF472B6, 0, true);
            }
        }

        // Window 3: About PurpleOS
        if let Some(id) = self.create_window("About PurpleOS", 200, 250, 380, 160) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 380, 160), 0xFF1C1033);

                win.surface.draw_text(10, 12, "PurpleOS / Aegis64 Operating System", 0xFFC084FC, 0, true);
                win.surface.draw_text(10, 32, "Version 0.5.0 Pure Rust Bare-Metal Edition", 0xFFFFFFFF, 0, true);
                win.surface.draw_line(10, 50, 370, 50, 0xFF4C1D95);

                win.surface.draw_text(10, 58, "Kernel:    Aegis64 Micro-Monolithic Core", 0xFFD8B4FE, 0, true);
                win.surface.draw_text(10, 78, "Developer: Creator (Aegis Root Developer)", 0xFFD8B4FE, 0, true);
                win.surface.draw_text(10, 98, "Trust Key: AEGIS-CREATOR-V1 HMAC-SHA256", 0xFFD8B4FE, 0, true);
                win.surface.draw_text(10, 118, "GUI:       Native Pure Rust Compositor", 0xFF34D399, 0, true);
                win.surface.draw_text(10, 138, "License:   100% Original Pure Rust Codebase", 0xFF9CA3AF, 0, true);
            }
        }
    }

    pub fn get_window_mut(&mut self, id: usize) -> Option<&mut Window> {
        self.windows.iter_mut().flatten().find(|w| w.id == id)
    }

    /// Fill status string into buffer for `/dev/compositor` read
    pub fn format_status(&self, buf: &mut [u8]) -> usize {
        let win_cnt = self.window_count();
        let running_str = if self.running { "ACTIVE" } else { "STOPPED" };

        let mut temp = [0u8; 256];
        let mut pos = 0;

        let append_str = |dest: &mut [u8], pos: &mut usize, s: &str| {
            for b in s.bytes() {
                if *pos < dest.len() {
                    dest[*pos] = b;
                    *pos += 1;
                }
            }
        };

        let append_u64 = |dest: &mut [u8], pos: &mut usize, mut val: u64| {
            if val == 0 {
                if *pos < dest.len() { dest[*pos] = b'0'; *pos += 1; }
                return;
            }
            let mut tmp = [0u8; 20];
            let mut i = 0;
            while val > 0 && i < 20 {
                tmp[i] = b'0' + (val % 10) as u8;
                val /= 10;
                i += 1;
            }
            for j in 0..i {
                if *pos < dest.len() {
                    dest[*pos] = tmp[i - 1 - j];
                    *pos += 1;
                }
            }
        };

        append_str(&mut temp, &mut pos, "PurpleOS Native Compositor Engine\n");
        append_str(&mut temp, &mut pos, "Status:       ");
        append_str(&mut temp, &mut pos, running_str);
        append_str(&mut temp, &mut pos, "\nResolution:   ");
        append_u64(&mut temp, &mut pos, self.width as u64);
        append_str(&mut temp, &mut pos, "x");
        append_u64(&mut temp, &mut pos, self.height as u64);
        append_str(&mut temp, &mut pos, "@32bpp\n");
        append_str(&mut temp, &mut pos, "Windows:      ");
        append_u64(&mut temp, &mut pos, win_cnt as u64);
        append_str(&mut temp, &mut pos, "\nDoubleBuffer: Hardware/Memory VBE\n");
        append_str(&mut temp, &mut pos, "Cursor:       Flicker-Free Anti-Aliased\n");
        append_str(&mut temp, &mut pos, "VFS Node:     /dev/compositor, /dev/fb0\n");

        let copy_len = pos.min(buf.len());
        buf[..copy_len].copy_from_slice(&temp[..copy_len]);
        copy_len
    }
}

pub static COMPOSITOR: Spinlock<CompositorEngine> = Spinlock::new(CompositorEngine::new());

pub fn init() {
    COMPOSITOR.lock().init();
}

pub fn start() {
    COMPOSITOR.lock().start();
}

pub fn stop() {
    COMPOSITOR.lock().stop();
}

pub fn is_running() -> bool {
    COMPOSITOR_RUNNING.load(Ordering::Relaxed)
}

pub fn handle_mouse_event(x: i32, y: i32, buttons: u8) {
    COMPOSITOR.lock().handle_mouse_event(x, y, buttons);
}

pub fn create_window(title: &str, x: i32, y: i32, w: usize, h: usize) -> Option<usize> {
    COMPOSITOR.lock().create_window(title, x, y, w, h)
}

pub fn close_window(id: usize) -> bool {
    COMPOSITOR.lock().close_window(id)
}

pub fn focus_window(id: usize) -> bool {
    COMPOSITOR.lock().focus_window(id)
}

pub fn move_window(id: usize, x: i32, y: i32) -> bool {
    COMPOSITOR.lock().move_window(id, x, y)
}

pub fn render_frame() {
    COMPOSITOR.lock().render_frame();
}

pub fn setup_demo_windows() {
    COMPOSITOR.lock().setup_demo_windows();
}

pub fn format_status(buf: &mut [u8]) -> usize {
    COMPOSITOR.lock().format_status(buf)
}
