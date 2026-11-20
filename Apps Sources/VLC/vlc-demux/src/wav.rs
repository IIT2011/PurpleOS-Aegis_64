// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC RIFF/WAVE Demuxer.
//! Port of modules/demux/wav.c into Pure Rust.

use vlccore::block::Block;
use vlccore::es::EsFormat;
use vlccore::fourcc::{VLC_CODEC_F32L, VLC_CODEC_S16L};
use vlccore::stream::{StreamError, VlcStream};
use vlccore::tick::VlcTick;

#[derive(Debug)]
pub struct WavDemuxer {
    format: EsFormat,
    data_offset: u64,
    data_size: u64,
    current_offset: u64,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    block_align: u16,
    duration: VlcTick,
}

impl WavDemuxer {
    pub fn probe(stream: &mut dyn VlcStream) -> Result<Self, StreamError> {
        stream.seek(0)?;
        let mut riff = [0u8; 4];
        stream.read_exact(&mut riff)?;
        if &riff != b"RIFF" {
            return Err(StreamError::Unsupported);
        }

        let _file_size = stream.read_u32_le()?;
        let mut wave = [0u8; 4];
        stream.read_exact(&mut wave)?;
        if &wave != b"WAVE" {
            return Err(StreamError::Unsupported);
        }

        let mut audio_format = 1u16; // 1 = PCM, 3 = IEEE float
        let mut channels = 2u16;
        let mut sample_rate = 44100u32;
        let mut bits_per_sample = 16u16;
        let mut block_align = 4u16;
        let mut data_offset = 0u64;
        let mut data_size = 0u64;
        let mut found_fmt = false;

        // Parse chunks
        while let Ok(chunk_id) = (|| -> Result<[u8; 4], StreamError> {
            let mut id = [0u8; 4];
            stream.read_exact(&mut id)?;
            Ok(id)
        })() {
            let chunk_size = stream.read_u32_le()? as u64;
            let chunk_start = stream.tell();

            if &chunk_id == b"fmt " {
                audio_format = stream.read_u16_le()?;
                channels = stream.read_u16_le()?;
                sample_rate = stream.read_u32_le()?;
                let _byte_rate = stream.read_u32_le()?;
                block_align = stream.read_u16_le()?;
                bits_per_sample = stream.read_u16_le()?;
                found_fmt = true;
            } else if &chunk_id == b"data" {
                data_offset = chunk_start;
                data_size = chunk_size;
                break;
            }

            stream.seek(chunk_start + chunk_size)?;
        }

        if !found_fmt || data_offset == 0 {
            return Err(StreamError::Unsupported);
        }

        let codec = if audio_format == 3 {
            VLC_CODEC_F32L
        } else {
            VLC_CODEC_S16L
        };

        let es_fmt = EsFormat::new_audio(codec, channels, sample_rate, bits_per_sample);
        let bytes_per_sec = (sample_rate * block_align as u32) as u64;
        let duration_ns = if bytes_per_sec > 0 {
            (data_size * 1_000_000_000) / bytes_per_sec
        } else {
            0
        };

        stream.seek(data_offset)?;

        Ok(Self {
            format: es_fmt,
            data_offset,
            data_size,
            current_offset: 0,
            sample_rate,
            channels,
            bits_per_sample,
            block_align,
            duration: VlcTick::from_nanos(duration_ns as i64),
        })
    }

    pub fn format(&self) -> &EsFormat {
        &self.format
    }

    pub fn duration(&self) -> VlcTick {
        self.duration
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn bits_per_sample(&self) -> u16 {
        self.bits_per_sample
    }

    pub fn demux_block(
        &mut self,
        stream: &mut dyn VlcStream,
        max_samples: usize,
    ) -> Result<Option<Block>, StreamError> {
        if self.current_offset >= self.data_size {
            return Ok(None);
        }

        let bytes_wanted = max_samples * self.block_align as usize;
        let remaining = (self.data_size - self.current_offset) as usize;
        let to_read = bytes_wanted.min(remaining);

        let mut buf = vec![0u8; to_read];
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(None);
        }
        buf.truncate(n);

        let pts_ns = if self.block_align > 0 && self.sample_rate > 0 {
            let samples_so_far = self.current_offset / self.block_align as u64;
            (samples_so_far * 1_000_000_000) / self.sample_rate as u64
        } else {
            0
        };

        self.current_offset += n as u64;

        let mut block = Block::from_slice(&buf);
        block.pts = VlcTick::from_nanos(pts_ns as i64);
        block.dts = block.pts;

        let samples_in_block = n / self.block_align as usize;
        let len_ns = (samples_in_block as u64 * 1_000_000_000) / self.sample_rate as u64;
        block.length = VlcTick::from_nanos(len_ns as i64);

        Ok(Some(block))
    }

    pub fn seek_to_time(
        &mut self,
        stream: &mut dyn VlcStream,
        target: VlcTick,
    ) -> Result<(), StreamError> {
        let sec = target.to_sec();
        let target_sample = (sec * self.sample_rate as f64) as u64;
        let mut target_byte = target_sample * self.block_align as u64;
        if target_byte > self.data_size {
            target_byte = self.data_size;
        }

        let file_pos = self.data_offset + target_byte;
        stream.seek(file_pos)?;
        self.current_offset = target_byte;
        Ok(())
    }
}
