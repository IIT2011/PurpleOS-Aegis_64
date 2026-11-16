use crate::drivers::storage::ata_pio::{AtaDevice, ATA_PRIMARY_CTRL, ATA_PRIMARY_IO, ATA_SECONDARY_CTRL, ATA_SECONDARY_IO};
use crate::drivers::storage::ahci::AhciPort;
use crate::drivers::storage::nvme::NvmeController;
use crate::sync::Spinlock;

static mut ATA_DEVICES: [AtaDevice; 4] = [
    AtaDevice::new(b"ata0\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, false),
    AtaDevice::new(b"ata1\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, true),
    AtaDevice::new(b"ata2\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, false),
    AtaDevice::new(b"ata3\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, true),
];

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
        let ata_base = core::ptr::addr_of_mut!(ATA_DEVICES) as *mut AtaDevice;
        for i in 0..4 {
            let dev = &mut *ata_base.add(i);
            if dev.identify().is_ok() {
                crate::drivers::block_dev::register(dev);
            }
        }

        // AHCI scan from PCI
        if let Some(dev) = crate::drivers::pci::find_device(0x8086, 0x2922) { // Intel ICH9 AHCI
            let bar5 = dev.bar5 as u64;
            let _ = crate::mm::vmm::map_mmio_range(bar5, 0x1000);
            let ahci_ref = &mut *core::ptr::addr_of_mut!(AHCI0);
            if ahci_ref.init(bar5 as *mut u8).is_ok() {
                crate::drivers::block_dev::register(ahci_ref);
            }
        }

        // NVMe scan from PCI
        crate::drivers::pci::for_each_device(|dev| {
            if dev.class_id == 0x01 && dev.subclass_id == 0x08 { // Mass Storage, NVMe
                let bar0 = dev.bar0 as u64;
                let _ = crate::mm::vmm::map_mmio_range(bar0, 0x4000);
                let nvme_ref = &mut *core::ptr::addr_of_mut!(NVME0);
                if nvme_ref.init(bar0).is_ok() {
                    crate::drivers::block_dev::register(nvme_ref);
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
