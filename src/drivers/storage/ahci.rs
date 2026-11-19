//! AHCI block device interface. The controller and port probing sequence is
//! intentionally unavailable until IDENTIFY data and command completion are
//! validated; this module must never advertise fabricated media.

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};

pub struct AhciPort {
    name: [u8; 8],
}

unsafe impl Send for AhciPort {}
unsafe impl Sync for AhciPort {}

impl AhciPort {
    pub const fn new(port_num: u8) -> Self {
        let mut name = [0u8; 8];
        name[0] = b's';
        name[1] = b'a';
        name[2] = b't';
        name[3] = b'a';
        name[4] = b'0' + port_num;
        Self { name }
    }

    pub fn init(&mut self, _regs: *mut u8) -> Result<(), &'static str> {
        Err("AHCI port discovery and IDENTIFY capacity probing are not implemented")
    }
}

impl BlockDevice for AhciPort {
    fn name(&self) -> &str {
        let len = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..len]).unwrap_or("sata0")
    }

    fn block_size(&self) -> u32 { 512 }
    fn total_blocks(&self) -> u64 { 0 }
    fn category(&self) -> StorageCategory { StorageCategory::Ssd }

    fn read_blocks(&mut self, _lba: u64, count: u32, _buf: &mut [u8]) -> i32 {
        if count == 0 { BLOCK_DEV_OK } else { BLOCK_DEV_ERR_IO }
    }

    fn write_blocks(&mut self, _lba: u64, count: u32, _buf: &[u8]) -> i32 {
        if count == 0 { BLOCK_DEV_OK } else { BLOCK_DEV_ERR_IO }
    }
}
