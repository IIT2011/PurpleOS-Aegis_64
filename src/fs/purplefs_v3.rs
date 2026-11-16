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
static GPU_INTEL_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/gpu-intel.apdrv");
static GPU_AMD_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/gpu-amd.apdrv");
static GPU_NVIDIA_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/gpu-nvidia.apdrv");
static BLUETOOTH_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/bluetooth.apdrv");
static CAMERA_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/camera.apdrv");
static INPUT_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/input.apdrv");
static WIFI_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/wifi.apdrv");
static NETWORK_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/network.apdrv");
static GRAPHICS_BIN: &[u8] = include_bytes!("../../build/drivers_apdrv/Graphics.apdrv");

#[allow(dead_code)]
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
        if trimmed == "welcome.txt" || trimmed == "Users/aegis/welcome.txt" || trimmed == "System/Users/aegis/welcome.txt" {
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
        } else if trimmed == "bochs_gpu.apdrv" || trimmed == "System/Drivers/bochs_gpu.apdrv" {
            Some(10)
        } else if trimmed == "r8139.apdrv" || trimmed == "System/Drivers/r8139.apdrv" {
            Some(11)
        } else if trimmed == "gpu-intel.apdrv" || trimmed == "System/Drivers/gpu-intel.apdrv" {
            Some(12)
        } else if trimmed == "gpu-amd.apdrv" || trimmed == "System/Drivers/gpu-amd.apdrv" {
            Some(13)
        } else if trimmed == "gpu-nvidia.apdrv" || trimmed == "System/Drivers/gpu-nvidia.apdrv" {
            Some(14)
        } else if trimmed == "bluetooth.apdrv" || trimmed == "System/Drivers/bluetooth.apdrv" {
            Some(15)
        } else if trimmed == "camera.apdrv" || trimmed == "System/Drivers/camera.apdrv" {
            Some(16)
        } else if trimmed == "input.apdrv" || trimmed == "System/Drivers/input.apdrv" {
            Some(17)
        } else if trimmed == "wifi.apdrv" || trimmed == "System/Drivers/wifi.apdrv" {
            Some(18)
        } else if trimmed == "network.apdrv" || trimmed == "System/Drivers/network.apdrv" {
            Some(19)
        } else if trimmed == "Graphics.apdrv" || trimmed == "System/Drivers/Graphics.apdrv" {
            Some(20)
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
            12 => GPU_INTEL_BIN,
            13 => GPU_AMD_BIN,
            14 => GPU_NVIDIA_BIN,
            15 => BLUETOOTH_BIN,
            16 => CAMERA_BIN,
            17 => INPUT_BIN,
            18 => WIFI_BIN,
            19 => NETWORK_BIN,
            20 => GRAPHICS_BIN,
            _ => return 0,
        };
        let len = content.len().min(buf.len());
        buf[0..len].copy_from_slice(&content[0..len]);
        len
    }
}
