// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Raw Audio Demuxer.
//! Port of modules/demux/rawaud.c into Pure Rust.

use vlccore::block::Block;
use vlccore::es::EsFormat;
use vlccore::fourcc::VLC_CODEC_S16L;
use vlccore::stream::{StreamError, VlcStream};
use vlccore::tick::VlcTick;

#[derive(Debug)]
pub struct RawAudDemuxer {
    format: EsFormat,
    sample_rate: u32,
    channels: u16,
    bytes_read: u64,
}

impl RawAudDemuxer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            format: EsFormat::new_audio(VLC_CODEC_S16L, channels, sample_rate, 16),
            sample_rate,
            channels,
            bytes_read: 0,
        }
    }

    pub fn format(&self) -> &EsFormat {
        &self.format
    }

    pub fn demux_chunk(
        &mut self,
        stream: &mut dyn VlcStream,
        bytes: usize,
    ) -> Result<Option<Block>, StreamError> {
        let mut buf = vec![0u8; bytes];
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(None);
        }
        buf.truncate(n);

        let frame_size = (self.channels * 2) as u64;
        let samples_so_far = if frame_size > 0 { self.bytes_read / frame_size } else { 0 };
        let pts_ns = if self.sample_rate > 0 {
            (samples_so_far * 1_000_000_000) / self.sample_rate as u64
        } else {
            0
        };

        self.bytes_read += n as u64;

        let mut block = Block::from_slice(&buf);
        block.pts = VlcTick::from_nanos(pts_ns as i64);
        block.dts = block.pts;
        Ok(Some(block))
    }
}
