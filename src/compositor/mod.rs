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
pub mod desktop;
pub mod icons;
pub mod pipeline;
pub mod surface;
pub mod wallpaper;
pub mod window;

pub use cursor::Cursor;
pub use desktop::DesktopAction;
pub use pipeline::CompositorPipeline;
pub use surface::{DamageTracker, Rect, Surface};
pub use wallpaper::Wallpaper;
pub use window::{Window, WindowHitArea, MAX_WINDOWS};

use crate::drivers::gpu::GPU;
use crate::drivers::video::{self, VIDEO};
use crate::sync::Spinlock;
use core::sync::atomic::{AtomicBool, Ordering};

pub static COMPOSITOR_RUNNING: AtomicBool = AtomicBool::new(false);

// Retain button transitions while coalescing high-rate motion between frames.
const POINTER_QUEUE_CAPACITY: usize = 32;
type PointerEvent = (i32, i32, u8);

static POINTER_EVENTS: Spinlock<([PointerEvent; POINTER_QUEUE_CAPACITY], usize)> =
    Spinlock::new(([(0, 0, 0); POINTER_QUEUE_CAPACITY], 0));


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
    animation_start: u64,
    rendered_tick: u64,
    dock_lift: [u8; desktop::DOCK_NUM_ICONS],
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
            animation_start: 0,
            rendered_tick: 0,
            dock_lift: [0; desktop::DOCK_NUM_ICONS],
        }
    }

    /// Initialize the compositor engine and display resolution
    pub fn init(&mut self) {
        let display = {
            let v = VIDEO.lock();
            if v.available && v.width > 0 && v.height > 0 {
                Some((v.width, v.height, v.pitch))
            } else {
                None
            }
        };

        let Some((w, h, pitch)) = display else {
            self.width = 0;
            self.height = 0;
            self.stride = 0;
            crate::drivers::serial::println(
                "Compositor: No active framebuffer dimensions; initialization deferred",
            );
            return;
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
            crate::drivers::serial::println("Compositor: Failed to allocate offscreen back buffer");
        }

        self.cursor.x = (w / 2) as i32;
        self.cursor.y = (h / 2) as i32;
        self.mouse_x = self.cursor.x;
        self.mouse_y = self.cursor.y;
        crate::drivers::mouse::set_position(self.mouse_x, self.mouse_y);

        crate::drivers::serial::println("Compositor: Aegis64 Window Server & Framebuffer Engine Initialized");
    }

    /// Start the compositor and present the initial desktop scene.
    pub fn start(&mut self) {
        if self.running {
            return;
        }

        if self.width == 0 || self.height == 0 {
            crate::drivers::serial::println(
                "Compositor: Cannot start without an active display mode",
            );
            return;
        }

        if !self.back_buffer.is_valid() {
            if let Some(surface) = Surface::new(self.width, self.height) {
                self.back_buffer = surface;
            }
        }

        if !self.back_buffer.is_valid() {
            crate::drivers::serial::println("Compositor: Cannot start without a back buffer");
            return;
        }

        POINTER_EVENTS.lock().1 = 0;
        self.last_mouse_buttons = 0;
        self.dock_lift.fill(0);
        self.animation_start = crate::drivers::pit::get_ticks();
        self.running = true;
        COMPOSITOR_RUNNING.store(true, Ordering::SeqCst);
        self.render_frame();
        crate::drivers::serial::println("Compositor: Native framebuffer compositor active (Purple Desktop UI online)");
    }

    /// Stop compositor and return to text console
    pub fn stop(&mut self) {
        if !self.running {
            return;
        }
        self.running = false;
        COMPOSITOR_RUNNING.store(false, Ordering::SeqCst);
        POINTER_EVENTS.lock().1 = 0;
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
            return;
        }

        // Handle left click hit-testing
        if left_clicked {
            // 1. First hit-test the desktop shell UI (Purple Bar and Purple Dock)
            let action = desktop::hit_test(x, y, self.width, self.height, self.rendered_tick, &self.dock_lift);
            match action {
                desktop::DesktopAction::PurpleLogoClick => {
                    crate::drivers::serial::println("Compositor: Purple logo clicked");
                    return;
                }
                desktop::DesktopAction::DockIconClick(i) => {
                    crate::drivers::serial::print("Compositor: Dock launcher icon ");
                    crate::drivers::serial::print_u64(i as u64);
                    crate::drivers::serial::println(" clicked");
                    return;
                }
                desktop::DesktopAction::None => {}
            }

            // 2. Find window clicked under cursor, searching from highest Z to lowest Z
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
                    }
                    WindowHitArea::MinimizeButton => {
                        for win in self.windows.iter_mut().flatten() {
                            if win.id == win_id {
                                win.is_minimized = true;
                                break;
                            }
                        }
                        self.focus_highest_window();
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
    }

    /// Render the complete scene after a window or compositor state change.
    pub fn render_frame(&mut self) {
        self.draw(false);
    }

    fn draw(&mut self, desktop_only: bool) {
        if !self.running {
            return;
        }

        let tick = self.animation_tick();
        let elapsed = tick.saturating_sub(self.rendered_tick).min(desktop::DOCK_LIFT as u64) as u8;
        desktop::update_dock_lift(self.width, self.height, &self.cursor, &mut self.dock_lift, elapsed);
        self.rendered_tick = tick;
        self.frame_count = self.frame_count.saturating_add(1);

        if desktop_only && self.window_count() == 0 {
            let [bar, dock] = desktop::animated_regions(self.width, self.height);
            let previous_pointer = self.cursor.saved_bounds();
            self.cursor.restore(&mut self.back_buffer);
            Wallpaper::restore_regions(&mut self.back_buffer, &[bar, dock]);
            desktop::render_desktop_shell(&mut self.back_buffer, tick, &self.dock_lift);
            self.cursor.draw(&mut self.back_buffer);
            self.present_frame(&[bar, dock, previous_pointer, self.cursor.bounds()]);
        } else {
            CompositorPipeline::render_scene(
                &mut self.back_buffer,
                &self.windows,
                &mut self.cursor,
                tick,
                &self.dock_lift,
            );
            self.present_frame(&[Rect::new(0, 0, self.width, self.height)]);
        }
    }

    fn animation_tick(&self) -> u64 {
        let elapsed = crate::drivers::pit::get_ticks().wrapping_sub(self.animation_start);
        crate::drivers::pit::ticks_to_millis(elapsed).saturating_mul(desktop::ANIMATION_HZ) / 1_000
    }

    /// Copy damaged rectangles from the back buffer to the linear framebuffer.
    fn present_frame(&self, regions: &[Rect]) {
        if !self.back_buffer.is_valid() {
            return;
        }

        let copy = |fb: *mut u32, width: usize, height: usize, pitch: usize| {
            let screen = Rect::new(0, 0, self.back_buffer.width.min(width), self.back_buffer.height.min(height));
            for rect in regions {
                if let Some(rect) = rect.intersection(&screen) {
                    let x = rect.x as usize;
                    for row in rect.y as usize..rect.bottom() as usize {
                        unsafe {
                            let src = self.back_buffer.pixels.add(row * self.back_buffer.stride + x);
                            let dst = fb.add(row * pitch + x);
                            core::ptr::copy_nonoverlapping(src, dst, rect.w);
                        }
                    }
                }
            }
        };
        let gpu = GPU.lock();
        if gpu.is_ready && !gpu.fb_ptr.is_null() {
            copy(gpu.fb_ptr, gpu.width, gpu.height, gpu.pitch);
        } else {
            drop(gpu);
            let v = VIDEO.lock();
            if v.available && !v.fb_ptr.is_null() {
                copy(v.fb_ptr, v.width, v.height, v.pitch);
            }
        }
    }

    /// Setup test demo surfaces for window manager diagnostics ('compositor demo')
    pub fn setup_demo_windows(&mut self) {
        if self.window_count() > 0 {
            return;
        }

        if let Some(id) = self.create_window("Aegis Terminal", 40, 50, 360, 210) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 360, 210), 0xFF140B24);
                win.surface.draw_text(10, 10, "Aegis Terminal [Surface]", 0xFF2ED573, 0, true);
            }
        }

        if let Some(id) = self.create_window("System Monitor", 420, 80, 340, 220) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 340, 220), 0xFF0F172A);
                win.surface.draw_text(10, 10, "System Monitor [Surface]", 0xFF38BDF8, 0, true);
            }
        }

        if let Some(id) = self.create_window("About PurpleOS", 200, 250, 380, 160) {
            if let Some(win) = self.get_window_mut(id) {
                win.surface.fill_rect(Rect::new(0, 0, 380, 160), 0xFF1C1033);
                win.surface.draw_text(10, 12, "About PurpleOS [Surface]", 0xFFC084FC, 0, true);
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
        append_str(&mut temp, &mut pos, "\nDoubleBuffer: RAM back buffer and linear framebuffer\n");
        append_str(&mut temp, &mut pos, "Cursor:       Software overlay\n");
        append_str(&mut temp, &mut pos, "Desktop:      Purple Desktop UI (Purple Bar & Purple Dock)\n");
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
    if !is_running() {
        return;
    }
    let mut queue = POINTER_EVENTS.lock();
    let (events, count) = &mut *queue;
    let event = (x, y, buttons);
    if *count > 0 && events[*count - 1].2 == buttons {
        events[*count - 1] = event;
    } else if *count < events.len() {
        events[*count] = event;
        *count += 1;
    } else {
        // Keep the final button state so overflow cannot leave a drag stuck.
        events[*count - 1] = event;
    }
}

/// Service pointer input and draw at most once per animation tick.
pub fn service() {
    if !is_running() {
        return;
    }
    let mut compositor = COMPOSITOR.lock();
    if !compositor.running || compositor.animation_tick() == compositor.rendered_tick {
        return;
    }
    let (events, count) = {
        let mut queue = POINTER_EVENTS.lock();
        let snapshot = *queue;
        queue.1 = 0;
        snapshot
    };
    for &(x, y, buttons) in &events[..count] {
        compositor.handle_mouse_event(x, y, buttons);
    }
    compositor.draw(true);
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
