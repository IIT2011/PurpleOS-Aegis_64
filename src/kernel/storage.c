#include "kernel_storage.h"
#include "serial.h"
#include "string.h"

kernel_storage_t g_kernel_storage;

extern uint64_t boot_pd_3[];
static void check_pd3_s(const char *where) {
    if (boot_pd_3[503] == 0) {
        kerror("STORAGE: boot_pd_3[503] wiped at %s!", where);
    }
}

void storage_init(void) {
    check_pd3_s("storage_init entry");
    memset(&g_kernel_storage, 0, sizeof(g_kernel_storage));
    g_kernel_storage.active_device_idx = -1;
    g_kernel_storage.active_part_idx = -1;

    kinfo("Storage: Scanning legacy ATA channels (Primary & Secondary)...");

    uint8_t channels[4] = { ATA_CHANNEL_PRIMARY, ATA_CHANNEL_PRIMARY, ATA_CHANNEL_SECONDARY, ATA_CHANNEL_SECONDARY };
    uint8_t drives[4]   = { ATA_DRIVE_MASTER,    ATA_DRIVE_SLAVE,     ATA_DRIVE_MASTER,      ATA_DRIVE_SLAVE };
    const char *names[4] = { "Primary Master", "Primary Slave", "Secondary Master", "Secondary Slave" };

    int found_count = 0;

    for (int i = 0; i < 4; i++) {
        ata_device_t *dev = &g_kernel_storage.devices[i];
        ata_device_init(dev, channels[i], drives[i], ata_get_default_ops());

        int id_rc = ata_identify(dev);
        check_pd3_s("after ata_identify");
        if (id_rc == ATA_OK && dev->is_present) {
            found_count++;
            uint32_t size_mib = (uint32_t)(dev->total_sectors * 512 / (1024 * 1024));
            kinfo("Storage: [%s] Drive detected: Model='%s', Serial='%s', Sectors=%u (%u MiB)",
                  names[i], dev->model, dev->serial, (uint32_t)dev->total_sectors, size_mib);

            // Attempt GPT parse
            gpt_info_t *gpt = &g_kernel_storage.gpt[i];
            int gpt_rc = gpt_parse(dev, gpt);
            check_pd3_s("after gpt_parse");
            if (gpt_rc == GPT_OK && gpt->valid_header && gpt->partition_count > 0) {
                kinfo("Storage: [%s] Valid GPT detected with %u partition(s)", names[i], gpt->partition_count);

                for (uint32_t p = 0; p < gpt->partition_count; p++) {
                    const gpt_partition_t *part = &gpt->partitions[p];
                    kinfo("Storage:   Part %u: '%s' [%u..%u, %u sectors] PurpleFS=%d",
                          p, part->name, (uint32_t)part->start_lba, (uint32_t)part->end_lba,
                          (uint32_t)part->sector_count, part->is_purplefs);
                }

                // Attempt to mount PurpleFS v2 partition if not already mounted
                if (!g_kernel_storage.fs_mounted && gpt->purplefs_part_index >= 0) {
                    uint32_t pidx = (uint32_t)gpt->purplefs_part_index;
                    kinfo("Storage: Found PurpleFS v2 partition at index %u on [%s]", pidx, names[i]);

                    int pinit = partition_dev_init(&g_kernel_storage.part_dev, dev, &gpt->partitions[pidx]);
                    check_pd3_s("after partition_dev_init");
                    if (pinit == PART_DEV_OK) {
                        int mrc = pfs2_mount(&g_kernel_storage.fs, &g_kernel_storage.part_dev.base);
                        check_pd3_s("after pfs2_mount");
                        if (mrc == PFS2_OK) {
                            g_kernel_storage.fs_mounted = 1;
                            g_kernel_storage.active_device_idx = i;
                            g_kernel_storage.active_part_idx = (int)pidx;
                            kinfo("PurpleFS: Mounted volume '%s' (%u blocks, %u free, read-only)",
                                  g_kernel_storage.fs.sb.volume_label,
                                  g_kernel_storage.fs.sb.total_blocks,
                                  g_kernel_storage.fs.sb.free_blocks);
                        } else {
                            kwarn("PurpleFS: Mount failed on partition %u (error code %d). Volume not formatted or corrupted.", pidx, mrc);
                        }
                    } else {
                        kwarn("Storage: Failed to initialize partition device (code %d)", pinit);
                    }
                }
            } else {
                kinfo("Storage: [%s] No valid GPT found (rc=%d)", names[i], gpt_rc);
            }
        }
    }

    g_kernel_storage.devices_count = found_count;

    if (found_count == 0) {
        kinfo("Storage: No ATA storage devices detected on primary or secondary buses");
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
