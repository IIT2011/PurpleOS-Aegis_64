// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC MPEG Audio (MP3) Frame Sync and Header Demuxer.
//! Port of modules/demux/mpeg/mp3.c into Pure Rust.

use vlccore::fourcc::VLC_CODEC_MP3;
use vlccore::es::EsFormat;
use vlccore::stream::{StreamError, VlcStream};

#[derive(Clone, Debug)]
pub struct Mp3Header {
    pub version: u8,      // 1 = MPEG-1, 2 = MPEG-2, 3 = MPEG-2.5
    pub layer: u8,        // 1 = Layer I, 2 = Layer II, 3 = Layer III
    pub bitrate: u32,     // bps
    pub sample_rate: u32, // Hz
    pub channels: u16,
    pub frame_size: usize,
    pub padding: bool,
}

const BITRATES_V1_L3: [u32; 16] = [
    0, 32000, 40000, 48000, 56000, 64000, 80000, 96000,
    112000, 128000, 160000, 192000, 224000, 256000, 320000, 0,
];

const SAMPLE_RATES_V1: [u32; 4] = [44100, 48000, 32000, 0];

impl Mp3Header {
    pub fn parse(header: u32) -> Option<Self> {
        // Sync word: 11 bits all 1s (0xFFE00000)
        if (header & 0xFFE00000) != 0xFFE00000 {
            return None;
        }

        let id = (header >> 19) & 0x3;
        let layer_bits = (header >> 17) & 0x3;
        let bitrate_idx = ((header >> 12) & 0xF) as usize;
        let samplerate_idx = ((header >> 10) & 0x3) as usize;
        let padding = ((header >> 9) & 0x1) == 1;
        let channel_mode = (header >> 6) & 0x3;

        if id != 3 || layer_bits != 1 {
            // Only MPEG-1 Layer 3 for now
            return None;
        }

        let bitrate = BITRATES_V1_L3[bitrate_idx];
        let sample_rate = SAMPLE_RATES_V1[samplerate_idx];
        if bitrate == 0 || sample_rate == 0 {
            return None;
        }

        let channels = if channel_mode == 3 { 1 } else { 2 };
        let pad_bytes = if padding { 1 } else { 0 };
        let frame_size = ((144 * bitrate) / sample_rate + pad_bytes) as usize;

        Some(Self {
            version: 1,
            layer: 3,
            bitrate,
            sample_rate,
            channels,
            frame_size,
            padding,
        })
    }
}

pub struct Mp3Demuxer {
    format: EsFormat,
}

impl Mp3Demuxer {
    pub fn probe(stream: &mut dyn VlcStream) -> Result<Self, StreamError> {
        stream.seek(0)?;
        let mut hdr = [0u8; 4];
        stream.read_exact(&mut hdr)?;
        let u = u32::from_be_bytes(hdr);
        if let Some(mp3) = Mp3Header::parse(u) {
            let es_fmt = EsFormat::new_audio(VLC_CODEC_MP3, mp3.channels, mp3.sample_rate, 16);
            Ok(Self { format: es_fmt })
        } else {
            Err(StreamError::Unsupported)
        }
    }

    pub fn format(&self) -> &EsFormat {
        &self.format
    }
}
