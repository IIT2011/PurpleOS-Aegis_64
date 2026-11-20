//! Polled NVMe admin and I/O queues with one outstanding command per controller.

use super::{
    map_bar, read_reg, transfer_len, wait_for, wait_for_ms, write_addr, write_reg, DMA_PAGE,
};
use crate::drivers::{
    block_dev::{self, BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK},
    dma, pci,
};
use alloc::{boxed::Box, string::String};
use core::{
    ptr,
    sync::atomic::{fence, Ordering},
};

const CAP: usize = 0;
const INTMS: usize = 0x0c;
const CC: usize = 0x14;
const CSTS: usize = 0x1c;
const AQA: usize = 0x24;
const ASQ: usize = 0x28;
const ACQ: usize = 0x30;
const DOORBELLS: usize = 0x1000;
const ENABLE: u32 = 1;
const CAP_TIMEOUT_UNIT_MS: u64 = 500;
const READY: u32 = 1;
const FATAL: u32 = 2;
const SQ_ENTRY_SHIFT: u32 = 6;
const CQ_ENTRY_SHIFT: u32 = 4;
const DMA_BYTES: usize = 5 * DMA_PAGE;
const ADMIN_SET_FEATURES: u32 = 9;
const NUMBER_OF_QUEUES: u32 = 7;
const ADMIN_IDENTIFY: u32 = 6;
const ADMIN_CREATE_CQ: u32 = 5;
const ADMIN_CREATE_SQ: u32 = 1;
const IO_FLUSH: u32 = 0;
const IO_WRITE: u32 = 1;
const IO_READ: u32 = 2;

struct Queue {
    sq: usize,
    cq: usize,
    id: u16,
    depth: u16,
    tail: u16,
    head: u16,
    cid: u16,
    phase: bool,
}

impl Queue {
    fn new(sq: usize, id: u16, depth: u16) -> Self {
        Self {
            sq,
            cq: sq + DMA_PAGE,
            id,
            depth,
            tail: 0,
            head: 0,
            cid: 0,
            phase: true,
        }
    }

    fn submit(&mut self, regs: usize, stride: usize, mut command: [u32; 16]) -> Result<(), bool> {
        let cid = self.cid;
        self.cid = self.cid.wrapping_add(1);
        command[0] |= u32::from(cid) << 16;
        let entry = self.sq + ((self.tail as usize) << SQ_ENTRY_SHIFT);
        for (index, word) in command.into_iter().enumerate() {
            write_reg(entry, index * 4, word);
        }
        self.tail = (self.tail + 1) % self.depth;
        let sq_doorbell = DOORBELLS + 2 * usize::from(self.id) * stride;
        let cq_doorbell = sq_doorbell + stride;
        fence(Ordering::SeqCst);
        write_reg(regs, sq_doorbell, u32::from(self.tail));

        let completion = self.cq + ((self.head as usize) << CQ_ENTRY_SHIFT);
        if !wait_for(|| {
            (read_reg(completion, 12) >> 16 & 1 != 0) == self.phase
                || read_reg(regs, CSTS) & FATAL != 0
        }) || read_reg(regs, CSTS) & FATAL != 0
        {
            return Err(true);
        }
        fence(Ordering::Acquire);
        let status = read_reg(completion, 12);
        let head_and_id = read_reg(completion, 8);
        if status as u16 != cid
            || (head_and_id >> 16) as u16 != self.id
            || head_and_id as u16 != self.tail
        {
            return Err(true);
        }
        self.head += 1;
        if self.head == self.depth {
            self.head = 0;
            self.phase = !self.phase;
        }
        write_reg(regs, cq_doorbell, u32::from(self.head));
        if status & 0xfffe_0000 != 0 {
            Err(false)
        } else {
            Ok(())
        }
    }
}

struct NvmeController {
    name: String,
    regs: usize,
    dma: usize,
    stride: usize,
    nsid: u32,
    blocks: u64,
    sector: usize,
    io: Queue,
    healthy: bool,
}

pub fn probe(device: &pci::PciDevice) -> Result<(), &'static str> {
    if !block_dev::has_capacity() {
        return Err("No block-device registry slots available for NVMe");
    }
    let regs = map_bar(device.bar0, device.bar1, DOORBELLS)?;
    pci::enable_bus_mastering(device.bus, device.slot, device.func);
    let cap_low = read_reg(regs, CAP);
    let ready_timeout_ms = u64::from((cap_low >> 24).max(1)) * CAP_TIMEOUT_UNIT_MS;
    let cap_high = read_reg(regs, CAP + 4);
    if cap_high & (1 << 5) == 0 || (cap_high >> 16) & 0xf != 0 {
        return Err("NVMe controller does not support NVM I/O with 4 KiB pages");
    }
    let stride = 4 << (cap_high & 0xf);
    map_bar(device.bar0, device.bar1, DOORBELLS + 4 * stride)?;
    let max_entries = (cap_low & 0xffff) + 1;
    let depth = max_entries.min((DMA_PAGE >> SQ_ENTRY_SHIFT) as u32) as u16;
    if depth < 2 {
        return Err("NVMe controller queue depth is too small");
    }
    write_reg(regs, INTMS, u32::MAX);
    write_reg(regs, CC, read_reg(regs, CC) & !ENABLE);
    if !wait_for_ms(ready_timeout_ms, || read_reg(regs, CSTS) & READY == 0) {
        return Err("NVMe controller did not disable");
    }
    let (_, address) =
        unsafe { dma::alloc_coherent(DMA_BYTES) }.ok_or("NVMe DMA allocation failed")?;
    let dma = address as usize;
    let mut controller = NvmeController {
        name: String::new(),
        regs,
        dma,
        stride,
        nsid: 0,
        blocks: 0,
        sector: 0,
        io: Queue::new(dma + 2 * DMA_PAGE, 1, depth),
        healthy: true,
    };
    if let Err(err) = controller.init(depth, ready_timeout_ms) {
        write_reg(regs, CC, read_reg(regs, CC) & !ENABLE);
        // A controller that refuses to disable may still access its DMA pages.
        if wait_for_ms(ready_timeout_ms, || read_reg(regs, CSTS) & READY == 0) {
            unsafe { dma::free_coherent(dma as *mut u8, DMA_BYTES) };
        }
        return Err(err);
    }
    controller.name = alloc::format!(
        "nvme-{:02x}:{:02x}.{}n{}",
        device.bus,
        device.slot,
        device.func,
        controller.nsid
    );
    block_dev::register(Box::leak(Box::new(controller)));
    Ok(())
}

fn command(
    opcode: u32,
    namespace: u32,
    prp: usize,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
) -> [u32; 16] {
    let mut entry = [0; 16];
    entry[0] = opcode;
    entry[1] = namespace;
    entry[6] = prp as u32;
    entry[7] = (prp as u64 >> 32) as u32;
    entry[10] = cdw10;
    entry[11] = cdw11;
    entry[12] = cdw12;
    entry
}

impl NvmeController {
    fn init(&mut self, depth: u16, ready_timeout_ms: u64) -> Result<(), &'static str> {
        let regs = self.regs;
        let mut admin = Queue::new(self.dma, 0, depth);
        let queue_size = u32::from(depth - 1);
        write_reg(regs, AQA, queue_size | (queue_size << 16));
        write_addr(regs, ASQ, admin.sq);
        write_addr(regs, ACQ, admin.cq);
        write_reg(
            regs,
            CC,
            (CQ_ENTRY_SHIFT << 20) | (SQ_ENTRY_SHIFT << 16) | ENABLE,
        );
        if !wait_for_ms(ready_timeout_ms, || {
            read_reg(regs, CSTS) & (READY | FATAL) != 0
        }) || read_reg(regs, CSTS) & (READY | FATAL) != READY
        {
            return Err("NVMe controller failed to become ready");
        }
        let prp = self.buffer();
        let mut submit = |entry| {
            admin
                .submit(regs, self.stride, entry)
                .map_err(|_| "NVMe admin command failed")
        };
        // Identify the first active namespace, then retrieve its formatted geometry.
        submit(command(ADMIN_IDENTIFY, 0, prp, 2, 0, 0))?;
        self.nsid = read_reg(prp, 0);
        if self.nsid == 0 || self.nsid == u32::MAX {
            return Err("NVMe controller has no active namespace");
        }
        submit(command(ADMIN_IDENTIFY, self.nsid, prp, 0, 0, 0))?;
        self.blocks = u64::from(read_reg(prp, 0)) | (u64::from(read_reg(prp, 4)) << 32);
        let geometry = read_reg(prp, 24);
        let format_count = (geometry >> 8) & 0xff;
        let flbas = (geometry >> 16) & 0xff;
        let format = (flbas & 0xf) | ((flbas & 0x60) >> 1);
        let protection = (read_reg(prp, 28) >> 8) & 7;
        if format > format_count || format >= 64 || protection != 0 {
            return Err("NVMe namespace format is unsupported");
        }
        let lbaf = read_reg(prp, 128 + format as usize * 4);
        let sector_shift = (lbaf >> 16) & 0xff;
        if lbaf & 0xffff != 0
            || !(9..=DMA_PAGE.trailing_zeros()).contains(&sector_shift)
            || self.blocks == 0
        {
            return Err("NVMe namespace has unsupported logical blocks");
        }
        self.sector = 1 << sector_shift;
        submit(command(ADMIN_SET_FEATURES, 0, 0, NUMBER_OF_QUEUES, 0, 0))?;
        let queue = (queue_size << 16) | u32::from(self.io.id);
        submit(command(ADMIN_CREATE_CQ, 0, self.io.cq, queue, 1, 0))?;
        submit(command(
            ADMIN_CREATE_SQ,
            0,
            self.io.sq,
            queue,
            (u32::from(self.io.id) << 16) | 1,
            0,
        ))?;
        Ok(())
    }

    fn buffer(&self) -> usize {
        self.dma + 4 * DMA_PAGE
    }

    fn submit(&mut self, opcode: u32, lba: u64, blocks: usize) -> i32 {
        if !self.healthy {
            return BLOCK_DEV_ERR_IO;
        }
        let prp = if blocks == 0 { 0 } else { self.buffer() };
        let entry = command(
            opcode,
            self.nsid,
            prp,
            lba as u32,
            (lba >> 32) as u32,
            blocks.saturating_sub(1) as u32,
        );
        match self.io.submit(self.regs, self.stride, entry) {
            Ok(()) => BLOCK_DEV_OK,
            Err(fatal) => {
                self.healthy = !fatal;
                BLOCK_DEV_ERR_IO
            }
        }
    }
}

impl BlockDevice for NvmeController {
    fn name(&self) -> &str {
        &self.name
    }
    fn block_size(&self) -> u32 {
        self.sector as u32
    }
    fn total_blocks(&self) -> u64 {
        self.blocks
    }
    fn category(&self) -> StorageCategory {
        StorageCategory::Ssd
    }

    fn read_blocks(&mut self, mut lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        let len = match transfer_len(lba, count, self.blocks, self.sector, buf.len()) {
            Ok(len) => len,
            Err(err) => return err,
        };
        for data in buf[..len].chunks_mut(DMA_PAGE) {
            let blocks = data.len() / self.sector;
            if self.submit(IO_READ, lba, blocks) != BLOCK_DEV_OK {
                return BLOCK_DEV_ERR_IO;
            }
            unsafe {
                ptr::copy_nonoverlapping(self.buffer() as *const u8, data.as_mut_ptr(), data.len())
            };
            lba += blocks as u64;
        }
        BLOCK_DEV_OK
    }

    fn write_blocks(&mut self, mut lba: u64, count: u32, buf: &[u8]) -> i32 {
        let len = match transfer_len(lba, count, self.blocks, self.sector, buf.len()) {
            Ok(len) => len,
            Err(err) => return err,
        };
        for data in buf[..len].chunks(DMA_PAGE) {
            let blocks = data.len() / self.sector;
            if !self.healthy {
                return BLOCK_DEV_ERR_IO;
            }
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr(), self.buffer() as *mut u8, data.len())
            };
            if self.submit(IO_WRITE, lba, blocks) != BLOCK_DEV_OK {
                return BLOCK_DEV_ERR_IO;
            }
            lba += blocks as u64;
        }
        BLOCK_DEV_OK
    }

    fn flush(&mut self) -> i32 {
        self.submit(IO_FLUSH, 0, 0)
    }
}
