//! Central Filesystem Driver Registry & Management Subsystem
//!
//! Maintains registration, capability introspection, partition probing,
//! and PurpleDetect hardware daemon integration for all supported filesystem drivers:
//!   - PurpleFS v2 & v3 (Native 64-bit Journaled Engine)
//!   - FAT12 / FAT16 / FAT32 (Removable Flash, SDXC, UEFI ESP)
//!   - Ext2 / Ext3 / Ext4 (Linux Compatibility Stack)
//!   - ISO9660 / El Torito (Optical CD-ROM / DVD Media)
//!   - RAMFS / TmpFS (In-Memory Ephemeral High-Speed Scratch)
//!   - DevFS & ProcFS (Synthetic Device & Telemetry Filesystems)

use crate::drivers::block_dev::BlockDevice;
use crate::drivers::purple_detect::{self, DeviceCategory, DeviceState};
use crate::drivers::serial;
use crate::fs::ext2::ExtDriver;
use crate::fs::fat::FatDriver;
use crate::fs::iso9660::Iso9660Driver;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FsDriverId {
    PurpleFsV2,
    PurpleFsV3,
    Fat12,
    Fat16,
    Fat32,
    Ext2,
    Ext3,
    Ext4,
    Iso9660,
    RamFs,
    DevFs,
    ProcFs,
}

pub struct FsDriverEntry {
    pub id: FsDriverId,
    pub name: &'static str,
    pub category: &'static str,
    pub vendor: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub active: bool,
    pub supports_write: bool,
    pub supports_journal: bool,
}

pub const FS_DRIVERS: &[FsDriverEntry] = &[
    FsDriverEntry {
        id: FsDriverId::PurpleFsV2,
        name: "purplefs_v2",
        category: "NATIVE",
        vendor: "Purple OS",
        description: "PurpleFS v2 read-only filesystem reader",
        version: "2.1.0",
        active: true,
        supports_write: false,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::PurpleFsV3,
        name: "purplefs_v3",
        category: "NATIVE",
        vendor: "Purple OS",
        description: "PurpleFS v3 is a design-only format; no mount implementation is available",
        version: "3.0.0",
        active: false,
        supports_write: false,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::Fat32,
        name: "fat32",
        category: "REMOVABLE",
        vendor: "Microsoft/Std",
        description: "FAT32 / VFAT Removable Media & ESP Filesystem",
        version: "1.2.0",
        active: false,
        supports_write: true,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::Fat16,
        name: "fat16",
        category: "REMOVABLE",
        vendor: "Microsoft/Std",
        description: "FAT16 Removable Flash Media Driver",
        version: "1.0.0",
        active: false,
        supports_write: true,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::Fat12,
        name: "fat12",
        category: "LEGACY",
        vendor: "Microsoft/Std",
        description: "FAT12 Legacy Floppy & Small Flash Driver",
        version: "1.0.0",
        active: false,
        supports_write: false,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::Ext4,
        name: "ext4",
        category: "COMPAT",
        vendor: "Linux Foundation",
        description: "Ext4 Extent-Based High-Capacity Linux Filesystem",
        version: "4.0.0",
        active: false,
        supports_write: true,
        supports_journal: true,
    },
    FsDriverEntry {
        id: FsDriverId::Ext3,
        name: "ext3",
        category: "COMPAT",
        vendor: "Linux Foundation",
        description: "Ext3 Journaled Linux Filesystem Driver",
        version: "3.0.0",
        active: false,
        supports_write: true,
        supports_journal: true,
    },
    FsDriverEntry {
        id: FsDriverId::Ext2,
        name: "ext2",
        category: "COMPAT",
        vendor: "Linux Foundation",
        description: "Ext2 Standard Second Extended Linux Filesystem",
        version: "2.0.0",
        active: false,
        supports_write: true,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::Iso9660,
        name: "iso9660",
        category: "OPTICAL",
        vendor: "ISO/ECMA",
        description: "ISO9660 / El Torito Optical CD-ROM & DVD Media",
        version: "1.0.0",
        active: false,
        supports_write: false,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::RamFs,
        name: "ramfs",
        category: "VIRTUAL",
        vendor: "Purple OS",
        description: "RAMFS / TmpFS Volatile Virtual Memory Filesystem",
        version: "1.1.0",
        active: true,
        supports_write: true,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::DevFs,
        name: "devfs",
        category: "SYNTHETIC",
        vendor: "Purple OS",
        description: "DevFS Dynamic Hardware & Character Device Nodes (/dev)",
        version: "2.0.0",
        active: true,
        supports_write: true,
        supports_journal: false,
    },
    FsDriverEntry {
        id: FsDriverId::ProcFs,
        name: "procfs",
        category: "SYNTHETIC",
        vendor: "Purple OS",
        description: "ProcFS Live Kernel Introspection & Telemetry (/proc)",
        version: "2.0.0",
        active: true,
        supports_write: false,
        supports_journal: false,
    },
];

pub fn init() {
    serial::println(
        "FS: Filesystem driver registry initialized; unsupported formats remain inactive",
    );
}

/// Link all registered filesystem drivers directly to the PurpleDetect device hierarchy
pub fn register_with_purple_detect() {
    for drv in FS_DRIVERS.iter() {
        let state = if drv.active {
            DeviceState::Active
        } else {
            DeviceState::Unsupported
        };
        purple_detect::register(
            DeviceCategory::Filesystem,
            drv.name,
            drv.vendor,
            drv.description,
            "VFS",
            0,
            0,
            state,
        );
    }
}

/// Identify a filesystem format on a block device; this does not mount it.
pub fn probe_filesystem(dev: &mut dyn BlockDevice, start_lba: u64) -> Option<&'static str> {
    // 1. Probe PurpleFS v2 / v3
    let mut sb = [0u8; 512];
    if dev.read_blocks(start_lba, 1, &mut sb) == 0 {
        if &sb[0..8] == b"PURPLEFS" {
            let ver = u32::from_le_bytes([sb[16], sb[17], sb[18], sb[19]]);
            if ver == 2 {
                return Some("purplefs_v2");
            } else if ver == 3 {
                return Some("purplefs_v3");
            }
            return Some("purplefs");
        }
    }

    // 2. Probe FAT Family
    let mut fat = FatDriver::new();
    if let Some(fat_type) = fat.probe(dev, start_lba) {
        return Some(fat_type.as_str());
    }

    // 3. Probe Ext Family
    let mut ext = ExtDriver::new();
    if let Some(ext_type) = ext.probe(dev, start_lba) {
        return Some(ext_type.as_str());
    }

    // 4. Probe ISO9660 Optical
    let mut iso = Iso9660Driver::new();
    if iso.probe(dev, start_lba) {
        return Some("iso9660");
    }

    None
}

/// Display a comprehensive table of all registered filesystem drivers
pub fn print_driver_table() {
    serial::println("\n=== Purple OS Registered Filesystem Drivers (filesystems.apdrv) ===");
    serial::println(
        "  DRIVER          CATEGORY   VERSION  FEATURES                 VENDOR / DESCRIPTION",
    );
    serial::println("  -----------------------------------------------------------------------------------------");

    for drv in FS_DRIVERS.iter() {
        serial::print("  ");
        serial::print(drv.name);
        for _ in drv.name.len()..16 {
            serial::print(" ");
        }

        serial::print(drv.category);
        for _ in drv.category.len()..11 {
            serial::print(" ");
        }

        serial::print(drv.version);
        for _ in drv.version.len()..9 {
            serial::print(" ");
        }

        if !drv.active {
            serial::print("UNAVAILABLE             ");
        } else if drv.supports_write && drv.supports_journal {
            serial::print("RW, JOURNAL, POSIX       ");
        } else if drv.supports_write {
            serial::print("RW, POSIX                ");
        } else {
            serial::print("RO, READ-ONLY            ");
        }

        serial::print(drv.vendor);
        serial::print(" - ");
        serial::println(drv.description);
    }
    serial::println("  -----------------------------------------------------------------------------------------\n");
}
