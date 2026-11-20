// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Tick definitions and timestamp conversions.
//! Port of include/vlc_tick.h into Pure Rust.

pub const VLC_TICK_INVALID: i64 = -1;
pub const VLC_TICK_0: i64 = 0;

pub const VLC_TICK_FROM_SEC: i64 = 1_000_000_000;
pub const VLC_TICK_FROM_MS: i64 = 1_000_000;
pub const VLC_TICK_FROM_US: i64 = 1_000;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlcTick(pub i64);

impl VlcTick {
    pub const INVALID: Self = Self(VLC_TICK_INVALID);
    pub const ZERO: Self = Self(VLC_TICK_0);

    #[inline]
    pub const fn from_nanos(ns: i64) -> Self {
        Self(ns)
    }

    #[inline]
    pub const fn from_micros(us: i64) -> Self {
        Self(us * VLC_TICK_FROM_US)
    }

    #[inline]
    pub const fn from_ms(ms: i64) -> Self {
        Self(ms * VLC_TICK_FROM_MS)
    }

    #[inline]
    pub const fn from_sec(s: i64) -> Self {
        Self(s * VLC_TICK_FROM_SEC)
    }

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    #[inline]
    pub const fn to_nanos(&self) -> i64 {
        self.0
    }

    #[inline]
    pub const fn to_micros(&self) -> i64 {
        self.0 / VLC_TICK_FROM_US
    }

    #[inline]
    pub const fn to_ms(&self) -> i64 {
        self.0 / VLC_TICK_FROM_MS
    }

    #[inline]
    pub fn to_sec(&self) -> f64 {
        self.0 as f64 / VLC_TICK_FROM_SEC as f64
    }
}

impl core::ops::Add for VlcTick {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for VlcTick {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
