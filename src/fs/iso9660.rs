//! Pure Rust Bare-Metal ISO9660 / ECMA-119 / El Torito Optical Filesystem Driver
//!
//! Provides structural parsing, Primary Volume Descriptor (PVD) identification ("CD001"),
//! optical directory record parsing, and CD/DVD live media inspection.

use crate::drivers::block_dev::BlockDevice;

pub const ISO_MAGIC: &[u8; 5] = b"CD001";
pub const ISO_SECTOR_SIZE: usize = 2048;
pub const ISO_PVD_LBA_OFFSET: u64 = 16; // Sector 16 (in 2048-byte sectors)

pub const ISO_FLAG_HIDDEN: u8    = 0x01;
pub const ISO_FLAG_DIRECTORY: u8 = 0x02;
pub const ISO_FLAG_ASSOCIATED: u8 = 0x04;

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IsoDirRecordHeader {
    pub length: u8,
    pub ext_attr_length: u8,
    pub extent_lba_le: u32,
    pub extent_lba_be: u32,
    pub data_length_le: u32,
    pub data_length_be: u32,
    pub date_time: [u8; 7],
    pub file_flags: u8,
    pub file_unit_size: u8,
    pub interleave_gap_size: u8,
    pub vol_seq_nr_le: u16,
    pub vol_seq_nr_be: u16,
    pub file_id_len: u8,
}

#[derive(Copy, Clone)]
pub struct IsoFsInfo {
    pub volume_id: [u8; 32],
    pub system_id: [u8; 32],
    pub volume_space_size: u32,
    pub logical_block_size: u16,
    pub root_extent_lba: u32,
    pub root_data_len: u32,
}

pub struct Iso9660Driver {
    pub info: Option<IsoFsInfo>,
    pub start_lba: u64,
}

impl Iso9660Driver {
    pub const fn new() -> Self {
        Self {
            info: None,
            start_lba: 0,
        }
    }

    /// Probe a 2048-byte buffer for ISO9660 Primary Volume Descriptor (PVD)
    pub fn probe_pvd(buf: &[u8]) -> Option<IsoFsInfo> {
        if buf.len() < 2048 {
            return None;
        }

        // PVD Type must be 1, Identifier must be "CD001", Version must be 1
        if buf[0] != 1 || &buf[1..6] != ISO_MAGIC || buf[6] != 1 {
            return None;
        }

        let mut sys_id = [0u8; 32];
        sys_id.copy_from_slice(&buf[8..40]);

        let mut vol_id = [0u8; 32];
        vol_id.copy_from_slice(&buf[40..72]);

        let vol_space_sz = u32::from_le_bytes([buf[80], buf[81], buf[82], buf[83]]);
        let log_blk_sz = u16::from_le_bytes([buf[128], buf[129]]);

        // Root directory record is located at byte 156
        let root_extent = u32::from_le_bytes([buf[156 + 2], buf[156 + 3], buf[156 + 4], buf[156 + 5]]);
        let root_len = u32::from_le_bytes([buf[156 + 10], buf[156 + 11], buf[156 + 12], buf[156 + 13]]);

        Some(IsoFsInfo {
            volume_id: vol_id,
            system_id: sys_id,
            volume_space_size: vol_space_sz,
            logical_block_size: if log_blk_sz > 0 { log_blk_sz } else { 2048 },
            root_extent_lba: root_extent,
            root_data_len: root_len,
        })
    }

    /// Probe a block device for ISO9660 filesystem at start_lba (PVD at 16 * 2048 bytes = 64 * 512 sectors)
    pub fn probe(&mut self, dev: &mut dyn BlockDevice, start_lba: u64) -> bool {
        // Sector 16 in 2048-byte units is 64 in 512-byte units
        let pvd_lba = start_lba + (ISO_PVD_LBA_OFFSET * 4);
        let mut buf = [0u8; 2048];
        if dev.read_blocks(pvd_lba, 4, &mut buf) != 0 {
            return false;
        }
        if let Some(info) = Self::probe_pvd(&buf) {
            self.info = Some(info);
            self.start_lba = start_lba;
            true
        } else {
            false
        }
    }
}
