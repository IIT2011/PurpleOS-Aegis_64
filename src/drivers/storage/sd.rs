//! SDXC & SDHC High Capacity Card Drivers (Aegis64 Subsystem)
//!
//! Provides SD card block device abstractions mapped to
//! canonical storage paths (`/sdxc/card1`, `/sdhc/card1`, ...).

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_OK, BLOCK_DEV_ERR_BOUNDS};

pub struct SdCardDevice {
    pub name: &'static str,
    pub total_sectors: u64,
    pub block_size: u32,
    pub is_sdxc: bool,
    storage_buf: [u8; 32768], // In-kernel 32 KiB cache
}

impl SdCardDevice {
    pub const fn new(name: &'static str, is_sdxc: bool, total_sectors: u64) -> Self {
        Self {
            name,
            total_sectors,
            block_size: 512,
            is_sdxc,
            storage_buf: [0; 32768],
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

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        if count == 0 {
            return BLOCK_DEV_OK;
        }
        let end_lba = match lba.checked_add(count as u64) {
            Some(e) => e,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if end_lba > self.total_sectors {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        let offset = ((lba as usize) * 512) % self.storage_buf.len();
        let copy_len = bytes.min(buf.len()).min(self.storage_buf.len() - offset);
        buf[..copy_len].copy_from_slice(&self.storage_buf[offset..offset + copy_len]);
        BLOCK_DEV_OK
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32 {
        if count == 0 {
            return BLOCK_DEV_OK;
        }
        let end_lba = match lba.checked_add(count as u64) {
            Some(e) => e,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if end_lba > self.total_sectors {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        let offset = ((lba as usize) * 512) % self.storage_buf.len();
        let copy_len = bytes.min(buf.len()).min(self.storage_buf.len() - offset);
        self.storage_buf[offset..offset + copy_len].copy_from_slice(&buf[..copy_len]);
        BLOCK_DEV_OK
    }
}
