//! Pure Rust Bare-Metal ISO9660 / ECMA-119 / El Torito Optical Filesystem Driver
//!
//! Provides structural parsing, Primary Volume Descriptor (PVD) identification ("CD001"),
//! optical directory record parsing, and CD/DVD live media inspection.

use crate::drivers::block_dev::BlockDevice;

pub const ISO_MAGIC: &[u8; 5] = b"CD001";
pub const ISO_SECTOR_SIZE: usize = 2048;
pub const ISO_PVD_LBA_OFFSET: u64 = 16; // Sector 16 (in 2048-byte sectors)

pub const ISO_FLAG_HIDDEN: u8 = 0x01;
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
        let volume_space_size_be = u32::from_be_bytes([buf[84], buf[85], buf[86], buf[87]]);
        let log_blk_sz_le = u16::from_le_bytes([buf[128], buf[129]]);
        let log_blk_sz_be = u16::from_be_bytes([buf[130], buf[131]]);
        if vol_space_sz == 0
            || vol_space_sz != volume_space_size_be
            || log_blk_sz_le != ISO_SECTOR_SIZE as u16
            || log_blk_sz_be != ISO_SECTOR_SIZE as u16
            || buf[156] < 34
            || buf[156 + 32] != 1
            || buf[156 + 25] & ISO_FLAG_DIRECTORY == 0
        {
            return None;
        }

        // Root directory record is located at byte 156.
        let root_extent = u32::from_le_bytes([buf[158], buf[159], buf[160], buf[161]]);
        let root_extent_be = u32::from_be_bytes([buf[162], buf[163], buf[164], buf[165]]);
        let root_len = u32::from_le_bytes([buf[166], buf[167], buf[168], buf[169]]);
        let root_len_be = u32::from_be_bytes([buf[170], buf[171], buf[172], buf[173]]);
        if root_extent != root_extent_be
            || root_len != root_len_be
            || root_extent >= vol_space_sz
            || root_len == 0
        {
            return None;
        }

        Some(IsoFsInfo {
            volume_id: vol_id,
            system_id: sys_id,
            volume_space_size: vol_space_sz,
            logical_block_size: ISO_SECTOR_SIZE as u16,
            root_extent_lba: root_extent,
            root_data_len: root_len,
        })
    }

    /// Probe a block device for ISO9660 filesystem at start_lba (PVD at 16 * 2048 bytes = 64 * 512 sectors)
    pub fn probe(&mut self, dev: &mut dyn BlockDevice, start_lba: u64) -> bool {
        // Sector 16 in 2048-byte units is 64 in 512-byte units
        if dev.block_size() != 512 {
            return false;
        }
        let Some(pvd_lba) = start_lba.checked_add(ISO_PVD_LBA_OFFSET * 4) else {
            return false;
        };
        let Some(pvd_end) = pvd_lba.checked_add(4) else {
            return false;
        };
        let mut buf = [0u8; ISO_SECTOR_SIZE];
        if pvd_end > dev.total_blocks() || dev.read_blocks(pvd_lba, 4, &mut buf) != 0 {
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
