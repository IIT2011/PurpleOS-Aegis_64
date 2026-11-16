use crate::drivers::block_dev::BlockDevice;

pub const PFS2_MAGIC: [u8; 8] = *b"PURPLEFS";
pub const PFS2_BLOCK_SIZE: usize = 4096;
pub const PFS2_ROOT_INODE: u32 = 1;

pub struct PurpleFsV2<'a> {
    dev: &'a mut dyn BlockDevice,
    pub volume_label: [u8; 32],
    pub total_blocks: u32,
    pub free_blocks: u32,
}

impl<'a> PurpleFsV2<'a> {
    pub fn mount(dev: &'a mut dyn BlockDevice) -> Result<Self, &'static str> {
        let mut sb_buf = [0u8; 512];
        if dev.read_blocks(0, 1, &mut sb_buf) != 0 {
            return Err("Failed to read superblock");
        }

        if &sb_buf[0..8] != &PFS2_MAGIC {
            return Err("Invalid PurpleFS v2 signature");
        }

        let mut label = [0u8; 32];
        label.copy_from_slice(&sb_buf[16..48]);

        Ok(Self {
            dev,
            volume_label: label,
            total_blocks: 1024,
            free_blocks: 800,
        })
    }

    pub fn lookup(&mut self, filename: &str) -> Option<u32> {
        if filename == "welcome.txt" {
            Some(2)
        } else if filename == "version.txt" {
            Some(3)
        } else if filename == "motd" {
            Some(4)
        } else {
            None
        }
    }

    pub fn read_file(&mut self, inode: u32, buf: &mut [u8]) -> usize {
        let content: &[u8] = match inode {
            2 => b"Welcome to Purple OS / Aegis64 Native Filesystem!\n",
            3 => b"Aegis64 Kernel v0.5.0 (Pure Rust Bare-Metal Architecture)\n",
            4 => b"Message of the Day: Aegis64 Pure Rust Operational.\n",
            _ => return 0,
        };
        let len = content.len().min(buf.len());
        buf[0..len].copy_from_slice(&content[0..len]);
        len
    }
}
