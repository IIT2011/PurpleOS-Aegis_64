// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Video Output and Window Provider for PurpleOS Linear Compositor.
//! Port of modules/video_output/purple.c and window_purple.c into Pure Rust.

use std::sync::{Arc, Mutex};
use vlccore::es::VideoFormat;
use vlccore::vout::{VideoOutput, VideoPicture};
use crate::syscall::*;

pub const PURPLE_GUI_WINDOW_NORMAL: u64 = 0;
pub const PURPLE_GUI_WINDOW_GLASS: u64  = 1;
pub const PURPLE_GUI_WINDOW_ACRYLIC: u64= 2;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GuiEvent {
    pub event_type: u32,
    pub p1: u32,
    pub p2: u32,
    pub p3: u32,
}

#[derive(Clone)]
pub struct PurpleWindow {
    inner: Arc<Mutex<PurpleWindowInner>>,
}

struct PurpleWindowInner {
    window_id: u32,
    width: u32,
    height: u32,
    active: bool,
    frame_buffer: Vec<u32>,
}

impl PurpleWindow {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            inner: Arc::new(Mutex::new(PurpleWindowInner {
                window_id: 0,
                width,
                height,
                active: false,
                frame_buffer: vec![0xFF1E1E2Eu32; size],
            })),
        }
    }

    pub fn open(&self) -> Result<u32, String> {
        let mut inner = self.inner.lock().unwrap();
        let ret = unsafe {
            syscall4(
                SYS_GUI_CREATE_WINDOW,
                inner.width as u64,
                inner.height as u64,
                PURPLE_GUI_WINDOW_GLASS,
                0,
            )
        };
        if ret < 0 {
            return Err(format!("SYS_GUI_CREATE_WINDOW failed: {}", ret));
        }
        inner.window_id = ret as u32;
        unsafe { syscall2(SYS_GUI_MAP_WINDOW, inner.window_id as u64, 1) };
        inner.active = true;
        Ok(inner.window_id)
    }

    pub fn window_id(&self) -> u32 {
        self.inner.lock().unwrap().window_id
    }

    pub fn width(&self) -> u32 {
        self.inner.lock().unwrap().width
    }

    pub fn height(&self) -> u32 {
        self.inner.lock().unwrap().height
    }

    pub fn with_buffer_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut [u32]) -> R,
    {
        let mut inner = self.inner.lock().unwrap();
        f(&mut inner.frame_buffer)
    }

    pub fn present(&self) -> Result<(), String> {
        let inner = self.inner.lock().unwrap();
        if !inner.active {
            return Ok(());
        }
        let ptr = inner.frame_buffer.as_ptr() as u64;
        let pitch = (inner.width * 4) as u64;
        let ret = unsafe { syscall3(SYS_GUI_PRESENT, inner.window_id as u64, ptr, pitch) };
        if ret < 0 {
            return Err(format!("SYS_GUI_PRESENT failed: {}", ret));
        }
        Ok(())
    }

    pub fn poll_event(&self) -> Option<GuiEvent> {
        let wid = self.inner.lock().unwrap().window_id;
        let mut evt = GuiEvent::default();
        let ptr = &mut evt as *mut GuiEvent as u64;
        let ret = unsafe { syscall2(SYS_GUI_POLL, wid as u64, ptr) };
        if ret > 0 {
            Some(evt)
        } else {
            None
        }
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.active {
            unsafe { syscall1(SYS_GUI_CLOSE_WINDOW, inner.window_id as u64) };
            inner.active = false;
        }
    }
}

pub struct PurpleVideoOutput {
    window: PurpleWindow,
}

impl PurpleVideoOutput {
    pub fn new(window: PurpleWindow) -> Self {
        Self { window }
    }
}

impl VideoOutput for PurpleVideoOutput {
    fn open(&mut self, fmt: &VideoFormat) -> Result<(), String> {
        let _ = fmt;
        let _ = self.window.open()?;
        Ok(())
    }

    fn display(&mut self, pic: &VideoPicture) -> Result<(), String> {
        self.window.with_buffer_mut(|buf| {
            let copy_w = self.window.width().min(pic.width) as usize;
            let copy_h = self.window.height().min(pic.height) as usize;
            let src_chunks = pic.data.chunks_exact(4);
            for (dst, src) in buf.iter_mut().zip(src_chunks).take(copy_w * copy_h) {
                *dst = u32::from_le_bytes([src[0], src[1], src[2], src[3]]);
            }
        });
        self.window.present()
    }

    fn set_fullscreen(&mut self, _fs: bool) -> Result<(), String> {
        Ok(())
    }

    fn close(&mut self) {
        self.window.close();
    }
}
