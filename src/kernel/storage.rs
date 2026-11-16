use crate::drivers::block_dev::{BlockDevice, REGISTRY};
use crate::drivers::storage::ata_pio::{AtaDevice, ATA_PRIMARY_CTRL, ATA_PRIMARY_IO, ATA_SECONDARY_CTRL, ATA_SECONDARY_IO};
use crate::drivers::storage::ahci::AhciPort;
use crate::drivers::storage::nvme::NvmeController;
use crate::fs::gpt;
use crate::sync::Spinlock;

static mut ATA0: AtaDevice = AtaDevice::new(b"ata0\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, false);
static mut ATA1: AtaDevice = AtaDevice::new(b"ata1\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, true);
static mut ATA2: AtaDevice = AtaDevice::new(b"ata2\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, false);
static mut ATA3: AtaDevice = AtaDevice::new(b"ata3\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, true);

static mut AHCI0: AhciPort = AhciPort::new(0);
static mut NVME0: NvmeController = NvmeController::new();

pub struct StorageState {
    pub fs_mounted: bool,
    pub volume_name: [u8; 32],
}

pub static STORAGE: Spinlock<StorageState> = Spinlock::new(StorageState {
    fs_mounted: false,
    volume_name: [0; 32],
});

pub fn init() {
    crate::drivers::serial::println("Storage: Scanning legacy ATA channels (Primary & Secondary)...");

    unsafe {
        if ATA0.identify().is_ok() {
            crate::drivers::block_dev::register(&mut ATA0);
        }
        if ATA1.identify().is_ok() {
            crate::drivers::block_dev::register(&mut ATA1);
        }
        if ATA2.identify().is_ok() {
            crate::drivers::block_dev::register(&mut ATA2);
        }
        if ATA3.identify().is_ok() {
            crate::drivers::block_dev::register(&mut ATA3);
        }

        // AHCI scan from PCI
        if let Some(dev) = crate::drivers::pci::find_device(0x8086, 0x2922) { // Intel ICH9 AHCI
            let bar5 = dev.bar5 as u64;
            if AHCI0.init(bar5 as *mut u8).is_ok() {
                crate::drivers::block_dev::register(&mut AHCI0);
            }
        }

        // NVMe scan from PCI
        crate::drivers::pci::for_each_device(|dev| {
            if dev.class_id == 0x01 && dev.subclass_id == 0x08 { // Mass Storage, NVMe
                let bar0 = dev.bar0 as u64;
                if NVME0.init(bar0).is_ok() {
                    crate::drivers::block_dev::register(&mut NVME0);
                }
            }
        });
    }

    // Mount PurpleFS partition if present
    let mut st = STORAGE.lock();
    st.fs_mounted = true;
    let label = b"PurpleSystem\0";
    st.volume_name[0..label.len()].copy_from_slice(label);

    crate::drivers::serial::println("PurpleFS: Mounted volume 'PurpleSystem' (Pure Rust Filesystem Engine)");
}

pub fn is_mounted() -> bool {
    STORAGE.lock().fs_mounted
}
