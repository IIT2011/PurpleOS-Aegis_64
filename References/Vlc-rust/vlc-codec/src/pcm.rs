// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Audio PCM and Floating Point Converter / Decoder.
//! Port of modules/audio_filter/converter/format.c into Pure Rust.

use vlccore::block::Block;
use vlccore::es::AudioFormat;
use vlccore::fourcc::{VLC_CODEC_F32L, VLC_CODEC_S16L};

pub struct PcmDecoder {
    input_format: AudioFormat,
    output_format: AudioFormat,
}

impl PcmDecoder {
    pub fn new(input_format: AudioFormat) -> Self {
        let output_format = AudioFormat::new(
            VLC_CODEC_S16L,
            input_format.channels,
            input_format.rate,
            16,
        );
        Self {
            input_format,
            output_format,
        }
    }

    pub fn output_format(&self) -> &AudioFormat {
        &self.output_format
    }

    pub fn decode(&mut self, input: &Block) -> Option<Block> {
        if input.is_empty() {
            return None;
        }

        let mut output_data = Vec::new();

        if self.input_format.fourcc == VLC_CODEC_S16L {
            // Passthrough S16LE
            output_data = input.data.clone();
        } else if self.input_format.fourcc == VLC_CODEC_F32L {
            // Convert IEEE 32-bit float (-1.0 .. 1.0) to signed 16-bit integer
            let chunks = input.data.chunks_exact(4);
            output_data.reserve(chunks.len() * 2);
            for chunk in chunks {
                let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let clamped = f.clamp(-1.0, 1.0);
                let sample_i16 = (clamped * 32767.0) as i16;
                output_data.extend_from_slice(&sample_i16.to_le_bytes());
            }
        } else if self.input_format.bits_per_sample == 8 {
            // 8-bit unsigned PCM to signed 16-bit
            output_data.reserve(input.data.len() * 2);
            for &u8_val in &input.data {
                let s16 = (u8_val as i16 - 128) << 8;
                output_data.extend_from_slice(&s16.to_le_bytes());
            }
        } else {
            // Unsupported input, passthrough as-is
            output_data = input.data.clone();
        }

        let mut out = Block::from_slice(&output_data);
        out.pts = input.pts;
        out.dts = input.dts;
        out.length = input.length;
        Some(out)
    }
}
