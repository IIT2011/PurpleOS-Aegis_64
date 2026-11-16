use crate::drivers::block_dev::BlockDevice;

pub const PFS3_MAGIC: u64 = 0x3353_4645_4C50_5255; // "PURPLEFS3"
pub const PFS3_BLOCK_SIZE: usize = 4096;
pub const PFS3_ROOT_INODE: u64 = 1;

static E1000_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/e1000.apdrv");
static STORAGE_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/storage.apdrv");
static XHCI_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/xhci.apdrv");
static HDAUDIO_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/hdaudio.apdrv");
static BOCHS_GPU_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/bochs_gpu.apdrv");
static R8139_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/r8139.apdrv");

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
        if trimmed == "welcome.txt" || trimmed == "Users/aegis/welcome.txt" {
            Some(2)
        } else if trimmed == "version.txt" {
            Some(3)
        } else if trimmed == "motd" {
            Some(4)
        } else if trimmed == "e1000.apdrv" || trimmed == "System/Drivers/e1000.apdrv" {
            Some(5)
        } else if trimmed == "storage.apdrv" || trimmed == "System/Drivers/storage.apdrv" || trimmed == "System/Drivers/Storage.apdrv" {
            Some(7)
        } else if trimmed == "xhci.apdrv" || trimmed == "System/Drivers/xhci.apdrv" || trimmed == "System/Drivers/usb.apdrv" {
            Some(8)
        } else if trimmed == "hdaudio.apdrv" || trimmed == "System/Drivers/hdaudio.apdrv" || trimmed == "System/Drivers/audio.apdrv" {
            Some(9)
        } else if trimmed == "bochs_gpu.apdrv" || trimmed == "System/Drivers/bochs_gpu.apdrv" || trimmed == "System/Drivers/Graphics.apdrv" {
            Some(10)
        } else if trimmed == "r8139.apdrv" || trimmed == "System/Drivers/r8139.apdrv" {
            Some(11)
        } else {
            None
        }
    }

    pub fn read_file(&mut self, inode: u64, buf: &mut [u8]) -> usize {
        let content: &[u8] = match inode {
            2 => b"Welcome to Purple OS / Aegis64 Native Filesystem!\n",
            3 => b"Aegis64 Kernel v0.5.0 (Pure Rust Bare-Metal Architecture)\n",
            4 => b"Message of the Day: Aegis64 Pure Rust Operational.\n",
            5 => E1000_BIN,
            7 => STORAGE_BIN,
            8 => XHCI_BIN,
            9 => HDAUDIO_BIN,
            10 => BOCHS_GPU_BIN,
            11 => R8139_BIN,
            _ => return 0,
        };
        let len = content.len().min(buf.len());
        buf[0..len].copy_from_slice(&content[0..len]);
        len
    }
}
