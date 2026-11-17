use crate::arch::x86_64::io::{inb, inw, outb, outw};
use crate::drivers::block_dev::{BlockDevice, StorageCategory, BLOCK_DEV_ERR_BOUNDS, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};

pub const ATA_PRIMARY_IO: u16 = 0x1F0;
pub const ATA_PRIMARY_CTRL: u16 = 0x3F6;
pub const ATA_SECONDARY_IO: u16 = 0x170;
pub const ATA_SECONDARY_CTRL: u16 = 0x376;

pub struct AtaDevice {
    pub name: [u8; 8],
    pub io_base: u16,
    pub ctrl_base: u16,
    pub is_slave: bool,
    pub total_sectors: u64,
    pub model: [u8; 40],
    pub is_present: bool,
}

impl AtaDevice {
    pub const fn new(name_str: &[u8], io_base: u16, ctrl_base: u16, is_slave: bool) -> Self {
        let mut name = [0u8; 8];
        let mut i = 0;
        while i < name_str.len() && i < 8 {
            name[i] = name_str[i];
            i += 1;
        }
        Self {
            name,
            io_base,
            ctrl_base,
            is_slave,
            total_sectors: 0,
            model: [0u8; 40],
            is_present: false,
        }
    }

    unsafe fn wait_not_busy(&self) -> bool {
        for _ in 0..100000 {
            let status = inb(self.ctrl_base);
            if (status & 0x80) == 0 {
                return true;
            }
        }
        for _ in 0..100 {
            crate::drivers::pit::delay_ms(1);
            let status = inb(self.ctrl_base);
            if (status & 0x80) == 0 {
                return true;
            }
        }
        false
    }

    unsafe fn wait_drq(&self) -> bool {
        for _ in 0..100000 {
            let status = inb(self.ctrl_base);
            if (status & 0x80) == 0 {
                if (status & 0x08) != 0 {
                    return true;
                }
                if (status & 0x01) != 0 { // ERR
                    return false;
                }
            }
        }
        for _ in 0..100 {
            crate::drivers::pit::delay_ms(1);
            let status = inb(self.ctrl_base);
            if (status & 0x80) == 0 {
                if (status & 0x08) != 0 {
                    return true;
                }
                if (status & 0x01) != 0 { // ERR
                    return false;
                }
            }
        }
        false
    }

    pub fn identify(&mut self) -> Result<(), &'static str> {
        unsafe {
            let drive_sel: u8 = if self.is_slave { 0xB0 } else { 0xA0 };
            outb(self.io_base + 6, drive_sel);

            outb(self.io_base + 2, 0);
            outb(self.io_base + 3, 0);
            outb(self.io_base + 4, 0);
            outb(self.io_base + 5, 0);

            outb(self.io_base + 7, 0xEC); // IDENTIFY command

            let status = inb(self.io_base + 7);
            if status == 0 || status == 0xFF {
                return Err("Device not present");
            }

            if !self.wait_not_busy() {
                return Err("Device stuck busy");
            }

            let mid = inb(self.io_base + 4);
            let hi = inb(self.io_base + 5);
            if mid != 0 || hi != 0 {
                return Err("Non-ATA device (ATAPI/SATA)");
            }

            if !self.wait_drq() {
                return Err("Device did not assert DRQ");
            }

            let mut id_data = [0u16; 256];
            for i in 0..256 {
                id_data[i] = inw(self.io_base);
            }

            // Read model (words 27..46)
            for i in 0..20 {
                let w = id_data[27 + i];
                self.model[i * 2] = (w >> 8) as u8;
                self.model[i * 2 + 1] = (w & 0xFF) as u8;
            }

            // Read sectors
            let lba48_supp = (id_data[83] & (1 << 10)) != 0;
            if lba48_supp {
                let s0 = id_data[100] as u64;
                let s1 = id_data[101] as u64;
                let s2 = id_data[102] as u64;
                let s3 = id_data[103] as u64;
                self.total_sectors = s0 | (s1 << 16) | (s2 << 32) | (s3 << 48);
            } else {
                let s0 = id_data[60] as u64;
                let s1 = id_data[61] as u64;
                self.total_sectors = s0 | (s1 << 16);
            }

            self.is_present = true;
            Ok(())
        }
    }
}

impl BlockDevice for AtaDevice {
    fn name(&self) -> &str {
        core::str::from_utf8(&self.name).unwrap_or("ata")
    }

    fn block_size(&self) -> u32 {
        512
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn category(&self) -> StorageCategory {
        StorageCategory::Hdd
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
        let required_bytes = match (count as usize).checked_mul(512) {
            Some(b) => b,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if buf.len() < required_bytes {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        unsafe {
            for c in 0..count {
                let cur_lba = lba + c as u64;
                if !self.wait_not_busy() {
                    return BLOCK_DEV_ERR_IO;
                }

                // LBA28 command
                outb(self.io_base + 6, 0xE0 | ((self.is_slave as u8) << 4) | (((cur_lba >> 24) & 0x0F) as u8));
                for _ in 0..4 { let _ = inb(self.ctrl_base); }
                outb(self.io_base + 2, 1);
                outb(self.io_base + 3, (cur_lba & 0xFF) as u8);
                outb(self.io_base + 4, ((cur_lba >> 8) & 0xFF) as u8);
                outb(self.io_base + 5, ((cur_lba >> 16) & 0xFF) as u8);
                outb(self.io_base + 7, 0x20); // READ SECTORS
                for _ in 0..4 { let _ = inb(self.ctrl_base); }

                if !self.wait_drq() {
                    return BLOCK_DEV_ERR_IO;
                }

                let offset = (c as usize) * 512;
                for i in 0..256 {
                    let w = inw(self.io_base);
                    buf[offset + i * 2] = (w & 0xFF) as u8;
                    buf[offset + i * 2 + 1] = (w >> 8) as u8;
                }
            }
        }
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
        let required_bytes = match (count as usize).checked_mul(512) {
            Some(b) => b,
            None => return BLOCK_DEV_ERR_BOUNDS,
        };
        if buf.len() < required_bytes {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        unsafe {
            for c in 0..count {
                let cur_lba = lba + c as u64;
                if !self.wait_not_busy() {
                    return BLOCK_DEV_ERR_IO;
                }

                outb(self.io_base + 6, 0xE0 | ((self.is_slave as u8) << 4) | (((cur_lba >> 24) & 0x0F) as u8));
                for _ in 0..4 { let _ = inb(self.ctrl_base); }
                outb(self.io_base + 2, 1);
                outb(self.io_base + 3, (cur_lba & 0xFF) as u8);
                outb(self.io_base + 4, ((cur_lba >> 8) & 0xFF) as u8);
                outb(self.io_base + 5, ((cur_lba >> 16) & 0xFF) as u8);
                outb(self.io_base + 7, 0x30); // WRITE SECTORS
                for _ in 0..4 { let _ = inb(self.ctrl_base); }

                if !self.wait_drq() {
                    return BLOCK_DEV_ERR_IO;
                }

                let offset = (c as usize) * 512;
                for i in 0..256 {
                    let b0 = buf[offset + i * 2] as u16;
                    let b1 = buf[offset + i * 2 + 1] as u16;
                    let w = b0 | (b1 << 8);
                    outw(self.io_base, w);
                }

                // Wait for the drive to complete the sector write
                for _ in 0..4 { let _ = inb(self.ctrl_base); }
                if !self.wait_not_busy() {
                    return BLOCK_DEV_ERR_IO;
                }
            }
        }
        BLOCK_DEV_OK
    }

    fn flush(&mut self) -> i32 {
        unsafe {
            if !self.wait_not_busy() {
                return BLOCK_DEV_ERR_IO;
            }
            outb(self.io_base + 6, 0xE0 | ((self.is_slave as u8) << 4));
            for _ in 0..4 { let _ = inb(self.ctrl_base); }
            outb(self.io_base + 7, 0xE7); // CACHE FLUSH
            for _ in 0..4 { let _ = inb(self.ctrl_base); }
            let _ = self.wait_not_busy();
        }
        BLOCK_DEV_OK
    }
}
