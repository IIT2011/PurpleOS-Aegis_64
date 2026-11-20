//! Polled AHCI SATA DMA using a private bounce buffer per active port.

use super::{map_bar, read_reg, transfer_len, wait_for, write_addr, write_reg, DMA_PAGE};
use crate::drivers::{
    block_dev::{self, BlockDevice, StorageCategory, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK},
    dma, pci,
};
use alloc::boxed::Box;
use core::{
    ptr,
    sync::atomic::{fence, Ordering},
};

const HBA_CAP: usize = 0;
const HBA_GHC: usize = 4;
const HBA_PI: usize = 0x0c;
const HBA_CAP2: usize = 0x24;
const HBA_BOHC: usize = 0x28;
const AHCI_ENABLE: u32 = 1 << 31;
const BIOS_OWNED_OR_BUSY: u32 = 1 | (1 << 4);
const OS_OWNED: u32 = 1 << 1;
const PORT_BASE: usize = 0x100;
const PORT_STRIDE: usize = 0x80;

const CLB: usize = 0;
const FB: usize = 8;
const IS: usize = 0x10;
const IE: usize = 0x14;
const CMD: usize = 0x18;
const TFD: usize = 0x20;
const SIG: usize = 0x24;
const SSTS: usize = 0x28;
const SERR: usize = 0x30;
const SACT: usize = 0x34;
const CI: usize = 0x38;
const START: u32 = 1;
const FIS_ENABLE: u32 = 1 << 4;
const FIS_RUNNING: u32 = 1 << 14;
const LIST_RUNNING: u32 = 1 << 15;
const PORT_ERRORS: u32 = 0x7d << 24;
const ATA_BUSY_OR_DRQ: u32 = 0x88;
const ATA_ERROR_OR_FAULT: u32 = 0x21;
const SATA_DISK: u32 = 0x0000_0101;
const SATA_LINK_ACTIVE: u32 = 0x103;

const FIS_OFFSET: usize = 1024;
const TABLE_OFFSET: usize = 1280;
const PRDT_OFFSET: usize = 128;
const IDENTIFY: u8 = 0xec;
const READ_DMA_EXT: u8 = 0x25;
const WRITE_DMA_EXT: u8 = 0x35;
const FLUSH_CACHE_EXT: u8 = 0xea;

struct AhciPort {
    name: alloc::string::String,
    regs: usize,
    dma: usize,
    blocks: u64,
    sector: usize,
    healthy: bool,
    solid_state: bool,
}

pub fn probe(device: &pci::PciDevice) -> Result<(), &'static str> {
    let hba = map_bar(device.bar5, 0, PORT_BASE)?;
    pci::enable_bus_mastering(device.bus, device.slot, device.func);
    if read_reg(hba, HBA_CAP2) & 1 != 0 {
        write_reg(hba, HBA_BOHC, read_reg(hba, HBA_BOHC) | OS_OWNED);
        if !wait_for(|| read_reg(hba, HBA_BOHC) & BIOS_OWNED_OR_BUSY == 0) {
            return Err("AHCI BIOS handoff timed out");
        }
    }
    write_reg(hba, HBA_GHC, AHCI_ENABLE);
    let implemented = read_reg(hba, HBA_PI);
    let port_count = (read_reg(hba, HBA_CAP) & 0x1f) as usize + 1;
    map_bar(device.bar5, 0, PORT_BASE + port_count * PORT_STRIDE)?;
    for port in 0..port_count {
        if !block_dev::has_capacity() {
            break;
        }
        if implemented & (1 << port) == 0 {
            continue;
        }
        let regs = hba + PORT_BASE + port * PORT_STRIDE;
        if read_reg(regs, SSTS) & 0xf0f != SATA_LINK_ACTIVE || read_reg(regs, SIG) != SATA_DISK {
            continue;
        }
        let name = alloc::format!(
            "sata-{:02x}:{:02x}.{}-{}",
            device.bus,
            device.slot,
            device.func,
            port
        );
        match AhciPort::init(name, regs) {
            Ok(disk) => block_dev::register(Box::leak(Box::new(disk))),
            Err(err) => crate::drivers::serial::println(err),
        }
    }
    Ok(())
}

impl AhciPort {
    fn init(name: alloc::string::String, regs: usize) -> Result<Self, &'static str> {
        write_reg(regs, IE, 0);
        write_reg(regs, CMD, read_reg(regs, CMD) & !START);
        if !wait_for(|| read_reg(regs, CMD) & LIST_RUNNING == 0) {
            return Err("AHCI command engine failed to stop");
        }
        write_reg(regs, CMD, read_reg(regs, CMD) & !FIS_ENABLE);
        if !wait_for(|| read_reg(regs, CMD) & FIS_RUNNING == 0)
            || read_reg(regs, CI) != 0
            || read_reg(regs, SACT) != 0
        {
            return Err("AHCI port did not become idle");
        }
        let (_, address) =
            unsafe { dma::alloc_coherent(2 * DMA_PAGE) }.ok_or("AHCI DMA allocation failed")?;
        let dma = address as usize;
        write_addr(regs, CLB, dma);
        write_addr(regs, FB, dma + FIS_OFFSET);
        write_reg(regs, SERR, u32::MAX);
        write_reg(regs, IS, u32::MAX);
        write_reg(regs, CMD, read_reg(regs, CMD) | FIS_ENABLE);
        write_reg(regs, CMD, read_reg(regs, CMD) | START);

        let mut port = Self {
            name,
            regs,
            dma,
            blocks: 0,
            sector: 512,
            healthy: true,
            solid_state: false,
        };
        let identified = port.command(IDENTIFY, 0, 0, 512, false) && port.identify();
        if !identified {
            // Reclaim DMA only after both engines confirm they have stopped.
            write_reg(regs, CMD, read_reg(regs, CMD) & !START);
            if wait_for(|| read_reg(regs, CMD) & LIST_RUNNING == 0) {
                write_reg(regs, CMD, read_reg(regs, CMD) & !FIS_ENABLE);
                if wait_for(|| read_reg(regs, CMD) & FIS_RUNNING == 0) {
                    unsafe { dma::free_coherent(dma as *mut u8, 2 * DMA_PAGE) };
                }
            }
            return Err("AHCI SATA IDENTIFY failed or returned unsupported geometry");
        }
        Ok(port)
    }

    fn identify(&mut self) -> bool {
        let word = |index: usize| unsafe {
            ptr::read_volatile((self.dma + DMA_PAGE + index * 2) as *const u16)
        };
        let capabilities = word(83);
        if capabilities & 0xc400 != 0x4400 || word(49) & (1 << 8) == 0 {
            return false;
        }
        self.blocks = u64::from(word(100))
            | (u64::from(word(101)) << 16)
            | (u64::from(word(102)) << 32)
            | (u64::from(word(103)) << 48);
        let format = word(106);
        if format & 0xd000 == 0x5000 {
            let words = u32::from(word(117)) | (u32::from(word(118)) << 16);
            self.sector = (words as usize) * 2;
        }
        self.solid_state = word(217) == 1;
        self.blocks > 0
            && self.blocks <= (1 << 48)
            && (512..=DMA_PAGE).contains(&self.sector)
            && self.sector.is_power_of_two()
    }

    fn command(&mut self, opcode: u8, lba: u64, count: u16, bytes: usize, write: bool) -> bool {
        let regs = self.regs;
        if !self.healthy {
            return false;
        }
        if !wait_for(|| read_reg(regs, TFD) & ATA_BUSY_OR_DRQ == 0)
            || read_reg(regs, CI) != 0
            || read_reg(regs, SACT) != 0
        {
            self.healthy = false;
            return false;
        }
        let header = self.dma;
        let table = self.dma + TABLE_OFFSET;
        let flags = 5 | if write { 1 << 6 } else { 0 };
        write_reg(header, 0, flags | (u32::from(bytes != 0) << 16));
        write_reg(header, 4, 0);
        write_addr(header, 8, table);
        // Register Host-to-Device FIS, ATA command, 48-bit LBA, sector count.
        write_reg(table, 0, 0x8027 | (u32::from(opcode) << 16));
        write_reg(table, 4, (lba as u32 & 0xff_ffff) | (1 << 30));
        write_reg(table, 8, (lba >> 24) as u32);
        write_reg(table, 12, u32::from(count));
        write_reg(table, 16, 0);
        if bytes != 0 {
            write_addr(table, PRDT_OFFSET, self.dma + DMA_PAGE);
            write_reg(table, PRDT_OFFSET + 8, 0);
            write_reg(table, PRDT_OFFSET + 12, bytes as u32 - 1);
        }
        write_reg(regs, IS, u32::MAX);
        fence(Ordering::SeqCst);
        write_reg(regs, CI, 1);
        let completed =
            wait_for(|| read_reg(regs, CI) & 1 == 0 || read_reg(regs, IS) & PORT_ERRORS != 0);
        fence(Ordering::Acquire);
        if !completed
            || read_reg(regs, CI) & 1 != 0
            || read_reg(regs, IS) & PORT_ERRORS != 0
            || read_reg(regs, TFD) & ATA_ERROR_OR_FAULT != 0
            || read_reg(header, 4) != bytes as u32
        {
            self.healthy = false;
            return false;
        }
        true
    }
}

impl BlockDevice for AhciPort {
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
        if self.solid_state {
            StorageCategory::Ssd
        } else {
            StorageCategory::Hdd
        }
    }

    fn read_blocks(&mut self, mut lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        let len = match transfer_len(lba, count, self.blocks, self.sector, buf.len()) {
            Ok(len) => len,
            Err(err) => return err,
        };
        let chunk = DMA_PAGE / self.sector * self.sector;
        for data in buf[..len].chunks_mut(chunk) {
            let blocks = data.len() / self.sector;
            if !self.command(READ_DMA_EXT, lba, blocks as u16, data.len(), false) {
                return BLOCK_DEV_ERR_IO;
            }
            unsafe {
                ptr::copy_nonoverlapping(
                    (self.dma + DMA_PAGE) as *const u8,
                    data.as_mut_ptr(),
                    data.len(),
                )
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
        let chunk = DMA_PAGE / self.sector * self.sector;
        for data in buf[..len].chunks(chunk) {
            let blocks = data.len() / self.sector;
            if !self.healthy {
                return BLOCK_DEV_ERR_IO;
            }
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    (self.dma + DMA_PAGE) as *mut u8,
                    data.len(),
                )
            };
            if !self.command(WRITE_DMA_EXT, lba, blocks as u16, data.len(), true) {
                return BLOCK_DEV_ERR_IO;
            }
            lba += blocks as u64;
        }
        BLOCK_DEV_OK
    }

    fn flush(&mut self) -> i32 {
        if self.command(FLUSH_CACHE_EXT, 0, 0, 0, false) {
            BLOCK_DEV_OK
        } else {
            BLOCK_DEV_ERR_IO
        }
    }
}
