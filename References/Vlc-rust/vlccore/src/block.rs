// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Memory Block Subsystem.
//! Port of include/vlc_block.h into Pure Rust.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use crate::tick::VlcTick;

pub const BLOCK_FLAG_DISCONTINUITY: u32 = 0x0001;
pub const BLOCK_FLAG_TYPE_I: u32        = 0x0002;
pub const BLOCK_FLAG_TYPE_P: u32        = 0x0004;
pub const BLOCK_FLAG_TYPE_B: u32        = 0x0008;
pub const BLOCK_FLAG_HEADER: u32        = 0x0020;
pub const BLOCK_FLAG_CORRUPTED: u32     = 0x0100;
pub const BLOCK_FLAG_PREROLL: u32       = 0x0200;

#[derive(Clone, Debug)]
pub struct Block {
    pub data: Vec<u8>,
    pub flags: u32,
    pub pts: VlcTick,
    pub dts: VlcTick,
    pub length: VlcTick,
}

impl Block {
    pub fn alloc(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            flags: 0,
            pts: VlcTick::INVALID,
            dts: VlcTick::INVALID,
            length: VlcTick::ZERO,
        }
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
            flags: 0,
            pts: VlcTick::INVALID,
            dts: VlcTick::INVALID,
            length: VlcTick::ZERO,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[derive(Clone, Default)]
pub struct BlockFifo {
    inner: Arc<Mutex<BlockFifoInner>>,
}

#[derive(Default)]
struct BlockFifoInner {
    queue: VecDeque<Block>,
    total_bytes: usize,
}

impl BlockFifo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&self, block: Block) {
        let mut inner = self.inner.lock().unwrap();
        inner.total_bytes += block.size();
        inner.queue.push_back(block);
    }

    pub fn get(&self) -> Option<Block> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(block) = inner.queue.pop_front() {
            inner.total_bytes -= block.size();
            Some(block)
        } else {
            None
        }
    }

    pub fn depth(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.queue.len()
    }

    pub fn bytes(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.total_bytes
    }

    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.queue.is_empty()
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.queue.clear();
        inner.total_bytes = 0;
    }
}
