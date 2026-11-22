// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Video Output Subsystem.
//! Port of include/vlc_vout.h and include/vlc_vout_display.h into Pure Rust.

use crate::es::VideoFormat;
use crate::tick::VlcTick;

#[derive(Clone, Debug)]
pub struct VideoPicture {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub data: Vec<u8>,
    pub pts: VlcTick,
}

impl VideoPicture {
    pub fn new_rgba(width: u32, height: u32) -> Self {
        let pitch = width * 4;
        let data = vec![0u8; (pitch * height) as usize];
        Self {
            width,
            height,
            pitch,
            data,
            pts: VlcTick::INVALID,
        }
    }
}

pub trait VideoOutput: Send {
    fn open(&mut self, fmt: &VideoFormat) -> Result<(), String>;
    fn display(&mut self, pic: &VideoPicture) -> Result<(), String>;
    fn set_fullscreen(&mut self, fs: bool) -> Result<(), String>;
    fn close(&mut self);

    /// Optional reference to hardware accelerator
    fn hw_accelerator(&self) -> Option<&dyn crate::hw_accel::HardwareAccelerator> {
        None
    }

    /// Optional mutable reference to hardware accelerator
    fn hw_accelerator_mut(&mut self) -> Option<&mut dyn crate::hw_accel::HardwareAccelerator> {
        None
    }

    /// Check if video output is currently backed by hardware acceleration
    fn is_hw_accelerated(&self) -> bool {
        self.hw_accelerator().map(|a| a.is_active()).unwrap_or(false)
    }
}

pub struct DummyVideoOutput {
    active: bool,
}

impl Default for DummyVideoOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl DummyVideoOutput {
    pub fn new() -> Self {
        Self { active: false }
    }
}

impl VideoOutput for DummyVideoOutput {
    fn open(&mut self, _fmt: &VideoFormat) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn display(&mut self, _pic: &VideoPicture) -> Result<(), String> {
        Ok(())
    }

    fn set_fullscreen(&mut self, _fs: bool) -> Result<(), String> {
        Ok(())
    }

    fn close(&mut self) {
        self.active = false;
    }
}
