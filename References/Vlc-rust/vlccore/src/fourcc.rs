// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC FourCC Codec and Container Definitions.
//! Port of include/vlc_fourcc.h into Pure Rust.

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourCC(pub [u8; 4]);

impl FourCC {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub const fn from_u32(val: u32) -> Self {
        Self(val.to_le_bytes())
    }

    pub const fn as_u32(&self) -> u32 {
        u32::from_le_bytes(self.0)
    }

    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        if b.len() == 4 {
            Some(Self([b[0], b[1], b[2], b[3]]))
        } else {
            None
        }
    }
}

impl core::fmt::Debug for FourCC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FourCC(\"{}{}{}{}\")",
            self.0[0] as char, self.0[1] as char, self.0[2] as char, self.0[3] as char
        )
    }
}

impl core::fmt::Display for FourCC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            self.0[0] as char, self.0[1] as char, self.0[2] as char, self.0[3] as char
        )
    }
}

// Audio FourCCs
pub const VLC_CODEC_S16L: FourCC = FourCC::new(b's', b'1', b'6', b'l');
pub const VLC_CODEC_S16B: FourCC = FourCC::new(b's', b'1', b'6', b'b');
pub const VLC_CODEC_U16L: FourCC = FourCC::new(b'u', b'1', b'6', b'l');
pub const VLC_CODEC_U16B: FourCC = FourCC::new(b'u', b'1', b'6', b'b');
pub const VLC_CODEC_S32L: FourCC = FourCC::new(b's', b'3', b'2', b'l');
pub const VLC_CODEC_F32L: FourCC = FourCC::new(b'f', b'3', b'2', b'l');
pub const VLC_CODEC_MPGA: FourCC = FourCC::new(b'm', b'p', b'g', b'a');
pub const VLC_CODEC_MP3: FourCC  = FourCC::new(b'm', b'p', b'3', b' ');
pub const VLC_CODEC_FLAC: FourCC = FourCC::new(b'f', b'l', b'a', b'c');
pub const VLC_CODEC_VORB: FourCC = FourCC::new(b'v', b'o', b'r', b'b');
pub const VLC_CODEC_OPUS: FourCC = FourCC::new(b'O', b'p', b'u', b's');

#[cfg(target_endian = "little")]
pub const VLC_CODEC_S16N: FourCC = VLC_CODEC_S16L;
#[cfg(target_endian = "big")]
pub const VLC_CODEC_S16N: FourCC = VLC_CODEC_S16B;

// Video FourCCs
pub const VLC_CODEC_I420: FourCC = FourCC::new(b'I', b'4', b'2', b'0');
pub const VLC_CODEC_YV12: FourCC = FourCC::new(b'Y', b'V', b'1', b'2');
pub const VLC_CODEC_RGB24: FourCC = FourCC::new(b'R', b'V', b'2', b'4');
pub const VLC_CODEC_RGB32: FourCC = FourCC::new(b'R', b'V', b'3', b'2');
pub const VLC_CODEC_RGBA: FourCC = FourCC::new(b'R', b'G', b'B', b'A');
pub const VLC_CODEC_BGRA: FourCC = FourCC::new(b'B', b'G', b'R', b'A');
pub const VLC_CODEC_H264: FourCC = FourCC::new(b'h', b'2', b'6', b'4');
pub const VLC_CODEC_HEVC: FourCC = FourCC::new(b'h', b'e', b'v', b'c');
pub const VLC_CODEC_VP8: FourCC  = FourCC::new(b'V', b'P', b'8', b'0');
pub const VLC_CODEC_VP9: FourCC  = FourCC::new(b'V', b'P', b'9', b'0');
pub const VLC_CODEC_AV01: FourCC = FourCC::new(b'a', b'v', b'0', b'1');
