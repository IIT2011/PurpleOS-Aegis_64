// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Master Clock and Presentation Timestamp Synchronization.
//! Port of include/vlc_clock.h into Pure Rust.

use crate::tick::VlcTick;

#[derive(Clone, Debug)]
pub struct VlcClock {
    rate: f32,
    pts_delay: VlcTick,
    last_pts: VlcTick,
    last_system: VlcTick,
}

impl Default for VlcClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VlcClock {
    pub fn new() -> Self {
        Self {
            rate: 1.0,
            pts_delay: VlcTick::from_ms(300), // Default 300ms buffering delay
            last_pts: VlcTick::INVALID,
            last_system: VlcTick::INVALID,
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        if rate > 0.05 && rate <= 10.0 {
            self.rate = rate;
        }
    }

    pub fn get_rate(&self) -> f32 {
        self.rate
    }

    pub fn set_delay(&mut self, delay: VlcTick) {
        self.pts_delay = delay;
    }

    pub fn update(&mut self, pts: VlcTick, system_now: VlcTick) {
        self.last_pts = pts;
        self.last_system = system_now;
    }

    pub fn to_system(&self, pts: VlcTick) -> Option<VlcTick> {
        if !self.last_pts.is_valid() || !self.last_system.is_valid() {
            return None;
        }
        let delta = pts - self.last_pts;
        let scaled_delta = (delta.to_nanos() as f64 / self.rate as f64) as i64;
        Some(self.last_system + VlcTick::from_nanos(scaled_delta) + self.pts_delay)
    }

    pub fn reset(&mut self) {
        self.last_pts = VlcTick::INVALID;
        self.last_system = VlcTick::INVALID;
    }
}
