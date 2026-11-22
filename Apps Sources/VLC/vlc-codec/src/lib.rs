// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Decoder and Converter Subsystem in Pure Rust.
//!
//! Provides decoders for:
//! - Raw PCM / Float formats (`pcm`)

pub mod pcm;
pub mod hw_converter;

pub use pcm::PcmDecoder;
pub use hw_converter::{nv12_to_argb8888_hw, scale_bilinear_hw, yuv420p_to_argb8888_hw, yuv_to_argb};

#[cfg(test)]
mod tests {
    use super::*;
    use vlccore::block::Block;
    use vlccore::es::AudioFormat;
    use vlccore::fourcc::VLC_CODEC_F32L;

    #[test]
    fn test_float_to_s16_conversion() {
        let fmt = AudioFormat::new(VLC_CODEC_F32L, 1, 44100, 32);
        let mut decoder = PcmDecoder::new(fmt);

        // 1.0f32 should convert to ~32767
        let f_val = 1.0f32;
        let block = Block::from_slice(&f_val.to_le_bytes());
        let decoded = decoder.decode(&block).expect("Decode failed");

        assert_eq!(decoded.size(), 2);
        let s16 = i16::from_le_bytes([decoded.data[0], decoded.data[1]]);
        assert_eq!(s16, 32767);
    }
}
