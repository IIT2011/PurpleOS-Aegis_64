#include "kernel_storage.h"
#include "serial.h"
#include "string.h"

kernel_storage_t g_kernel_storage;

void storage_init(void) {
    memset(&g_kernel_storage, 0, sizeof(g_kernel_storage));
    g_kernel_storage.active_device_idx = -1;
    g_kernel_storage.active_part_idx = -1;

    // 1. Initialize Unified Block Device Registry
    block_dev_init();

    // 2. Scan Legacy ATA channels (Primary & Secondary)
    kinfo("Storage: Scanning legacy ATA channels (Primary & Secondary)...");

    uint8_t channels[4] = { ATA_CHANNEL_PRIMARY, ATA_CHANNEL_PRIMARY, ATA_CHANNEL_SECONDARY, ATA_CHANNEL_SECONDARY };
    uint8_t drives[4]   = { ATA_DRIVE_MASTER,    ATA_DRIVE_SLAVE,     ATA_DRIVE_MASTER,      ATA_DRIVE_SLAVE };
    const char *names[4] = { "ata0", "ata1", "ata2", "ata3" };
    const char *desc[4]  = { "Primary Master", "Primary Slave", "Secondary Master", "Secondary Slave" };

    int found_ata = 0;

    for (int i = 0; i < 4; i++) {
        ata_device_t *dev = &g_kernel_storage.devices[i];
        ata_device_init(dev, channels[i], drives[i], ata_get_default_ops());

        int id_rc = ata_identify(dev);
        if (id_rc == ATA_OK && dev->is_present) {
            found_ata++;
            uint32_t size_mib = (uint32_t)(dev->total_sectors * 512 / (1024 * 1024));
            kinfo("Storage: [%s - %s] Drive detected: Model='%s', Serial='%s', Sectors=%u (%u MiB)",
                  names[i], desc[i], dev->model, dev->serial, (uint32_t)dev->total_sectors, size_mib);

            // Register into block device registry
            block_device_t *bdev = &g_kernel_storage.ata_bdevs[i];
            ata_bind_block_dev(dev, bdev, names[i]);
            block_dev_register(bdev);
        }
    }
    g_kernel_storage.devices_count = found_ata;

    // 3. Scan & Initialize AHCI SATA controllers
    kinfo("Storage: Scanning for AHCI / SATA storage controllers...");
    ahci_init();

    // 4. Scan & Initialize NVMe PCIe controllers
    kinfo("Storage: Scanning for NVMe PCIe storage controllers...");
    nvme_init();

    // 5. Partition Table & Filesystem Discovery across all detected block devices
    uint32_t bdev_total = block_dev_count();
    kinfo("Storage: Probing %u registered block device(s) for GPT partitions...", bdev_total);

    for (uint32_t i = 0; i < bdev_total && i < MAX_BLOCK_DEVICES; i++) {
        block_device_t *bdev = block_dev_get_by_index(i);
        if (!bdev) continue;

        gpt_info_t *gpt = &g_kernel_storage.gpt[i];
        int gpt_rc = gpt_parse_block_dev(bdev, gpt);
        if (gpt_rc == GPT_OK && gpt->valid_header && gpt->partition_count > 0) {
            kinfo("Storage: [%s] Valid GPT detected with %u partition(s)", bdev->name, gpt->partition_count);

            for (uint32_t p = 0; p < gpt->partition_count; p++) {
                const gpt_partition_t *part = &gpt->partitions[p];
                kinfo("Storage:   Part %u: '%s' [%u..%u, %u sectors] PurpleFS=%d",
                      p, part->name, (uint32_t)part->start_lba, (uint32_t)part->end_lba,
                      (uint32_t)part->sector_count, part->is_purplefs);
            }

            // Attempt to mount PurpleFS v2 partition if not already mounted
            if (!g_kernel_storage.fs_mounted && gpt->purplefs_part_index >= 0) {
                uint32_t pidx = (uint32_t)gpt->purplefs_part_index;
                kinfo("Storage: Found PurpleFS v2 partition at index %u on [%s]", pidx, bdev->name);

                int pinit = partition_dev_init_bdev(&g_kernel_storage.part_dev, bdev, &gpt->partitions[pidx]);
                if (pinit == PART_DEV_OK) {
                    int mrc = pfs2_mount(&g_kernel_storage.fs, &g_kernel_storage.part_dev.base);
                    if (mrc == PFS2_OK) {
                        g_kernel_storage.fs_mounted = 1;
                        g_kernel_storage.active_device_idx = (int)i;
                        g_kernel_storage.active_part_idx = (int)pidx;
                        kinfo("PurpleFS: Mounted volume '%s' (%u blocks, %u free, read-only) from [%s]",
                              g_kernel_storage.fs.sb.volume_label,
                              g_kernel_storage.fs.sb.total_blocks,
                              g_kernel_storage.fs.sb.free_blocks,
                              bdev->name);
                    } else {
                        kwarn("PurpleFS: Mount failed on partition %u of [%s] (code %d)", pidx, bdev->name, mrc);
                    }
                } else {
                    kwarn("Storage: Failed to initialize partition device for [%s] (code %d)", bdev->name, pinit);
                }
            }
        } else {
            kinfo("Storage: [%s] No valid GPT found (rc=%d)", bdev->name, gpt_rc);
        }
    }

    if (bdev_total == 0) {
        kinfo("Storage: No storage devices detected on any bus (ATA, SATA, NVMe)");
    } else if (!g_kernel_storage.fs_mounted) {
        kinfo("Storage: No mountable PurpleFS v2 partition detected on any device");
    }
}

int storage_is_fs_mounted(void) {
    return g_kernel_storage.fs_mounted;
}

pfs2_fs_t *storage_get_fs(void) {
    return g_kernel_storage.fs_mounted ? &g_kernel_storage.fs : NULL;
}

int storage_is_writable(void) {
    if (!g_kernel_storage.fs_mounted) return 0;
    return partition_dev_is_writable(&g_kernel_storage.part_dev) && !g_kernel_storage.fs.read_only;
}

int storage_set_writable(int writable) {
    if (!g_kernel_storage.fs_mounted) return -1;
    partition_dev_set_writable(&g_kernel_storage.part_dev, writable);
    g_kernel_storage.fs.read_only = writable ? 0 : 1;
    return 0;
}
