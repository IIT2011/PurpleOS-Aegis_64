//! NVMe block device interface. Namespace discovery, controller setup,
//! queue initialization, and completion validation are not implemented yet.
//! Fail closed rather than advertising synthetic capacity or completing I/O.

use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};

pub struct NvmeController;

impl NvmeController {
    pub const fn new() -> Self { Self }

    pub fn init(&mut self, _bar0_phys: u64) -> Result<(), &'static str> {
        Err("NVMe queue and namespace initialization is not implemented")
    }
}

impl BlockDevice for NvmeController {
    fn name(&self) -> &str { "nvme0n1" }
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
