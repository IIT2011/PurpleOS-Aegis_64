//! Purple OS GUI ABI v1 syscall wrappers and core data types.
#![allow(dead_code)]

use core::arch::asm;

pub const SYS_ABI_VERSION: u64 = 0x1100;
pub const SYS_STARTUP_INFO: u64 = 0x1101;
pub const SYS_CREATE_WINDOW: u64 = 0x1102;
pub const SYS_CLOSE_WINDOW: u64 = 0x1103;
pub const SYS_PRESENT: u64 = 0x1104;
pub const SYS_POLL_EVENT: u64 = 0x1105;
pub const SYS_WINDOW_SIZE: u64 = 0x1106;
pub const SYS_SERIAL_LOG: u64 = 0x1107;
pub const SYS_ALLOC: u64 = 0x1108;
pub const SYS_FREE: u64 = 0x1109;
pub const SYS_SHELL_EXEC: u64 = 0x110A;
pub const SYS_SHELL_PROMPT: u64 = 0x110B;
pub const SYS_SET_GLASS: u64 = 0x110C;
pub const SYS_YIELD: u64 = 24;
pub const SYS_GETPID: u64 = 39;
pub const SYS_EXIT: u64 = 60;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StartupInfo {
    pub abi_version: u64,
    pub window_handle: u64,
    pub width: u64,
    pub height: u64,
    pub resource_addr: u64,
    pub resource_size: u64,
    pub metadata_addr: u64,
    pub metadata_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RawEvent {
    pub kind: u32,
    pub x: i32,
    pub y: i32,
    pub detail: u32,
}

pub const EVENT_POINTER: u32 = 1;
pub const EVENT_KEY: u32 = 2;
pub const EVENT_FOCUS: u32 = 3;
pub const EVENT_CLOSE: u32 = 4;
pub const EVENT_RESIZE: u32 = 5;
pub const EVENT_OVERFLOW: u32 = 6;

pub const BTN_LEFT: u32 = 1 << 0;
pub const BTN_RIGHT: u32 = 1 << 1;
pub const BTN_MIDDLE: u32 = 1 << 2;

#[inline(always)]
unsafe fn syscall0(num: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
unsafe fn syscall1(num: u64, a1: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
unsafe fn syscall2(num: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
unsafe fn syscall3(num: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    ret
}

pub fn abi_version() -> i64 {
    unsafe { syscall0(SYS_ABI_VERSION) }
}

pub fn startup_info() -> Option<&'static StartupInfo> {
    let ptr = unsafe { syscall0(SYS_STARTUP_INFO) };
    if ptr <= 0 {
        None
    } else {
        Some(unsafe { &*(ptr as *const StartupInfo) })
    }
}

pub fn set_glass(handle: u64, enabled: bool) -> i64 {
    unsafe { syscall2(SYS_SET_GLASS, handle, if enabled { 1 } else { 0 }) }
}

pub fn window_size(handle: u64) -> (u32, u32) {
    let res = unsafe { syscall1(SYS_WINDOW_SIZE, handle) };
    if res <= 0 {
        return (0, 0);
    }
    let packed = res as u64;
    let w = (packed >> 32) as u32;
    let h = (packed & 0xFFFF_FFFF) as u32;
    (w, h)
}

pub fn poll_event(handle: u64) -> Option<RawEvent> {
    let mut ev = RawEvent::default();
    let res = unsafe { syscall2(SYS_POLL_EVENT, handle, (&mut ev as *mut RawEvent) as u64) };
    if res == 1 {
        Some(ev)
    } else {
        None
    }
}

pub fn present(handle: u64, pixels: &[u32]) -> i64 {
    let byte_len = (pixels.len() * 4) as u64;
    unsafe { syscall3(SYS_PRESENT, handle, pixels.as_ptr() as u64, byte_len) }
}

pub fn alloc_pages(bytes: usize) -> Option<*mut u8> {
    if bytes == 0 {
        return None;
    }
    let res = unsafe { syscall1(SYS_ALLOC, bytes as u64) };
    if res <= 0 {
        None
    } else {
        Some(res as *mut u8)
    }
}

pub fn free_pages(base: *mut u8) -> i64 {
    if base.is_null() {
        return 0;
    }
    unsafe { syscall1(SYS_FREE, base as u64) }
}

/// Executes an AegisShell command (<= 240 printable ASCII bytes) into a 4096-byte buffer.
/// Returns the number of bytes written on success, or negative errno.
pub fn shell_exec(cmd: &[u8], out_buf: &mut [u8; 4096]) -> i64 {
    let len = cmd.len().min(240);
    unsafe {
        syscall3(
            SYS_SHELL_EXEC,
            cmd.as_ptr() as u64,
            len as u64,
            out_buf.as_mut_ptr() as u64,
        )
    }
}

/// Queries the current AegisShell prompt into a 96-byte buffer.
pub fn shell_prompt(out_buf: &mut [u8; 96]) -> i64 {
    unsafe { syscall2(SYS_SHELL_PROMPT, out_buf.as_mut_ptr() as u64, 96) }
}

pub fn serial_log(msg: &[u8]) {
    let len = msg.len().min(256);
    unsafe {
        let _ = syscall2(SYS_SERIAL_LOG, msg.as_ptr() as u64, len as u64);
    }
}

pub fn yield_cpu() {
    unsafe {
        let _ = syscall0(SYS_YIELD);
    }
}

pub fn exit(status: i32) -> ! {
    unsafe {
        let _ = syscall1(SYS_EXIT, status as u64);
    }
    loop {
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}

/// RAII-managed memory allocation backed by Purple OS `0x1108` / `0x1109`.
pub struct FrameBuffer {
    ptr: *mut u32,
    cap_pixels: usize,
    pub width: u32,
    pub height: u32,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let pixels = (width as usize).saturating_mul(height as usize).max(1);
        let bytes = pixels.saturating_mul(4);
        let raw = alloc_pages(bytes)?;
        Some(Self {
            ptr: raw as *mut u32,
            cap_pixels: pixels,
            width,
            height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        let needed = (width as usize).saturating_mul(height as usize).max(1);
        if needed <= self.cap_pixels {
            self.width = width;
            self.height = height;
            return true;
        }
        let bytes = needed.saturating_mul(4);
        if let Some(new_raw) = alloc_pages(bytes) {
            free_pages(self.ptr as *mut u8);
            self.ptr = new_raw as *mut u32;
            self.cap_pixels = needed;
            self.width = width;
            self.height = height;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u32] {
        let len = (self.width as usize).saturating_mul(self.height as usize);
        unsafe { core::slice::from_raw_parts(self.ptr, len) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u32] {
        let len = (self.width as usize).saturating_mul(self.height as usize);
        unsafe { core::slice::from_raw_parts_mut(self.ptr, len) }
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            free_pages(self.ptr as *mut u8);
            self.ptr = core::ptr::null_mut();
        }
    }
}
