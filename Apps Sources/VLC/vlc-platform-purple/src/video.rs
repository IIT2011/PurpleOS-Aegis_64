// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Video Output and Window Provider for PurpleOS Linear Compositor.
//! Port of modules/video_output/purple.c and window_purple.c into Pure Rust.

use std::sync::{Arc, Mutex};
use vlccore::es::VideoFormat;
use vlccore::hw_accel::HardwareAccelerator;
use vlccore::vout::{VideoOutput, VideoPicture};
use crate::accel::PurpleHardwareAccelerator;
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

    pub fn hw_clear(&self, color: u32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.frame_buffer.fill(color);
        Ok(())
    }

    pub fn hw_fill_rect(&self, x: u32, y: u32, w: u32, h: u32, color: u32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        let width = inner.width;
        let height = inner.height;
        let x_end = (x + w).min(width);
        let y_end = (y + h).min(height);
        for cy in y..y_end {
            let row = (cy * width) as usize;
            for cx in x..x_end {
                inner.frame_buffer[row + cx as usize] = color;
            }
        }
        Ok(())
    }

    pub fn hw_blit(&self, src: &[u32], src_pitch: u32, dst_x: u32, dst_y: u32, w: u32, h: u32) -> Result<usize, String> {
        let mut inner = self.inner.lock().unwrap();
        let width = inner.width as usize;
        let height = inner.height as usize;
        let copy_w = (w as usize).min(width.saturating_sub(dst_x as usize));
        let copy_h = (h as usize).min(height.saturating_sub(dst_y as usize));
        let src_stride = (src_pitch / 4) as usize;

        for r in 0..copy_h {
            let dst_row = (dst_y as usize + r) * width + (dst_x as usize);
            let src_row = r * src_stride;
            if src_row + copy_w <= src.len() && dst_row + copy_w <= inner.frame_buffer.len() {
                inner.frame_buffer[dst_row..dst_row + copy_w]
                    .copy_from_slice(&src[src_row..src_row + copy_w]);
            }
        }
        Ok(copy_w * copy_h)
    }
}

pub struct PurpleVideoOutput {
    window: PurpleWindow,
    hw_accel: PurpleHardwareAccelerator,
}

impl PurpleVideoOutput {
    pub fn new(window: PurpleWindow) -> Self {
        Self {
            window,
            hw_accel: PurpleHardwareAccelerator::probe(),
        }
    }

    pub fn hw_accel(&self) -> &PurpleHardwareAccelerator {
        &self.hw_accel
    }

    pub fn hw_accel_mut(&mut self) -> &mut PurpleHardwareAccelerator {
        &mut self.hw_accel
    }
}

impl VideoOutput for PurpleVideoOutput {
    fn open(&mut self, fmt: &VideoFormat) -> Result<(), String> {
        let _ = fmt;
        let _ = self.window.open()?;
        self.hw_accel = PurpleHardwareAccelerator::probe();
        Ok(())
    }

    fn display(&mut self, pic: &VideoPicture) -> Result<(), String> {
        self.window.with_buffer_mut(|buf| {
            let copy_w = self.window.width().min(pic.width) as usize;
            let copy_h = self.window.height().min(pic.height) as usize;
            let src_chunks = pic.data.chunks_exact(4);
            let total_pixels = copy_w * copy_h;
            let mut i = 0;
            let mut src_iter = src_chunks;
            while i + 4 <= total_pixels {
                if let (Some(c0), Some(c1), Some(c2), Some(c3)) =
                    (src_iter.next(), src_iter.next(), src_iter.next(), src_iter.next())
                {
                    buf[i] = u32::from_le_bytes([c0[0], c0[1], c0[2], c0[3]]);
                    buf[i + 1] = u32::from_le_bytes([c1[0], c1[1], c1[2], c1[3]]);
                    buf[i + 2] = u32::from_le_bytes([c2[0], c2[1], c2[2], c2[3]]);
                    buf[i + 3] = u32::from_le_bytes([c3[0], c3[1], c3[2], c3[3]]);
                    i += 4;
                } else {
                    break;
                }
            }
            while i < total_pixels {
                if let Some(c) = src_iter.next() {
                    buf[i] = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    i += 1;
                } else {
                    break;
                }
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

    fn hw_accelerator(&self) -> Option<&dyn HardwareAccelerator> {
        Some(&self.hw_accel)
    }

    fn hw_accelerator_mut(&mut self) -> Option<&mut dyn HardwareAccelerator> {
        Some(&mut self.hw_accel)
    }
}
