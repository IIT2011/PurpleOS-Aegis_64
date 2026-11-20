// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Audio Output Plugin for PurpleOS Native Audio Subsystem.
//! Port of modules/audio_output/purple.c into Pure Rust.

use vlccore::aout::AudioOutput;
use vlccore::block::Block;
use vlccore::es::AudioFormat;
use crate::syscall::*;

pub struct PurpleAudioOutput {
    active: bool,
    volume: f32,
    sample_rate: u32,
    channels: u16,
}

impl Default for PurpleAudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl PurpleAudioOutput {
    pub fn new() -> Self {
        Self {
            active: false,
            volume: 1.0,
            sample_rate: 44100,
            channels: 2,
        }
    }
}

impl AudioOutput for PurpleAudioOutput {
    fn open(&mut self, fmt: &AudioFormat) -> Result<(), String> {
        self.sample_rate = fmt.rate;
        self.channels = fmt.channels;
        let res = unsafe { syscall0(SYS_PURPLE_AUDIO_STREAM_START) };
        if res < 0 {
            return Err(format!("SYS_PURPLE_AUDIO_STREAM_START failed: {}", res));
        }
        self.active = true;
        Ok(())
    }

    fn play(&mut self, block: &Block) -> Result<(), String> {
        if !self.active || block.is_empty() {
            return Ok(());
        }

        let ptr = block.data.as_ptr() as u64;
        let len = block.data.len() as u64;
        let written = unsafe { syscall2(SYS_PURPLE_AUDIO_SUBMIT_PCM, ptr, len) };
        if written < 0 {
            return Err(format!("SYS_PURPLE_AUDIO_SUBMIT_PCM failed: {}", written));
        }
        Ok(())
    }

    fn pause(&mut self, paused: bool) -> Result<(), String> {
        if paused {
            unsafe { syscall0(SYS_PURPLE_AUDIO_STREAM_STOP) };
        } else {
            unsafe { syscall0(SYS_PURPLE_AUDIO_STREAM_START) };
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), String> {
        self.volume = volume.clamp(0.0, 2.0);
        let vol_int = (self.volume * 100.0) as u64;
        unsafe { syscall1(SYS_PURPLE_AUDIO_SET_VOLUME, vol_int) };
        Ok(())
    }

    fn close(&mut self) {
        if self.active {
            unsafe { syscall0(SYS_PURPLE_AUDIO_STREAM_STOP) };
            self.active = false;
        }
    }
}

impl Drop for PurpleAudioOutput {
    fn drop(&mut self) {
        self.close();
    }
}
