//! SDXC & SDHC High Capacity Card Drivers (Aegis64 Subsystem)
//!
//! Provides SD card block device abstractions mapped to
//! canonical storage paths (`/sdxc/card1`, `/sdhc/card1`, ...).

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};

pub struct SdCardDevice {
    pub name: &'static str,
    pub total_sectors: u64,
    pub block_size: u32,
    pub is_sdxc: bool,
}

impl SdCardDevice {
    pub const fn new(name: &'static str, is_sdxc: bool, total_sectors: u64) -> Self {
        Self {
            name,
            total_sectors,
            block_size: 512,
            is_sdxc,
        }
    }
}

impl BlockDevice for SdCardDevice {
    fn name(&self) -> &str {
        self.name
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn category(&self) -> StorageCategory {
        if self.is_sdxc {
            StorageCategory::Sdxc
        } else {
            StorageCategory::Sdhc
        }
    }

    fn read_blocks(&mut self, _lba: u64, count: u32, _buf: &mut [u8]) -> i32 {
        if count == 0 {
            BLOCK_DEV_OK
        } else {
            BLOCK_DEV_ERR_IO
        }
    }

    fn write_blocks(&mut self, _lba: u64, count: u32, _buf: &[u8]) -> i32 {
        if count == 0 {
            BLOCK_DEV_OK
        } else {
            BLOCK_DEV_ERR_IO
        }
    }
}
