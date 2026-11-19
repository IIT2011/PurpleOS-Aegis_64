use crate::drivers::block_dev::{BlockDevice, BLOCK_DEV_ERR_BOUNDS};

pub struct PartitionDevice<'a> {
    parent: &'a mut dyn BlockDevice,
    start_lba: u64,
    sector_count: u64,
}

impl<'a> PartitionDevice<'a> {
    pub fn new(parent: &'a mut dyn BlockDevice, start_lba: u64, sector_count: u64) -> Self {
        Self {
            parent,
            start_lba,
            sector_count,
        }
    }
}

impl<'a> BlockDevice for PartitionDevice<'a> {
    fn name(&self) -> &str {
        "part"
    }

    fn block_size(&self) -> u32 {
        self.parent.block_size()
    }

    fn total_blocks(&self) -> u64 {
        self.sector_count
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        let end = match lba.checked_add(count as u64) {
            Some(e) => e,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if end > self.sector_count {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let absolute_start = match self.start_lba.checked_add(lba) {
            Some(value) => value,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        let absolute_end = match self.start_lba.checked_add(end) {
            Some(value) => value,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if absolute_end > self.parent.total_blocks() {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        self.parent.read_blocks(absolute_start, count, buf)
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32 {
        let end = match lba.checked_add(count as u64) {
            Some(e) => e,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if end > self.sector_count {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let absolute_start = match self.start_lba.checked_add(lba) {
            Some(value) => value,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        let absolute_end = match self.start_lba.checked_add(end) {
            Some(value) => value,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if absolute_end > self.parent.total_blocks() {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        self.parent.write_blocks(absolute_start, count, buf)
    }

    fn flush(&mut self) -> i32 {
        self.parent.flush()
    }
}
