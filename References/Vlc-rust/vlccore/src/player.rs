// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Player State Machine and Controller.
//! Port of include/vlc_player.h into Pure Rust.

use crate::aout::AudioOutput;
use crate::clock::VlcClock;
use crate::tick::VlcTick;
use crate::vout::VideoOutput;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Opening,
    Buffering,
    Playing,
    Paused,
    Stopping,
}

pub struct VlcPlayer {
    state: PlayerState,
    clock: VlcClock,
    position: f64,
    time: VlcTick,
    length: VlcTick,
    volume: f32,
    muted: bool,
    rate: f32,
    aout: Box<dyn AudioOutput>,
    vout: Box<dyn VideoOutput>,
}

impl VlcPlayer {
    pub fn new(aout: Box<dyn AudioOutput>, vout: Box<dyn VideoOutput>) -> Self {
        Self {
            state: PlayerState::Stopped,
            clock: VlcClock::new(),
            position: 0.0,
            time: VlcTick::ZERO,
            length: VlcTick::ZERO,
            volume: 1.0,
            muted: false,
            rate: 1.0,
            aout,
            vout,
        }
    }

    pub fn state(&self) -> PlayerState {
        self.state
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlayerState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state == PlayerState::Paused
    }

    pub fn play(&mut self) -> Result<(), String> {
        if self.state == PlayerState::Paused {
            self.aout.pause(false)?;
            self.state = PlayerState::Playing;
        } else if self.state == PlayerState::Stopped {
            self.state = PlayerState::Playing;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.state == PlayerState::Playing {
            self.aout.pause(true)?;
            self.state = PlayerState::Paused;
        }
        Ok(())
    }

    pub fn toggle_pause(&mut self) -> Result<(), String> {
        if self.is_playing() {
            self.pause()
        } else {
            self.play()
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.state = PlayerState::Stopped;
        self.aout.flush()?;
        self.clock.reset();
        self.time = VlcTick::ZERO;
        self.position = 0.0;
        Ok(())
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
        self.clock.set_rate(rate);
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn set_volume(&mut self, vol: f32) -> Result<(), String> {
        self.volume = vol.clamp(0.0, 2.0);
        if !self.muted {
            self.aout.set_volume(self.volume)?;
        }
        Ok(())
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_mute(&mut self, mute: bool) -> Result<(), String> {
        self.muted = mute;
        if self.muted {
            self.aout.set_volume(0.0)?;
        } else {
            self.aout.set_volume(self.volume)?;
        }
        Ok(())
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_length(&mut self, length: VlcTick) {
        self.length = length;
    }

    pub fn length(&self) -> VlcTick {
        self.length
    }

    pub fn set_time(&mut self, time: VlcTick) {
        self.time = time;
        if self.length.is_valid() && self.length.to_nanos() > 0 {
            self.position = (time.to_nanos() as f64 / self.length.to_nanos() as f64).clamp(0.0, 1.0);
        }
    }

    pub fn time(&self) -> VlcTick {
        self.time
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn seek(&mut self, pos: f64) -> Result<(), String> {
        self.position = pos.clamp(0.0, 1.0);
        if self.length.is_valid() && self.length.to_nanos() > 0 {
            let ns = (self.length.to_nanos() as f64 * self.position) as i64;
            self.time = VlcTick::from_nanos(ns);
        }
        self.aout.flush()?;
        Ok(())
    }

    pub fn aout_mut(&mut self) -> &mut Box<dyn AudioOutput> {
        &mut self.aout
    }

    pub fn vout_mut(&mut self) -> &mut Box<dyn VideoOutput> {
        &mut self.vout
    }
}
