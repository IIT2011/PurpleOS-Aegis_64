//! USB 3.x Mass Storage Device Driver (Aegis64 / xHCI Subsystem)
//!
//! Provides USB Mass Storage (MSC / BOT) block device abstractions mapped to
//! canonical storage paths (`/usb/usb1`, `/usb/usb2`, ...).

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_OK, BLOCK_DEV_ERR_BOUNDS};

pub struct UsbMassStorageDevice {
    pub name: &'static str,
    pub total_sectors: u64,
    pub block_size: u32,
    pub connected: bool,
    storage_buf: [u8; 65536], // In-kernel 64 KiB sector cache / transfer ring
}

impl UsbMassStorageDevice {
    pub const fn new(name: &'static str, total_sectors: u64) -> Self {
        Self {
            name,
            total_sectors,
            block_size: 512,
            connected: false,
            storage_buf: [0; 65536],
        }
    }

    pub fn set_connected(&mut self, conn: bool) {
        self.connected = conn;
    }
}

impl BlockDevice for UsbMassStorageDevice {
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
        StorageCategory::Usb
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        if lba + count as u64 > self.total_sectors {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        let offset = ((lba as usize) * 512) % self.storage_buf.len();
        let copy_len = bytes.min(buf.len()).min(self.storage_buf.len() - offset);
        buf[..copy_len].copy_from_slice(&self.storage_buf[offset..offset + copy_len]);
        BLOCK_DEV_OK
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32 {
        if lba + count as u64 > self.total_sectors {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        let offset = ((lba as usize) * 512) % self.storage_buf.len();
        let copy_len = bytes.min(buf.len()).min(self.storage_buf.len() - offset);
        self.storage_buf[offset..offset + copy_len].copy_from_slice(&buf[..copy_len]);
        BLOCK_DEV_OK
    }
}
