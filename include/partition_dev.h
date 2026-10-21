#ifndef PARTITION_DEV_H
#define PARTITION_DEV_H

#include "types.h"
#include "ata_pio.h"
#include "purplefs_v2.h"
#include "gpt.h"

#define PART_DEV_OK                 0
#define PART_DEV_ERR_INVALID       -1
#define PART_DEV_ERR_BOUNDS        -2
#define PART_DEV_ERR_OVERFLOW      -3
#define PART_DEV_ERR_IO            -4
#define PART_DEV_ERR_ROFS          -5

/* Partition-backed block device adapter */
typedef struct partition_dev {
    pfs2_block_dev_t base;              /* Inherited PFS2 block device interface */
    ata_device_t     *ata_dev;          /* Underlying ATA controller / device */
    uint64_t         start_lba;         /* Starting sector LBA (inclusive) */
    uint64_t         end_lba;           /* Ending sector LBA (inclusive) */
    uint64_t         total_sectors;     /* Total sectors allocated to partition */
    uint32_t         sectors_per_block; /* Typically 8 (4096 / 512) */
    int              read_only;         /* 1 = Enforce read-only */
} partition_dev_t;

int partition_dev_init(partition_dev_t *pdev, ata_device_t *ata_dev, const gpt_partition_t *part);
void partition_dev_set_writable(partition_dev_t *pdev, int writable);
int partition_dev_is_writable(const partition_dev_t *pdev);

#endif /* PARTITION_DEV_H */
