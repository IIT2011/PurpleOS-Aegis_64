use crate::drivers::storage::ata_pio::{AtaDevice, ATA_PRIMARY_CTRL, ATA_PRIMARY_IO, ATA_SECONDARY_CTRL, ATA_SECONDARY_IO};
use crate::fs::purplefs_v2::PurpleFsV2;
use crate::sync::Spinlock;

static mut ATA_DEVICES: [AtaDevice; 4] = [
    AtaDevice::new(b"ata0\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, false),
    AtaDevice::new(b"ata1\0", ATA_PRIMARY_IO, ATA_PRIMARY_CTRL, true),
    AtaDevice::new(b"ata2\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, false),
    AtaDevice::new(b"ata3\0", ATA_SECONDARY_IO, ATA_SECONDARY_CTRL, true),
];


pub struct StorageState {
    pub fs_mounted: bool,
    pub read_write: bool,
    pub volume_name: [u8; 32],
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub partition_start_lba: u64,
    pub partition_sector_count: u64,
    pub dev_index: usize,
}

pub static STORAGE: Spinlock<StorageState> = Spinlock::new(StorageState {
    fs_mounted: false,
    read_write: false,
    volume_name: [0; 32],
    total_blocks: 0,
    free_blocks: 0,
    partition_start_lba: 0,
    partition_sector_count: 0,
    dev_index: 0,
});

pub fn init() {
    crate::drivers::serial::println("Storage: Scanning legacy ATA channels (Primary & Secondary)...");

    unsafe {
        let ata_base = core::ptr::addr_of_mut!(ATA_DEVICES) as *mut AtaDevice;
        for i in 0..4 {
            let dev = &mut *ata_base.add(i);
            if dev.identify().is_ok() {
                crate::drivers::serial::print("Drive detected: Model=");
                let model_str = core::str::from_utf8(&dev.model).unwrap_or("ATA Disk").trim();
                crate::drivers::serial::print(model_str);
                crate::drivers::serial::println("");
                crate::drivers::block_dev::register(dev);
            }
        }

        // AHCI, NVMe, USB MSC, and SD block-device discovery are not yet
        // implemented with verified capacity and command completion handling.
        // Keep them out of the registry until they can guarantee real media I/O.
        crate::drivers::serial::println("Storage: AHCI, NVMe, USB MSC, and SD I/O unavailable; placeholders skipped");

    }

    // Dynamic GPT Scan & PurpleFS Auto-Mount across registered block devices
    let dev_count = crate::drivers::block_dev::count();
    let mut mounted = false;

    for d in 0..dev_count {
        let mount_res = crate::drivers::block_dev::with_dev(d, |dev| {
            if let Ok(info) = crate::fs::gpt::parse(dev) {
                crate::drivers::serial::print("Storage: Valid GPT detected with ");
                crate::drivers::serial::print_u64(info.partition_count as u64);
                crate::drivers::serial::println(" partition(s)");

                if let Some(p_idx) = info.purplefs_part_index {
                    crate::drivers::serial::print("Storage: Found PurpleFS v2 partition at index ");
                    crate::drivers::serial::print_u64(p_idx as u64);
                    crate::drivers::serial::println("");

                    let part = &info.partitions[p_idx];
                    let partition_sectors = match part.end_lba.checked_sub(part.start_lba)
                        .and_then(|length| length.checked_add(1)) {
                        Some(length) => length,
                        None => return None,
                    };
                    if let Ok(fs) = PurpleFsV2::mount(dev, part.start_lba, partition_sectors) {
                        return Some((fs.volume_label, fs.total_blocks, fs.free_blocks, part.start_lba, partition_sectors));
                    }
                }
            }
            None
        });

        if let Some(Some((vol_label, tot_blocks, free_blks, start_lba, partition_sectors))) = mount_res {
            let mut st = STORAGE.lock();
            st.fs_mounted = true;
            st.read_write = false;
            st.volume_name = vol_label;
            st.total_blocks = tot_blocks;
            st.free_blocks = free_blks;
            st.partition_start_lba = start_lba;
            st.partition_sector_count = partition_sectors;
            st.dev_index = d;
            mounted = true;

            let name_len = vol_label.iter().position(|&c| c == 0).unwrap_or(vol_label.len());
            let name_str = core::str::from_utf8(&vol_label[..name_len]).unwrap_or("PurpleSystem");
            crate::drivers::serial::print("PurpleFS: Mounted volume '");
            crate::drivers::serial::print(name_str);
            crate::drivers::serial::println("' (Pure Rust Filesystem Engine)");
            break;
        }
    }

    if !mounted {
        let mut st = STORAGE.lock();
        st.fs_mounted = false;
        st.read_write = false;
        crate::drivers::serial::println("Storage: No PurpleFS volume found on attached block devices.");
    }
}

pub fn is_mounted() -> bool {
    STORAGE.lock().fs_mounted
}

pub fn is_read_write() -> bool {
    let st = STORAGE.lock();
    st.fs_mounted && st.read_write
}

pub fn set_read_write(rw: bool) {
    let mut st = STORAGE.lock();
    if st.fs_mounted {
        st.read_write = rw;
    }
}

pub fn with_fs<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut PurpleFsV2) -> R,
{
    let st = STORAGE.lock();
    if !st.fs_mounted {
        return None;
    }
    let dev_idx = st.dev_index;
    let start_lba = st.partition_start_lba;
    let partition_sector_count = st.partition_sector_count;
    let rw = st.read_write;

    let res = crate::drivers::block_dev::with_dev(dev_idx, |dev| {
        if let Ok(mut fs) = PurpleFsV2::mount(dev, start_lba, partition_sector_count) {
            fs.read_only = !rw;
            Some(f(&mut fs))
        } else {
            None
        }
    }).flatten();
    drop(st);
    res
}

pub fn sync() -> Result<(), &'static str> {
    with_fs(|fs| fs.sync()).unwrap_or(Err("No filesystem mounted"))
}
