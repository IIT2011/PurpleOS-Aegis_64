use super::{CompositorEngine, Window};
use crate::kernel::{posix, task, uaccess};

pub const EVENT_POINTER: u32 = 1;
pub const EVENT_KEY: u32 = 2;
pub const EVENT_FOCUS: u32 = 3;
pub const EVENT_CLOSE: u32 = 4;
pub const EVENT_RESIZE: u32 = 5;
pub const EVENT_OVERFLOW: u32 = 6;
const EVENTS: usize = 32;
const WINDOWS_PER_TASK: usize = 4;

pub struct ClientEvents {
    buffer: [[u32; 4]; EVENTS],
    start: usize,
    len: usize,
    close: bool,
    dropped: bool,
}

impl ClientEvents {
    pub const fn new() -> Self {
        Self {
            buffer: [[0; 4]; EVENTS],
            start: 0,
            len: 0,
            close: false,
            dropped: false,
        }
    }

    pub fn push(&mut self, event: [u32; 4]) {
        if event[0] == EVENT_CLOSE {
            self.close = true;
            return;
        }
        if self.len > 0 && event[0] == EVENT_POINTER {
            let previous = (self.start + self.len - 1) % EVENTS;
            if self.buffer[previous][0] == EVENT_POINTER && self.buffer[previous][3] == event[3] {
                self.buffer[previous] = event;
                return;
            }
        }
        if self.len == EVENTS {
            self.start = (self.start + 1) % EVENTS;
            self.len -= 1;
            self.dropped = true;
        }
        self.buffer[(self.start + self.len) % EVENTS] = event;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<[u32; 4]> {
        if core::mem::take(&mut self.close) {
            return Some([EVENT_CLOSE, 0, 0, 0]);
        }
        if core::mem::take(&mut self.dropped) {
            return Some([EVENT_OVERFLOW, 0, 0, 0]);
        }
        if self.len == 0 {
            return None;
        }
        let event = self.buffer[self.start];
        self.start = (self.start + 1) % EVENTS;
        self.len -= 1;
        Some(event)
    }
}

impl Window {
    pub(crate) fn client_event(&mut self, event: [u32; 4]) {
        if self.owner.is_some() {
            self.events.push(event);
        }
    }
}

impl CompositorEngine {
    pub(crate) fn create_client_window(
        &mut self,
        owner: Option<u32>,
        title: &str,
        width: usize,
        height: usize,
    ) -> Option<usize> {
        const FRAME: usize = super::window::BORDER_WIDTH * 2;
        let top = super::desktop::BAR_HEIGHT + 16;
        let bottom = super::desktop::DOCK_HEIGHT + 24;
        let available_h = self
            .height
            .saturating_sub(top + bottom + super::window::TITLEBAR_HEIGHT + FRAME);
        if !self.running
            || width < 80
            || height == 0
            || width > self.width.saturating_sub(FRAME)
            || height > available_h
        {
            return None;
        }
        if let Some(id) = owner {
            if self
                .windows
                .iter()
                .flatten()
                .filter(|window| window.owner == Some(id))
                .count()
                >= WINDOWS_PER_TASK
            {
                return None;
            }
        }
        let existing = self
            .windows
            .iter()
            .flatten()
            .filter(|w| w.is_visible && !w.is_minimized)
            .count();
        let offset = (existing % 8) * 24;
        let x = ((self.width - width - FRAME) / 2 + offset).min(self.width - width - FRAME);
        let y = (top + (available_h - height) / 2 + offset)
            .min(self.height - height - super::window::TITLEBAR_HEIGHT - FRAME - 24);
        let id = self.create_window(title, x as i32, y as i32, width, height)?;
        let window = self.get_window_mut(id)?;
        window.owner = owner;
        window.client_event([EVENT_FOCUS, 1, 0, 0]);
        Some(id)
    }

    pub(crate) fn reap_client_windows(&mut self) {
        let mut gone = [0; super::MAX_WINDOWS];
        let mut count = 0;
        for window in self.windows.iter().flatten() {
            if window.owner.is_some_and(|id| !task::native_alive(id)) {
                gone[count] = window.id;
                count += 1;
            }
        }
        for &id in &gone[..count] {
            self.close_window(id);
        }
    }

    pub(crate) fn client_pointer(&mut self, id: usize, x: i32, y: i32, buttons: u8) {
        if let Some(window) = self.get_window_mut(id) {
            let rect = window.client_rect();
            window.client_event([
                EVENT_POINTER,
                (x - rect.x) as u32,
                (y - rect.y) as u32,
                u32::from(buttons),
            ]);
        }
    }
}

pub fn keyboard(byte: u8, modifiers: u8) -> bool {
    let mut compositor = super::COMPOSITOR.lock();
    if compositor.running && byte == b'\t' && modifiers & crate::drivers::keyboard::KBD_MOD_ALT != 0
    {
        if compositor.panel != super::desktop::Panel::Closed {
            compositor.panel = super::desktop::Panel::Closed;
        }
        compositor.cycle_window_focus(modifiers & crate::drivers::keyboard::KBD_MOD_SHIFT != 0);
        return true;
    }
    if !compositor.running || compositor.panel != super::desktop::Panel::Closed {
        return false;
    }
    let Some(id) = compositor.focused_window_id else {
        return false;
    };
    let Some(window) = compositor.get_window_mut(id) else {
        return false;
    };
    if window.owner.is_none() || window.is_minimized {
        return false;
    }
    window.client_event([EVENT_KEY, u32::from(byte), 0, 0]);
    true
}

fn owned_window(
    engine: &mut CompositorEngine,
    owner: u32,
    handle: u64,
) -> Result<&mut Window, i64> {
    let id = usize::try_from(handle).map_err(|_| -posix::EBADF)?;
    engine
        .get_window_mut(id)
        .filter(|window| window.owner == Some(owner))
        .ok_or(-posix::EBADF)
}

pub fn create(owner: u32, name: u64, width: u64, height: u64) -> Result<i64, i64> {
    let mut title = [0; 64];
    let size = uaccess::copy_str_from_user(name, &mut title)?;
    let title = core::str::from_utf8(&title[..size]).map_err(|_| -posix::EINVAL)?;
    if title.is_empty() || title.chars().any(char::is_control) {
        return Err(-posix::EINVAL);
    }
    let width = usize::try_from(width).map_err(|_| -posix::EINVAL)?;
    let height = usize::try_from(height).map_err(|_| -posix::EINVAL)?;
    let id = super::COMPOSITOR
        .lock()
        .create_client_window(Some(owner), title, width, height)
        .ok_or(-posix::ENOSPC)?;
    Ok(id as i64)
}

pub fn close(owner: u32, handle: u64) -> Result<i64, i64> {
    let mut engine = super::COMPOSITOR.lock();
    let id = owned_window(&mut engine, owner, handle)?.id;
    engine.close_window(id);
    Ok(0)
}

pub fn poll(owner: u32, handle: u64, destination: u64) -> Result<i64, i64> {
    let mut engine = super::COMPOSITOR.lock();
    let window = owned_window(&mut engine, owner, handle)?;
    uaccess::validate_user_range(destination, 16, true)?;
    let Some(event) = window.events.pop() else {
        return Ok(0);
    };
    let mut bytes = [0; 16];
    for (word, slot) in event.into_iter().zip(bytes.chunks_exact_mut(4)) {
        slot.copy_from_slice(&word.to_le_bytes());
    }
    uaccess::copy_to_user(&bytes, destination)?;
    Ok(1)
}

pub fn present(owner: u32, handle: u64, source: u64, length: u64) -> Result<i64, i64> {
    let mut engine = super::COMPOSITOR.lock();
    let window = owned_window(&mut engine, owner, handle)?;
    let surface = &mut window.surface;
    // Frames are tightly packed. Opaque windows use XRGB8888; glass windows use ARGB8888.
    let count = surface
        .width
        .checked_mul(surface.height)
        .and_then(|p| p.checked_mul(4))
        .ok_or(-posix::EINVAL)?;
    if length != count as u64 {
        return Err(-posix::EINVAL);
    }
    let destination =
        unsafe { core::slice::from_raw_parts_mut(surface.pixels.cast::<u8>(), count) };
    uaccess::copy_from_user(source, destination)?;
    engine.client_frame_pending = true;
    Ok(0)
}

pub fn size(owner: u32, handle: u64) -> Result<i64, i64> {
    let mut engine = super::COMPOSITOR.lock();
    let window = owned_window(&mut engine, owner, handle)?;
    Ok(((window.surface.width as u64) << 32 | window.surface.height as u64) as i64)
}

/// Enable live acrylic composition for an owned native window. Glass clients
/// submit ARGB8888 pixels; zero alpha reveals the blurred, tinted scene below.
pub fn set_glass(owner: u32, handle: u64, enabled: u64) -> Result<i64, i64> {
    if enabled > 1 {
        return Err(-posix::EINVAL);
    }
    let mut engine = super::COMPOSITOR.lock();
    let window = owned_window(&mut engine, owner, handle)?;
    window.is_glass = enabled != 0;
    if enabled != 0 {
        window.surface.clear(0);
    }
    engine.client_frame_pending = true;
    Ok(0)
}
