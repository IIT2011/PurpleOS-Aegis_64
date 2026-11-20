//! Pure Rust Bare-Metal Ext2 / Ext3 / Ext4 Filesystem Driver
//!
//! Provides structural parsing, superblock verification (0xEF53), block group
//! descriptors, inode table resolution, and directory record traversal.

use crate::drivers::block_dev::BlockDevice;

pub const EXT_MAGIC: u16 = 0xEF53;

pub const EXT_FT_UNKNOWN: u8 = 0;
pub const EXT_FT_REG_FILE: u8 = 1;
pub const EXT_FT_DIR: u8 = 2;
pub const EXT_FT_CHRDEV: u8 = 3;
pub const EXT_FT_BLKDEV: u8 = 4;
pub const EXT_FT_FIFO: u8 = 5;
pub const EXT_FT_SOCK: u8 = 6;
pub const EXT_FT_SYMLINK: u8 = 7;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExtType {
    Ext2,
    Ext3,
    Ext4,
}

impl ExtType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExtType::Ext2 => "EXT2",
            ExtType::Ext3 => "EXT3",
            ExtType::Ext4 => "EXT4",
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ExtSuperblock {
    pub s_inodes_count: u32,
    pub s_blocks_count: u32,
    pub s_r_blocks_count: u32,
    pub s_free_blocks_count: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_frag_size: u32,
    pub s_blocks_per_group: u32,
    pub s_frags_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    // Extended fields (rev >= 1)
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_volume_name: [u8; 16],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ExtBlockGroupDesc {
    pub bg_block_bitmap: u32,
    pub bg_inode_bitmap: u32,
    pub bg_inode_table: u32,
    pub bg_free_blocks_count: u16,
    pub bg_free_inodes_count: u16,
    pub bg_used_dirs_count: u16,
    pub bg_pad: u16,
    pub bg_reserved: [u32; 3],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ExtInode {
    pub i_mode: u16,
    pub i_uid: u16,
    pub i_size: u32,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u16,
    pub i_links_count: u16,
    pub i_blocks: u32,
    pub i_flags: u32,
    pub i_osd1: u32,
    pub i_block: [u32; 15], // 12 direct, 1 indirect, 1 double, 1 triple
    pub i_generation: u32,
    pub i_file_acl: u32,
    pub i_dir_acl: u32,
    pub i_faddr: u32,
    pub i_osd2: [u32; 3],
}

#[derive(Copy, Clone)]
pub struct ExtFsInfo {
    pub ext_type: ExtType,
    pub block_size: u32,
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub total_inodes: u32,
    pub free_inodes: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inode_size: u16,
    pub volume_name: [u8; 16],
}

pub struct ExtDriver {
    pub info: Option<ExtFsInfo>,
    pub start_lba: u64,
}

impl ExtDriver {
    pub const fn new() -> Self {
        Self {
            info: None,
            start_lba: 0,
        }
    }

    /// Check if a 1024-byte superblock contains the Ext magic (0xEF53)
    pub fn probe_superblock(buf: &[u8]) -> Option<ExtFsInfo> {
        if buf.len() < 1024 {
            return None;
        }

        // Magic at byte 56..58 (offset inside superblock)
        let magic = u16::from_le_bytes([buf[56], buf[57]]);
        if magic != EXT_MAGIC {
            return None;
        }

        let total_inodes = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let total_blocks = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let free_blocks = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let free_inodes = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let log_blk_sz = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
        let block_size = 1024u32.checked_shl(log_blk_sz)?;
        if !(1024..=65536).contains(&block_size)
            || total_blocks == 0
            || total_inodes == 0
            || free_blocks > total_blocks
            || free_inodes > total_inodes
        {
            return None;
        }

        let blocks_per_grp = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
        let inodes_per_grp = u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]);
        if blocks_per_grp == 0 || inodes_per_grp == 0 {
            return None;
        }
        let rev_level = u32::from_le_bytes([buf[76], buf[77], buf[78], buf[79]]);

        let inode_size = if rev_level >= 1 && buf.len() >= 90 {
            u16::from_le_bytes([buf[88], buf[89]])
        } else {
            128
        };

        if inode_size < 128 || !inode_size.is_power_of_two() || inode_size as u32 > block_size {
            return None;
        }

        // Ext2 vs Ext3 vs Ext4 features
        let compat = if rev_level >= 1 && buf.len() >= 96 {
            u32::from_le_bytes([buf[92], buf[93], buf[94], buf[95]])
        } else {
            0
        };
        let incompat = if rev_level >= 1 && buf.len() >= 100 {
            u32::from_le_bytes([buf[96], buf[97], buf[98], buf[99]])
        } else {
            0
        };
        let ro_compat = if rev_level >= 1 && buf.len() >= 104 {
            u32::from_le_bytes([buf[100], buf[101], buf[102], buf[103]])
        } else {
            0
        };

        // EXT4: has extents (incompat & 0x0040) or 64bit (incompat & 0x0080)
        // EXT3: has has_journal (compat & 0x0004)
        // EXT2: baseline
        let ext_type =
            if (incompat & 0x0040) != 0 || (incompat & 0x0080) != 0 || (ro_compat & 0x0008) != 0 {
                ExtType::Ext4
            } else if (compat & 0x0004) != 0 {
                ExtType::Ext3
            } else {
                ExtType::Ext2
            };

        let mut vol_name = [0u8; 16];
        if buf.len() >= 136 {
            vol_name.copy_from_slice(&buf[120..136]);
        }

        Some(ExtFsInfo {
            ext_type,
            block_size,
            total_blocks,
            free_blocks,
            total_inodes,
            free_inodes,
            blocks_per_group: blocks_per_grp,
            inodes_per_group: inodes_per_grp,
            inode_size,
            volume_name: vol_name,
        })
    }

    /// Probe a block device for an Ext filesystem (reads LBA offset + 2 sectors for superblock at 1024)
    pub fn probe(&mut self, dev: &mut dyn BlockDevice, start_lba: u64) -> Option<ExtType> {
        let mut buf = [0u8; 1024];
        // Superblock begins at byte 1024 (LBA start + 2)
        let superblock_lba = start_lba.checked_add(2)?;
        if dev.block_size() != 512
            || superblock_lba.checked_add(2)? > dev.total_blocks()
            || dev.read_blocks(superblock_lba, 2, &mut buf) != 0
        {
            return None;
        }
        let info = Self::probe_superblock(&buf)?;
        let t = info.ext_type;
        self.info = Some(info);
        self.start_lba = start_lba;
        Some(t)
    }
}
