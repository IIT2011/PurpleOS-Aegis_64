// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Stream Access Abstraction.
//! Port of include/vlc_stream.h into Pure Rust.

#[derive(Debug)]
pub enum StreamError {
    EndOfStream,
    IoError(String),
    SeekError,
    Unsupported,
}

pub trait VlcStream: Send {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError>;
    fn seek(&mut self, offset: u64) -> Result<u64, StreamError>;
    fn tell(&self) -> u64;
    fn size(&self) -> Option<u64>;

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), StreamError> {
        let mut total = 0;
        while total < buf.len() {
            let n = self.read(&mut buf[total..])?;
            if n == 0 {
                return Err(StreamError::EndOfStream);
            }
            total += n;
        }
        Ok(())
    }

    fn read_u32_le(&mut self) -> Result<u32, StreamError> {
        let mut b = [0u8; 4];
        self.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_u16_le(&mut self) -> Result<u16, StreamError> {
        let mut b = [0u8; 2];
        self.read_exact(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }
}

pub struct MemoryStream {
    data: Vec<u8>,
    pos: usize,
}

impl MemoryStream {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }
}

impl VlcStream for MemoryStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let available = self.data.len() - self.pos;
        let to_read = available.min(buf.len());
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }

    fn seek(&mut self, offset: u64) -> Result<u64, StreamError> {
        let target = offset as usize;
        if target > self.data.len() {
            return Err(StreamError::SeekError);
        }
        self.pos = target;
        Ok(self.pos as u64)
    }

    fn tell(&self) -> u64 {
        self.pos as u64
    }

    fn size(&self) -> Option<u64> {
        Some(self.data.len() as u64)
    }
}
