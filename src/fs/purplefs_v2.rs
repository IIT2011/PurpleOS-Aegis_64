use crate::drivers::block_dev::BlockDevice;

pub const PFS2_MAGIC: [u8; 8] = *b"PURPLEFS";
pub const PFS2_BLOCK_SIZE: usize = 4096;
pub const PFS2_ROOT_INODE: u32 = 1;
pub const PFS2_SECTOR_SIZE: usize = 512;
pub const PFS2_SECTORS_PER_BLOCK: u64 = (PFS2_BLOCK_SIZE / PFS2_SECTOR_SIZE) as u64;
pub const PFS2_MAX_INODES: u32 = 256;
pub const PFS2_ROOT_DIR_BLOCK: u32 = 10;
pub const PFS2_FIRST_DATA_BLOCK: u32 = 11;
pub const PFS2_DIRECT_BLOCKS: usize = 12;
pub const PFS2_INDIRECT_POINTERS: usize = PFS2_BLOCK_SIZE / core::mem::size_of::<u32>();
pub const PFS2_SUPERBLOCK_CRC_OFFSET: usize = 0x80;

pub struct PurpleFsV2<'a> {
    pub dev: &'a mut dyn BlockDevice,
    pub start_lba: u64,
    pub partition_sector_count: u64,
    pub volume_label: [u8; 32],
    pub total_blocks: u32,
    journal_start: u32,
    pub free_blocks: u32,
    pub read_only: bool,
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

impl<'a> PurpleFsV2<'a> {
    pub fn mount(
        dev: &'a mut dyn BlockDevice,
        start_lba: u64,
        partition_sector_count: u64,
    ) -> Result<Self, &'static str> {
        if dev.block_size() as usize != PFS2_SECTOR_SIZE {
            return Err("PurpleFS v2 requires 512-byte logical sectors");
        }
        let mut sb_buf = [0u8; PFS2_SECTOR_SIZE];
        if dev.read_blocks(start_lba, 1, &mut sb_buf) != 0 {
            return Err("Failed to read superblock");
        }

        if &sb_buf[0..8] != &PFS2_MAGIC {
            return Err("Invalid PurpleFS v2 signature");
        }

        let expected_crc = u32::from_le_bytes(
            sb_buf[PFS2_SUPERBLOCK_CRC_OFFSET..PFS2_SUPERBLOCK_CRC_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        if crc32(&sb_buf[..PFS2_SUPERBLOCK_CRC_OFFSET]) != expected_crc {
            return Err("PurpleFS v2 superblock CRC mismatch");
        }
        let version = u32::from_le_bytes(sb_buf[16..20].try_into().unwrap());
        let block_size = u32::from_le_bytes(sb_buf[20..24].try_into().unwrap());
        if version != 2 || block_size as usize != PFS2_BLOCK_SIZE {
            return Err("Unsupported PurpleFS version or block size");
        }
        let field = |offset| u32::from_le_bytes(sb_buf[offset..offset + 4].try_into().unwrap());
        let (mut total_blocks, mut free_blocks) = (field(24), field(28));
        let journal_start = field(60);
        let journal_blocks = field(64);
        let journal_end = journal_start
            .checked_add(journal_blocks)
            .ok_or("PurpleFS v2 journal range overflow")?;

        // Older image builders stored free blocks at 24 and free inodes at 28.
        // Their contiguous journal still records the formatted volume boundary.
        if total_blocks < journal_end
            && journal_blocks != 0
            && total_blocks < journal_start
            && free_blocks < PFS2_MAX_INODES
            && field(32) == PFS2_MAX_INODES
            && field(36) == PFS2_MAX_INODES
        {
            (total_blocks, free_blocks) = (journal_end, total_blocks);
        }

        if total_blocks <= PFS2_ROOT_DIR_BLOCK
            || free_blocks > total_blocks
            || total_blocks as usize > PFS2_BLOCK_SIZE * 8
            || (journal_blocks != 0
                && (journal_start < PFS2_FIRST_DATA_BLOCK || journal_end > total_blocks))
        {
            return Err("Invalid PurpleFS v2 block counts");
        }
        let journal_start = if journal_blocks == 0 {
            total_blocks
        } else {
            journal_start
        };
        let volume_sectors = (total_blocks as u64)
            .checked_mul(PFS2_SECTORS_PER_BLOCK)
            .ok_or("PurpleFS v2 volume size overflow")?;
        let volume_end = start_lba
            .checked_add(volume_sectors)
            .ok_or("PurpleFS v2 volume range overflow")?;
        let partition_end = start_lba
            .checked_add(partition_sector_count)
            .ok_or("PurpleFS v2 partition range overflow")?;
        if partition_end > dev.total_blocks()
            || volume_end > partition_end
            || volume_sectors > partition_sector_count
        {
            return Err("PurpleFS v2 volume extends past its partition");
        }

        let mut label = [0u8; 32];
        label.copy_from_slice(&sb_buf[0x70..0x70 + 32]);

        Ok(Self {
            dev,
            start_lba,
            partition_sector_count,
            volume_label: label,
            total_blocks,
            journal_start,
            free_blocks,
            read_only: true,
        })
    }

    /// Read an entire 4096-byte filesystem block (8 sectors of 512 bytes)
    pub fn read_fs_block(
        &mut self,
        block_idx: u32,
        buf: &mut [u8; PFS2_BLOCK_SIZE],
    ) -> Result<(), &'static str> {
        if block_idx >= self.total_blocks {
            return Err("Filesystem block index out of range");
        }
        let sector_offset = (block_idx as u64)
            .checked_mul(PFS2_SECTORS_PER_BLOCK)
            .ok_or("Filesystem block LBA overflow")?;
        let lba = self
            .start_lba
            .checked_add(sector_offset)
            .ok_or("Filesystem block LBA overflow")?;
        if self
            .dev
            .read_blocks(lba, PFS2_SECTORS_PER_BLOCK as u32, buf)
            == 0
        {
            Ok(())
        } else {
            Err("Block device read failure")
        }
    }

    /// Write an entire 4096-byte filesystem block (8 sectors of 512 bytes)
    pub fn write_fs_block(
        &mut self,
        block_idx: u32,
        buf: &[u8; PFS2_BLOCK_SIZE],
    ) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        if block_idx >= self.total_blocks {
            return Err("Filesystem block index out of range");
        }
        let sector_offset = (block_idx as u64)
            .checked_mul(PFS2_SECTORS_PER_BLOCK)
            .ok_or("Filesystem block LBA overflow")?;
        let lba = self
            .start_lba
            .checked_add(sector_offset)
            .ok_or("Filesystem block LBA overflow")?;
        if self
            .dev
            .write_blocks(lba, PFS2_SECTORS_PER_BLOCK as u32, buf)
            == 0
        {
            Ok(())
        } else {
            Err("Block device write failure")
        }
    }

    pub fn lookup(&mut self, filename: &str) -> Option<u32> {
        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        if self.read_fs_block(10, &mut dir_block).is_err() {
            return None;
        }

        let mut off = 0;
        while off + 32 <= PFS2_BLOCK_SIZE {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let rec_len =
                u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            let name_len = dir_block[off + 6] as usize;

            if rec_len == 0
                || rec_len < 8 + name_len
                || rec_len % 4 != 0
                || rec_len > PFS2_BLOCK_SIZE - off
            {
                return None;
            }
            if ino != 0 && ino < PFS2_MAX_INODES && name_len > 0 && name_len <= rec_len - 8 {
                let name = &dir_block[off + 8..off + 8 + name_len];
                if name == filename.as_bytes() {
                    return Some(ino);
                }
            }

            off += rec_len;
        }

        None
    }

    pub fn get_file_size(&mut self, inode: u32) -> Option<usize> {
        if inode == 0 || inode >= PFS2_MAX_INODES {
            return None;
        }
        let blk_idx = 2 + (inode / 32);
        let ino_off = ((inode % 32) * 128) as usize;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
        if self.read_fs_block(blk_idx, &mut inode_block).is_err() {
            return None;
        }
        let size_bytes =
            u64::from_le_bytes(inode_block[ino_off + 8..ino_off + 16].try_into().unwrap()) as usize;
        Some(size_bytes)
    }

    pub fn read_file_at(&mut self, inode: u32, offset: usize, buf: &mut [u8]) -> usize {
        if inode == 0 || inode >= PFS2_MAX_INODES || buf.is_empty() {
            return 0;
        }

        let blk_idx = 2 + (inode / 32);
        let ino_off = ((inode % 32) * 128) as usize;

        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
        if self.read_fs_block(blk_idx, &mut inode_block).is_err() {
            return 0;
        }

        let size_bytes =
            u64::from_le_bytes(inode_block[ino_off + 8..ino_off + 16].try_into().unwrap()) as usize;

        if offset >= size_bytes {
            return 0;
        }

        let total_file_blocks = size_bytes.saturating_add(PFS2_BLOCK_SIZE - 1) / PFS2_BLOCK_SIZE;
        let mut bytes_copied = 0;
        let mut block_buf = [0u8; PFS2_BLOCK_SIZE];
        let mut indirect_buf = [0u8; PFS2_BLOCK_SIZE];
        let mut indirect_loaded = false;
        let indirect_blk = u32::from_le_bytes(
            inode_block[ino_off + 0x5C..ino_off + 0x60]
                .try_into()
                .unwrap(),
        );

        for i in 0..total_file_blocks.min(PFS2_DIRECT_BLOCKS + PFS2_INDIRECT_POINTERS) {
            if bytes_copied >= buf.len() {
                break;
            }
            let blk_start = i * PFS2_BLOCK_SIZE;
            let blk_end = blk_start + PFS2_BLOCK_SIZE;
            let current_pos = match offset.checked_add(bytes_copied) {
                Some(pos) => pos,
                None => break,
            };

            if current_pos >= size_bytes {
                break;
            }

            if current_pos >= blk_end {
                continue;
            }

            let data_blk = if i < PFS2_DIRECT_BLOCKS {
                let ptr_off = ino_off + 0x2C + i * 4;
                u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap())
            } else {
                if indirect_blk == 0 {
                    0
                } else {
                    if !indirect_loaded {
                        if self.read_fs_block(indirect_blk, &mut indirect_buf).is_ok() {
                            indirect_loaded = true;
                        } else {
                            break;
                        }
                    }
                    let ind_idx = i - PFS2_DIRECT_BLOCKS;
                    u32::from_le_bytes(
                        indirect_buf[ind_idx * core::mem::size_of::<u32>()
                            ..(ind_idx + 1) * core::mem::size_of::<u32>()]
                            .try_into()
                            .unwrap(),
                    )
                }
            };
            if data_blk == 0 {
                break;
            }

            if self.read_fs_block(data_blk, &mut block_buf).is_ok() {
                let off_in_blk = current_pos - blk_start;
                let avail_in_blk = PFS2_BLOCK_SIZE - off_in_blk;
                let avail_in_file = size_bytes - current_pos;
                let chunk_len = avail_in_blk
                    .min(avail_in_file)
                    .min(buf.len() - bytes_copied);

                buf[bytes_copied..bytes_copied + chunk_len]
                    .copy_from_slice(&block_buf[off_in_blk..off_in_blk + chunk_len]);
                bytes_copied += chunk_len;
            } else {
                break;
            }
        }

        bytes_copied
    }

    pub fn read_file(&mut self, inode: u32, buf: &mut [u8]) -> usize {
        self.read_file_at(inode, 0, buf)
    }

    pub fn list_files<F: FnMut(u32, u8, u64, &str)>(&mut self, mut callback: F) {
        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        if self.read_fs_block(10, &mut dir_block).is_err() {
            return;
        }

        let mut off = 0;
        while off + 32 <= PFS2_BLOCK_SIZE {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let rec_len =
                u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            let name_len = dir_block[off + 6] as usize;
            let ftype = dir_block[off + 7];

            if rec_len == 0
                || rec_len < 8 + name_len
                || rec_len % 4 != 0
                || rec_len > PFS2_BLOCK_SIZE - off
            {
                break;
            }
            if ino != 0 && ino < PFS2_MAX_INODES && name_len > 0 && name_len <= rec_len - 8 {
                if let Ok(name_str) = core::str::from_utf8(&dir_block[off + 8..off + 8 + name_len])
                {
                    if name_str != "." && name_str != ".." {
                        // Lookup size from inode
                        let blk_idx = 2 + (ino / 32);
                        let ino_off = ((ino % 32) * 128) as usize;
                        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
                        let mut fsize = 0u64;
                        if self.read_fs_block(blk_idx, &mut inode_block).is_ok() {
                            fsize = u64::from_le_bytes(
                                inode_block[ino_off + 8..ino_off + 16].try_into().unwrap(),
                            );
                        }
                        callback(ino, ftype, fsize, name_str);
                    }
                }
            }

            off += rec_len;
        }
    }

    pub fn touch(&mut self, filename: &str) -> Result<u32, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        if let Some(existing) = self.lookup(filename) {
            return Ok(existing);
        }

        // Find free inode (scan inodes 2..255)
        let mut free_ino = None;
        let mut target_blk = 0;
        let mut target_ino_off = 0;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];

        for ino in 2..256 {
            let blk = 2 + (ino / 32);
            let off = ((ino % 32) * 128) as usize;
            if self.read_fs_block(blk, &mut inode_block).is_ok() {
                let mode = u16::from_le_bytes(inode_block[off..off + 2].try_into().unwrap());
                if mode == 0 {
                    free_ino = Some(ino);
                    target_blk = blk;
                    target_ino_off = off;
                    break;
                }
            }
        }

        let new_ino = free_ino.ok_or("No free inodes available")?;

        // Initialize new inode (Regular file, 0644, size 0)
        inode_block[target_ino_off..target_ino_off + 2].copy_from_slice(&0x81A4u16.to_le_bytes());
        inode_block[target_ino_off + 6..target_ino_off + 8].copy_from_slice(&1u16.to_le_bytes()); // links=1
        inode_block[target_ino_off + 8..target_ino_off + 16].copy_from_slice(&0u64.to_le_bytes()); // size=0
        self.write_fs_block(target_blk, &inode_block)?;

        // Add dirent into root dir block (block 10)
        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(10, &mut dir_block)?;

        let mut off = 0;
        let mut slot_found = false;
        let b = filename.as_bytes();
        let name_len = b.len().min(55);
        let rec_len = if name_len > 23 { 64u16 } else { 32u16 };

        while off + (rec_len as usize) <= PFS2_BLOCK_SIZE {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let cur_rec_len =
                u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            if cur_rec_len == 0 || (ino == 0 && cur_rec_len == rec_len as usize) {
                dir_block[off..off + 4].copy_from_slice(&new_ino.to_le_bytes());
                dir_block[off + 4..off + 6].copy_from_slice(&rec_len.to_le_bytes());
                dir_block[off + 6] = name_len as u8;
                dir_block[off + 7] = 1; // regular file
                dir_block[off + 8..off + (rec_len as usize)].fill(0);
                dir_block[off + 8..off + 8 + name_len].copy_from_slice(&b[..name_len]);
                slot_found = true;
                break;
            }
            off += cur_rec_len.max(32);
        }

        if !slot_found {
            return Err("Root directory full");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(new_ino)
    }

    pub fn mkdir(&mut self, dirname: &str) -> Result<u32, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let clean = dirname.trim_start_matches('/').trim_end_matches('/');
        if clean.is_empty() {
            return Err("Cannot create directory with empty name");
        }
        if self.lookup(clean).is_some() {
            return Err("Directory already exists");
        }

        // Find free inode (scan inodes 2..256)
        let mut free_ino = None;
        let mut target_blk = 0;
        let mut target_ino_off = 0;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];

        for ino in 2..256 {
            let blk = 2 + (ino / 32);
            let off = ((ino % 32) * 128) as usize;
            if self.read_fs_block(blk, &mut inode_block).is_ok() {
                let mode = u16::from_le_bytes(inode_block[off..off + 2].try_into().unwrap());
                if mode == 0 {
                    free_ino = Some(ino);
                    target_blk = blk;
                    target_ino_off = off;
                    break;
                }
            }
        }

        let new_ino = free_ino.ok_or("No free inodes available")?;

        // Initialize new directory inode (Directory, 0755, size PFS2_BLOCK_SIZE, links 2)
        inode_block[target_ino_off..target_ino_off + 2].copy_from_slice(&0x41EDu16.to_le_bytes());
        inode_block[target_ino_off + 2..target_ino_off + 4].copy_from_slice(&0u16.to_le_bytes()); // uid=0
        inode_block[target_ino_off + 4..target_ino_off + 6].copy_from_slice(&0u16.to_le_bytes()); // gid=0
        inode_block[target_ino_off + 6..target_ino_off + 8].copy_from_slice(&2u16.to_le_bytes()); // links=2
        inode_block[target_ino_off + 8..target_ino_off + 16]
            .copy_from_slice(&(PFS2_BLOCK_SIZE as u64).to_le_bytes()); // size=PFS2_BLOCK_SIZE
        self.write_fs_block(target_blk, &inode_block)?;

        // Add dirent into root dir block (block 10)
        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(10, &mut dir_block)?;

        let mut off = 0;
        let mut slot_found = false;
        let b = clean.as_bytes();
        let name_len = b.len().min(55);
        let rec_len = if name_len > 23 { 64u16 } else { 32u16 };

        while off + (rec_len as usize) <= PFS2_BLOCK_SIZE {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let cur_rec_len =
                u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            if cur_rec_len == 0 || (ino == 0 && cur_rec_len == rec_len as usize) {
                dir_block[off..off + 4].copy_from_slice(&new_ino.to_le_bytes());
                dir_block[off + 4..off + 6].copy_from_slice(&rec_len.to_le_bytes());
                dir_block[off + 6] = name_len as u8;
                dir_block[off + 7] = 2; // Directory file type
                dir_block[off + 8..off + (rec_len as usize)].fill(0);
                dir_block[off + 8..off + 8 + name_len].copy_from_slice(&b[..name_len]);
                slot_found = true;
                break;
            }
            off += cur_rec_len.max(32);
        }

        if !slot_found {
            return Err("Root directory full");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(new_ino)
    }

    pub fn rmdir(&mut self, dirname: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let clean = dirname.trim_start_matches('/').trim_end_matches('/');
        let ino = self.lookup(clean).ok_or("Directory not found")?;
        if ino == PFS2_ROOT_INODE {
            return Err("Cannot remove the root directory");
        }

        // Read inode to verify it is a directory
        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        let mode = u16::from_le_bytes(inode_block[ino_off..ino_off + 2].try_into().unwrap());
        if (mode & 0xF000) != 0x4000 {
            return Err("Not a directory");
        }

        // Check if directory has any children in the filesystem
        let mut has_children = false;
        self.list_files(|_child_ino, _ftype, _fsize, name| {
            if name.starts_with(clean) && name.as_bytes().get(clean.len()) == Some(&b'/') {
                has_children = true;
            }
        });
        if has_children {
            return Err("Directory not empty");
        }

        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(PFS2_ROOT_DIR_BLOCK, &mut dir_block)?;
        let (offset, len) = Self::find_dirent(&dir_block, ino)?;
        Self::clear_dirent(&mut dir_block, offset, len);
        self.write_fs_block(PFS2_ROOT_DIR_BLOCK, &dir_block)?;

        inode_block[ino_off..ino_off + 128].fill(0);
        self.write_fs_block(blk_idx, &inode_block)?;
        Ok(())
    }

    pub fn write_file(&mut self, filename: &str, content: &[u8]) -> Result<usize, &'static str> {
        self.write_file_at(filename, 0, content)
    }

    pub fn write_file_at(
        &mut self,
        filename: &str,
        offset: usize,
        content: &[u8],
    ) -> Result<usize, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        if content.is_empty() {
            return Ok(0);
        }
        if offset >= PFS2_DIRECT_BLOCKS * PFS2_BLOCK_SIZE {
            return Err("File write offset exceeds maximum direct block capacity (48 KiB)");
        }

        let ino = match self.lookup(filename) {
            Some(i) => i,
            None => self.touch(filename)?,
        };

        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        let current_size =
            u64::from_le_bytes(inode_block[ino_off + 8..ino_off + 16].try_into().unwrap()) as usize;

        let mut bytes_written = 0usize;
        let mut bitmap = [0u8; PFS2_BLOCK_SIZE];
        let mut bitmap_loaded = false;

        while bytes_written < content.len() {
            let cur_pos = offset + bytes_written;
            let block_idx = cur_pos / PFS2_BLOCK_SIZE;
            if block_idx >= PFS2_DIRECT_BLOCKS {
                break;
            }
            let off_in_block = cur_pos % PFS2_BLOCK_SIZE;
            let avail_in_block = PFS2_BLOCK_SIZE - off_in_block;
            let chunk_len = (content.len() - bytes_written).min(avail_in_block);

            let ptr_off = ino_off + 0x2C + (block_idx * 4);
            let mut blk = u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap());

            if blk == 0 {
                if !bitmap_loaded {
                    self.read_fs_block(1, &mut bitmap)?;
                    bitmap_loaded = true;
                }
                let mut free_blk = None;
                for b in PFS2_FIRST_DATA_BLOCK..self.journal_start {
                    let byte_idx = (b / 8) as usize;
                    let bit_idx = (b % 8) as usize;
                    if byte_idx < PFS2_BLOCK_SIZE && (bitmap[byte_idx] & (1 << bit_idx)) == 0 {
                        bitmap[byte_idx] |= 1 << bit_idx;
                        free_blk = Some(b);
                        break;
                    }
                }
                blk = free_blk.ok_or("No free data blocks available")?;
                inode_block[ptr_off..ptr_off + 4].copy_from_slice(&blk.to_le_bytes());
            }

            let mut data_buf = [0u8; PFS2_BLOCK_SIZE];
            if off_in_block > 0 || chunk_len < PFS2_BLOCK_SIZE {
                let _ = self.read_fs_block(blk, &mut data_buf);
            }
            data_buf[off_in_block..off_in_block + chunk_len]
                .copy_from_slice(&content[bytes_written..bytes_written + chunk_len]);
            self.write_fs_block(blk, &data_buf)?;
            bytes_written += chunk_len;
        }

        if bitmap_loaded {
            self.write_fs_block(1, &bitmap)?;
        }

        let new_size = current_size.max(offset + bytes_written);
        inode_block[ino_off + 8..ino_off + 16].copy_from_slice(&(new_size as u64).to_le_bytes());
        self.write_fs_block(blk_idx, &inode_block)?;

        Ok(bytes_written)
    }

    pub fn copy_file(&mut self, src_name: &str, dst_name: &str) -> Result<usize, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(src_name).ok_or("Source file not found")?;
        let file_size = self.get_file_size(ino).ok_or("Failed to get file size")?;

        let mut total_copied = 0;
        let mut chunk_buf = [0u8; PFS2_BLOCK_SIZE];
        while total_copied < file_size {
            let to_read = (file_size - total_copied).min(PFS2_BLOCK_SIZE);
            let n = self.read_file_at(ino, total_copied, &mut chunk_buf[..to_read]);
            if n == 0 {
                break;
            }
            self.write_file_at(dst_name, total_copied, &chunk_buf[..n])?;
            total_copied += n;
        }
        Ok(total_copied)
    }

    pub fn move_file(&mut self, src_name: &str, dst_name: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(src_name).ok_or("Source file not found")?;
        if ino == PFS2_ROOT_INODE {
            return Err("Cannot rename the root directory");
        }
        if self.lookup(dst_name).is_some() {
            return Err("Destination file already exists");
        }

        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(10, &mut dir_block)?;
        let (offset, len) = Self::find_dirent(&dir_block, ino)?;
        let name = dst_name.as_bytes();
        if name.is_empty() || name.len() > len - 8 {
            return Err("New name does not fit the existing directory entry");
        }
        dir_block[offset + 6] = name.len() as u8;
        dir_block[offset + 8..offset + len].fill(0);
        dir_block[offset + 8..offset + 8 + name.len()].copy_from_slice(name);

        self.write_fs_block(10, &dir_block)?;
        Ok(())
    }

    pub fn delete_file(&mut self, filename: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(filename).ok_or("File not found")?;

        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(blk_idx, &mut inode_block)?;
        let mode = u16::from_le_bytes(inode_block[ino_off..ino_off + 2].try_into().unwrap());
        if mode & 0xF000 != 0x8000 {
            return Err("Not a regular file");
        }

        let mut dir_block = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(PFS2_ROOT_DIR_BLOCK, &mut dir_block)?;
        let (entry_offset, entry_len) = Self::find_dirent(&dir_block, ino)?;

        let mut bitmap = [0u8; PFS2_BLOCK_SIZE];
        self.read_fs_block(1, &mut bitmap)?;
        let data_end = self.journal_start as usize;
        let mut release = |blk: u32| -> Result<(), &'static str> {
            let blk = blk as usize;
            if blk != 0 {
                if blk < PFS2_FIRST_DATA_BLOCK as usize
                    || blk >= data_end
                    || blk / 8 >= bitmap.len()
                {
                    return Err("Invalid allocated file block");
                }
                let mask = 1 << (blk % 8);
                if bitmap[blk / 8] & mask == 0 {
                    return Err("File block is unallocated or duplicated");
                }
                bitmap[blk / 8] &= !mask;
            }
            Ok(())
        };

        let direct_end = ino_off + 0x2C + PFS2_DIRECT_BLOCKS * size_of::<u32>();
        for ptr in inode_block[ino_off + 0x2C..direct_end].chunks_exact(size_of::<u32>()) {
            release(u32::from_le_bytes(ptr.try_into().unwrap()))?;
        }
        let indirect = u32::from_le_bytes(
            inode_block[direct_end..direct_end + size_of::<u32>()]
                .try_into()
                .unwrap(),
        );
        if indirect != 0 {
            release(indirect)?;
            self.read_fs_block(indirect, &mut dir_block)?;
            for ptr in dir_block.chunks_exact(size_of::<u32>()) {
                release(u32::from_le_bytes(ptr.try_into().unwrap()))?;
            }
        }

        // Remove the reference before allowing the inode or any data blocks to be reused.
        if indirect != 0 {
            self.read_fs_block(PFS2_ROOT_DIR_BLOCK, &mut dir_block)?;
        }
        Self::clear_dirent(&mut dir_block, entry_offset, entry_len);
        self.write_fs_block(PFS2_ROOT_DIR_BLOCK, &dir_block)?;

        inode_block[ino_off..ino_off + 128].fill(0);
        self.write_fs_block(blk_idx, &inode_block)?;
        self.write_fs_block(1, &bitmap)?;

        Ok(())
    }

    fn find_dirent(
        block: &[u8; PFS2_BLOCK_SIZE],
        inode: u32,
    ) -> Result<(usize, usize), &'static str> {
        let mut offset = 0;
        while offset + 8 <= block.len() {
            let entry_inode = u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap());
            let len =
                u16::from_le_bytes(block[offset + 4..offset + 6].try_into().unwrap()) as usize;
            if len == 0 {
                break;
            }
            if len < 8 || len % 4 != 0 || len > block.len() - offset {
                return Err("Invalid directory entry length");
            }
            if entry_inode == inode {
                return Ok((offset, len));
            }
            offset += len;
        }
        Err("Dirent not found in directory block")
    }

    fn clear_dirent(block: &mut [u8; PFS2_BLOCK_SIZE], offset: usize, len: usize) {
        block[offset..offset + len].fill(0);
        block[offset + 4..offset + 6].copy_from_slice(&(len as u16).to_le_bytes());
    }

    pub fn sync(&mut self) -> Result<(), &'static str> {
        if self.dev.flush() == 0 {
            Ok(())
        } else {
            Err("Failed to flush device")
        }
    }
}
