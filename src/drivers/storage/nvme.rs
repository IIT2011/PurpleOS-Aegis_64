use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_BOUNDS, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};
use crate::drivers::dma::alloc_coherent;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct NvmeSqe {
    pub opcode: u8,
    pub flags: u8,
    pub cid: u16,
    pub nsid: u32,
    pub rsv0: [u32; 2],
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct NvmeCqe {
    pub dw0: u32,
    pub rsv: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub cid: u16,
    pub status: u16,
}

#[allow(dead_code)]
pub struct NvmeController {
    bar0: *mut u32,
    admin_sq: *mut NvmeSqe,
    admin_sq_phys: u64,
    admin_cq: *mut NvmeCqe,
    admin_cq_phys: u64,
    io_sq: *mut NvmeSqe,
    io_sq_phys: u64,
    io_cq: *mut NvmeCqe,
    io_cq_phys: u64,
    dma_buf: *mut u8,
    dma_buf_phys: u64,
    admin_sq_tail: u16,
    admin_cq_head: u16,
    admin_cq_phase: u8,
    io_sq_tail: u16,
    io_cq_head: u16,
    io_cq_phase: u8,
    pub total_blocks: u64,
    pub is_ready: bool,
}

unsafe impl Send for NvmeController {}
unsafe impl Sync for NvmeController {}

impl NvmeController {
    pub const fn new() -> Self {
        Self {
            bar0: core::ptr::null_mut(),
            admin_sq: core::ptr::null_mut(),
            admin_sq_phys: 0,
            admin_cq: core::ptr::null_mut(),
            admin_cq_phys: 0,
            io_sq: core::ptr::null_mut(),
            io_sq_phys: 0,
            io_cq: core::ptr::null_mut(),
            io_cq_phys: 0,
            dma_buf: core::ptr::null_mut(),
            dma_buf_phys: 0,
            admin_sq_tail: 0,
            admin_cq_head: 0,
            admin_cq_phase: 1,
            io_sq_tail: 0,
            io_cq_head: 0,
            io_cq_phase: 1,
            total_blocks: 0,
            is_ready: false,
        }
    }

    pub fn init(&mut self, bar0_phys: u64) -> Result<(), &'static str> {
        self.bar0 = bar0_phys as *mut u32;

        unsafe {
            let (asq_ptr, asq_phys) = alloc_coherent(4096).ok_or("Failed to alloc ASQ")?;
            self.admin_sq = asq_ptr as *mut NvmeSqe;
            self.admin_sq_phys = asq_phys;

            let (acq_ptr, acq_phys) = alloc_coherent(4096).ok_or("Failed to alloc ACQ")?;
            self.admin_cq = acq_ptr as *mut NvmeCqe;
            self.admin_cq_phys = acq_phys;

            let (iosq_ptr, iosq_phys) = alloc_coherent(4096).ok_or("Failed to alloc IOSQ")?;
            self.io_sq = iosq_ptr as *mut NvmeSqe;
            self.io_sq_phys = iosq_phys;

            let (iocq_ptr, iocq_phys) = alloc_coherent(4096).ok_or("Failed to alloc IOCQ")?;
            self.io_cq = iocq_ptr as *mut NvmeCqe;
            self.io_cq_phys = iocq_phys;

            let (db_ptr, db_phys) = alloc_coherent(65536).ok_or("Failed to alloc NVMe buffer")?;
            self.dma_buf = db_ptr;
            self.dma_buf_phys = db_phys;

            self.total_blocks = 2097152; // Default 1 GiB NVMe device
            self.is_ready = true;
        }
        Ok(())
    }
}

impl BlockDevice for NvmeController {
    fn name(&self) -> &str {
        "nvme0n1"
    }

    fn block_size(&self) -> u32 {
        512
    }

    fn total_blocks(&self) -> u64 {
        self.total_blocks
    }

    fn category(&self) -> StorageCategory {
        StorageCategory::Ssd
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        if count == 0 {
            return BLOCK_DEV_OK;
        }
        if lba + count as u64 > self.total_blocks || !self.is_ready {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        if bytes > 65536 || buf.len() < bytes {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        unsafe {
            let tail = self.io_sq_tail as usize;
            let sqe = &mut *self.io_sq.add(tail);
            *sqe = NvmeSqe::default();
            sqe.opcode = 0x02; // NVME_NVM_CMD_READ
            sqe.nsid = 1;
            sqe.prp1 = self.dma_buf_phys;
            sqe.cdw10 = (lba & 0xFFFF_FFFF) as u32;
            sqe.cdw11 = (lba >> 32) as u32;
            sqe.cdw12 = count - 1;

            self.io_sq_tail = (self.io_sq_tail + 1) % 64;

            // Doorbell write
            let db = self.bar0.add(0x1000 / 4 + 2 * (4 / 4));
            core::ptr::write_volatile(db, self.io_sq_tail as u32);

            // Bounded poll on completion
            let mut timeout = 1000000;
            while timeout > 0 {
                let cqe = &*self.io_cq.add(self.io_cq_head as usize);
                let phase = (cqe.status & 1) as u8;
                if phase == self.io_cq_phase {
                    self.io_cq_head = (self.io_cq_head + 1) % 64;
                    if self.io_cq_head == 0 {
                        self.io_cq_phase ^= 1;
                    }
                    core::ptr::copy_nonoverlapping(self.dma_buf, buf.as_mut_ptr(), bytes);
                    return BLOCK_DEV_OK;
                }
                core::hint::spin_loop();
                timeout -= 1;
            }
        }
        BLOCK_DEV_ERR_IO
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32 {
        if count == 0 {
            return BLOCK_DEV_OK;
        }
        if lba + count as u64 > self.total_blocks || !self.is_ready {
            return BLOCK_DEV_ERR_BOUNDS;
        }
        let bytes = (count as usize) * 512;
        if bytes > 65536 || buf.len() < bytes {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), self.dma_buf, bytes);

            let tail = self.io_sq_tail as usize;
            let sqe = &mut *self.io_sq.add(tail);
            *sqe = NvmeSqe::default();
            sqe.opcode = 0x01; // NVME_NVM_CMD_WRITE
            sqe.nsid = 1;
            sqe.prp1 = self.dma_buf_phys;
            sqe.cdw10 = (lba & 0xFFFF_FFFF) as u32;
            sqe.cdw11 = (lba >> 32) as u32;
            sqe.cdw12 = count - 1;

            self.io_sq_tail = (self.io_sq_tail + 1) % 64;

            let db = self.bar0.add(0x1000 / 4 + 2 * (4 / 4));
            core::ptr::write_volatile(db, self.io_sq_tail as u32);

            let mut timeout = 1000000;
            while timeout > 0 {
                let cqe = &*self.io_cq.add(self.io_cq_head as usize);
                let phase = (cqe.status & 1) as u8;
                if phase == self.io_cq_phase {
                    self.io_cq_head = (self.io_cq_head + 1) % 64;
                    if self.io_cq_head == 0 {
                        self.io_cq_phase ^= 1;
                    }
                    return BLOCK_DEV_OK;
                }
                core::hint::spin_loop();
                timeout -= 1;
            }
        }
        BLOCK_DEV_ERR_IO
    }
}
