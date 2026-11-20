// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Demuxer Subsystem in Pure Rust.
//!
//! Provides container parsers for:
//! - RIFF / WAVE audio (`wav`)
//! - Raw Audio (`raw`)
//! - MPEG Audio (`mp3`)

pub mod wav;
pub mod raw;
pub mod mp3;

pub use wav::WavDemuxer;
pub use raw::RawAudDemuxer;
pub use mp3::{Mp3Demuxer, Mp3Header};

#[cfg(test)]
mod tests {
    use super::*;
    use vlccore::stream::MemoryStream;

    fn create_dummy_wav() -> Vec<u8> {
        let mut data = Vec::new();
        // RIFF header
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&36u32.to_le_bytes()); // Size
        data.extend_from_slice(b"WAVE");

        // fmt chunk
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
        data.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 = PCM)
        data.extend_from_slice(&2u16.to_le_bytes());  // NumChannels (2)
        data.extend_from_slice(&44100u32.to_le_bytes()); // SampleRate
        data.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // ByteRate
        data.extend_from_slice(&4u16.to_le_bytes());  // BlockAlign
        data.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample

        // data chunk
        data.extend_from_slice(b"data");
        data.extend_from_slice(&16u32.to_le_bytes()); // 16 bytes of sample data
        data.extend_from_slice(&[0u8; 16]);
        data
    }

    #[test]
    fn test_wav_demuxer() {
        let wav_bytes = create_dummy_wav();
        let mut stream = MemoryStream::new(wav_bytes);
        let mut demux = WavDemuxer::probe(&mut stream).expect("Wav probe failed");

        assert_eq!(demux.channels(), 2);
        assert_eq!(demux.bits_per_sample(), 16);

        let block = demux.demux_block(&mut stream, 4).expect("Demux block failed").unwrap();
        assert_eq!(block.size(), 16);
    }
}
