// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Elementary Stream (ES) Format definitions.
//! Port of include/vlc_es.h into Pure Rust.

use crate::fourcc::FourCC;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EsCategory {
    Unknown,
    Audio,
    Video,
    Subtitle,
    Data,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioFormat {
    pub fourcc: FourCC,
    pub channels: u16,
    pub rate: u32,
    pub bits_per_sample: u16,
    pub block_align: u16,
    pub channel_mask: u32,
}

impl AudioFormat {
    pub fn new(fourcc: FourCC, channels: u16, rate: u32, bits_per_sample: u16) -> Self {
        let block_align = (channels * ((bits_per_sample + 7) / 8)) as u16;
        Self {
            fourcc,
            channels,
            rate,
            bits_per_sample,
            block_align,
            channel_mask: if channels == 1 { 0x4 } else { 0x3 }, // Front Center vs Front L/R
        }
    }

    pub fn bytes_per_second(&self) -> u32 {
        self.rate * self.block_align as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFormat {
    pub fourcc: FourCC,
    pub width: u32,
    pub height: u32,
    pub visible_width: u32,
    pub visible_height: u32,
    pub sar_num: u32,
    pub sar_den: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
}

impl VideoFormat {
    pub fn new(fourcc: FourCC, width: u32, height: u32) -> Self {
        Self {
            fourcc,
            width,
            height,
            visible_width: width,
            visible_height: height,
            sar_num: 1,
            sar_den: 1,
            frame_rate_num: 30,
            frame_rate_den: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EsFormat {
    pub category: EsCategory,
    pub codec: FourCC,
    pub id: i32,
    pub profile: i32,
    pub level: i32,
    pub bitrate: u32,
    pub audio: Option<AudioFormat>,
    pub video: Option<VideoFormat>,
}

impl EsFormat {
    pub fn new_audio(codec: FourCC, channels: u16, rate: u32, bits_per_sample: u16) -> Self {
        Self {
            category: EsCategory::Audio,
            codec,
            id: 0,
            profile: 0,
            level: 0,
            bitrate: rate * channels as u32 * bits_per_sample as u32,
            audio: Some(AudioFormat::new(codec, channels, rate, bits_per_sample)),
            video: None,
        }
    }

    pub fn new_video(codec: FourCC, width: u32, height: u32) -> Self {
        Self {
            category: EsCategory::Video,
            codec,
            id: 0,
            profile: 0,
            level: 0,
            bitrate: 0,
            audio: None,
            video: Some(VideoFormat::new(codec, width, height)),
        }
    }
}
