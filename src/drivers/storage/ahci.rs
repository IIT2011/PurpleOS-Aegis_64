use crate::drivers::block_dev::{BlockDevice, BLOCK_DEV_ERR_BOUNDS, BLOCK_DEV_ERR_IO, BLOCK_DEV_OK};
use crate::drivers::dma::{alloc_coherent, free_coherent};

#[repr(C, packed)]
struct HbaPortRegs {
    clb: u32,
    clbu: u32,
    fb: u32,
    fbu: u32,
    is: u32,
    ie: u32,
    cmd: u32,
    rsv0: u32,
    tfd: u32,
    sig: u32,
    ssts: u32,
    sctl: u32,
    serr: u32,
    sact: u32,
    ci: u32,
    sntf: u32,
    fbs: u32,
    rsv1: [u32; 11],
    vendor: [u32; 4],
}

#[repr(C, packed)]
struct AhciCmdHeader {
    flags: u16,
    prdtl: u16,
    prdbc: u32,
    ctba: u32,
    ctbau: u32,
    rsv: [u32; 4],
}

#[repr(C, packed)]
struct AhciPrdtEntry {
    dba: u32,
    dbau: u32,
    rsv: u32,
    dbc: u32,
}

pub struct AhciPort {
    name: [u8; 8],
    regs: *mut HbaPortRegs,
    cmd_list: *mut AhciCmdHeader,
    cmd_list_phys: u64,
    cmd_table: *mut u8,
    cmd_table_phys: u64,
    dma_buf: *mut u8,
    dma_buf_phys: u64,
    pub total_sectors: u64,
    pub is_sata: bool,
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
        Self {
            name,
            regs: core::ptr::null_mut(),
            cmd_list: core::ptr::null_mut(),
            cmd_list_phys: 0,
            cmd_table: core::ptr::null_mut(),
            cmd_table_phys: 0,
            dma_buf: core::ptr::null_mut(),
            dma_buf_phys: 0,
            total_sectors: 0,
            is_sata: false,
        }
    }

    pub fn init(&mut self, regs: *mut u8) -> Result<(), &'static str> {
        self.regs = regs as *mut HbaPortRegs;
        unsafe {
            // Allocate command list (1024 bytes)
            let (cl_ptr, cl_phys) = alloc_coherent(1024).ok_or("Failed to alloc cmd list")?;
            self.cmd_list = cl_ptr as *mut AhciCmdHeader;
            self.cmd_list_phys = cl_phys;

            // Allocate command table (256 bytes)
            let (ct_ptr, ct_phys) = alloc_coherent(256).ok_or("Failed to alloc cmd table")?;
            self.cmd_table = ct_ptr;
            self.cmd_table_phys = ct_phys;

            // Allocate DMA scratch buffer (64 KiB)
            let (db_ptr, db_phys) = alloc_coherent(65536).ok_or("Failed to alloc dma buffer")?;
            self.dma_buf = db_ptr;
            self.dma_buf_phys = db_phys;

            (*self.regs).clb = self.cmd_list_phys as u32;
            (*self.regs).clbu = (self.cmd_list_phys >> 32) as u32;

            (*self.cmd_list).ctba = self.cmd_table_phys as u32;
            (*self.cmd_list).ctbau = (self.cmd_table_phys >> 32) as u32;

            // Start command engine (FRE | ST)
            (*self.regs).cmd |= 0x10 | 0x01;

            self.is_sata = true;
            self.total_sectors = 2097152; // Default 1 GiB disk if not yet identified
        }
        Ok(())
    }
}

impl BlockDevice for AhciPort {
    fn name(&self) -> &str {
        core::str::from_utf8(&self.name).unwrap_or("sata")
    }

    fn block_size(&self) -> u32 {
        512
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32 {
        if lba + count as u64 > self.total_sectors || self.dma_buf.is_null() {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        let bytes = (count as usize) * 512;
        if bytes > 65536 {
            return BLOCK_DEV_ERR_IO;
        }

        unsafe {
            // Setup FIS Register H2D (READ DMA EXT = 0x25)
            core::ptr::write_bytes(self.cmd_table, 0, 256);
            let fis = self.cmd_table;
            *fis.add(0) = 0x27; // FIS_TYPE_REG_H2D
            *fis.add(1) = 0x80; // Command bit set
            *fis.add(2) = 0x25; // ATA_CMD_READ_DMA_EXT
            *fis.add(4) = (lba & 0xFF) as u8;
            *fis.add(5) = ((lba >> 8) & 0xFF) as u8;
            *fis.add(6) = ((lba >> 16) & 0xFF) as u8;
            *fis.add(7) = 1 << 6; // LBA mode
            *fis.add(8) = ((lba >> 24) & 0xFF) as u8;
            *fis.add(9) = ((lba >> 32) & 0xFF) as u8;
            *fis.add(10) = ((lba >> 40) & 0xFF) as u8;
            *fis.add(12) = (count & 0xFF) as u8;
            *fis.add(13) = ((count >> 8) & 0xFF) as u8;

            let prdt = self.cmd_table.add(128) as *mut AhciPrdtEntry;
            (*prdt).dba = self.dma_buf_phys as u32;
            (*prdt).dbau = (self.dma_buf_phys >> 32) as u32;
            (*prdt).dbc = (bytes as u32) - 1;

            (*self.cmd_list).flags = 5; // 5 DWORDS, Read
            (*self.cmd_list).prdtl = 1;

            // Issue Slot 0
            (*self.regs).ci = 1;

            // Wait for completion
            let mut timeout = 1000000;
            while ((*self.regs).ci & 1) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            if timeout == 0 {
                return BLOCK_DEV_ERR_IO;
            }

            core::ptr::copy_nonoverlapping(self.dma_buf, buf.as_mut_ptr(), bytes);
        }
        BLOCK_DEV_OK
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32 {
        if lba + count as u64 > self.total_sectors || self.dma_buf.is_null() {
            return BLOCK_DEV_ERR_BOUNDS;
        }

        let bytes = (count as usize) * 512;
        if bytes > 65536 {
            return BLOCK_DEV_ERR_IO;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), self.dma_buf, bytes);

            // Setup FIS Register H2D (WRITE DMA EXT = 0x35)
            core::ptr::write_bytes(self.cmd_table, 0, 256);
            let fis = self.cmd_table;
            *fis.add(0) = 0x27;
            *fis.add(1) = 0x80;
            *fis.add(2) = 0x35; // ATA_CMD_WRITE_DMA_EXT
            *fis.add(4) = (lba & 0xFF) as u8;
            *fis.add(5) = ((lba >> 8) & 0xFF) as u8;
            *fis.add(6) = ((lba >> 16) & 0xFF) as u8;
            *fis.add(7) = 1 << 6;
            *fis.add(8) = ((lba >> 24) & 0xFF) as u8;
            *fis.add(9) = ((lba >> 32) & 0xFF) as u8;
            *fis.add(10) = ((lba >> 40) & 0xFF) as u8;
            *fis.add(12) = (count & 0xFF) as u8;
            *fis.add(13) = ((count >> 8) & 0xFF) as u8;

            let prdt = self.cmd_table.add(128) as *mut AhciPrdtEntry;
            (*prdt).dba = self.dma_buf_phys as u32;
            (*prdt).dbau = (self.dma_buf_phys >> 32) as u32;
            (*prdt).dbc = (bytes as u32) - 1;

            (*self.cmd_list).flags = 5 | (1 << 6); // 5 DWORDS, Write bit 6
            (*self.cmd_list).prdtl = 1;

            (*self.regs).ci = 1;

            let mut timeout = 1000000;
            while ((*self.regs).ci & 1) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            if timeout == 0 {
                return BLOCK_DEV_ERR_IO;
            }
        }
        BLOCK_DEV_OK
    }
}
