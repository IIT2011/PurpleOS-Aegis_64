//! USB 3.x Mass Storage Device Driver (Aegis64 / xHCI Subsystem)
//!
//! Provides USB Mass Storage (MSC / BOT) block device abstractions mapped to
//! canonical storage paths (`/usb/usb1`, `/usb/usb2`, ...).

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};

pub struct UsbMassStorageDevice {
    pub name: &'static str,
    pub total_sectors: u64,
    pub block_size: u32,
    pub connected: bool,
}

impl UsbMassStorageDevice {
    pub const fn new(name: &'static str, total_sectors: u64) -> Self {
        Self {
            name,
            total_sectors,
            block_size: 512,
            connected: false,
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
