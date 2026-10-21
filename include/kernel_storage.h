#ifndef KERNEL_STORAGE_H
#define KERNEL_STORAGE_H

#include "types.h"
#include "block_dev.h"
#include "ata_pio.h"
#include "ahci.h"
#include "nvme.h"
#include "gpt.h"
#include "partition_dev.h"
#include "purplefs_v2.h"

#define MAX_ATA_DEVICES 4

typedef struct kernel_storage {
    ata_device_t    devices[MAX_ATA_DEVICES];
    block_device_t  ata_bdevs[MAX_ATA_DEVICES];
    gpt_info_t      gpt[MAX_BLOCK_DEVICES];
    partition_dev_t part_dev;
    pfs2_fs_t       fs;
    int             devices_count;
    int             fs_mounted;
    int             active_device_idx;
    int             active_part_idx;
} kernel_storage_t;

extern kernel_storage_t g_kernel_storage;

void storage_init(void);
int storage_is_fs_mounted(void);
pfs2_fs_t *storage_get_fs(void);
int storage_is_writable(void);
int storage_set_writable(int writable);

#endif /* KERNEL_STORAGE_H */
