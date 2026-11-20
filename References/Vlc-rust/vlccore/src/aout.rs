// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Audio Output Subsystem.
//! Port of include/vlc_aout.h into Pure Rust.

use crate::block::Block;
use crate::es::AudioFormat;

pub trait AudioOutput: Send {
    fn open(&mut self, fmt: &AudioFormat) -> Result<(), String>;
    fn play(&mut self, block: &Block) -> Result<(), String>;
    fn pause(&mut self, paused: bool) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
    fn set_volume(&mut self, volume: f32) -> Result<(), String>;
    fn close(&mut self);
}

pub struct DummyAudioOutput {
    volume: f32,
    active: bool,
}

impl Default for DummyAudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl DummyAudioOutput {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            active: false,
        }
    }
}

impl AudioOutput for DummyAudioOutput {
    fn open(&mut self, _fmt: &AudioFormat) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn play(&mut self, _block: &Block) -> Result<(), String> {
        Ok(())
    }

    fn pause(&mut self, _paused: bool) -> Result<(), String> {
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), String> {
        self.volume = volume.clamp(0.0, 2.0);
        Ok(())
    }

    fn close(&mut self) {
        self.active = false;
    }
}
