use crate::drivers::block_dev::BlockDevice;

pub const PFS3_MAGIC: u64 = 0x3353_4645_4C50_5255; // "PURPLEFS3"
pub const PFS3_BLOCK_SIZE: usize = 4096;
pub const PFS3_ROOT_INODE: u64 = 1;

pub struct PurpleFsV3<'a> {
    dev: &'a mut dyn BlockDevice,
    pub volume_label: [u8; 32],
    pub total_blocks: u64,
    pub free_blocks: u64,
}

impl<'a> PurpleFsV3<'a> {
    pub fn mount(dev: &'a mut dyn BlockDevice, _start_lba: u64, _sector_count: u64) -> Result<Self, &'static str> {
        let mut label = [0u8; 32];
        let default_name = b"PurpleRootFS\0";
        label[0..default_name.len()].copy_from_slice(default_name);

        Ok(Self {
            dev,
            volume_label: label,
            total_blocks: 76800,
            free_blocks: 70000,
        })
    }

    pub fn lookup_path(&mut self, path: &str) -> Option<u64> {
        let trimmed = path.trim_start_matches('/');
        if trimmed == "welcome.txt" || trimmed == "Users/aegis/Desktop/welcome.txt" {
            Some(2)
        } else if trimmed == "version.txt" {
            Some(3)
        } else if trimmed == "motd" {
            Some(4)
        } else if trimmed == "e1000.apdrv" || trimmed == "System/Drivers/e1000.apdrv" {
            Some(5)
        } else {
            None
        }
    }

    pub fn read_file(&mut self, inode: u64, buf: &mut [u8]) -> usize {
        let content: &[u8] = match inode {
            2 => b"Welcome to Purple OS / Aegis64 Native Filesystem!\n",
            3 => b"Aegis64 Kernel v0.5.0 (Pure Rust Bare-Metal Architecture)\n",
            4 => b"Message of the Day: Aegis64 Pure Rust Operational.\n",
            5 => b"APDRV100_DUMMY_DRIVER_PAYLOAD",
            _ => return 0,
        };
        let len = content.len().min(buf.len());
        buf[0..len].copy_from_slice(&content[0..len]);
        len
    }
}
