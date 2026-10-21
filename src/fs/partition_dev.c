#include "partition_dev.h"
#include "string.h"

static int part_read_block(pfs2_block_dev_t *dev, uint32_t block_num, void *buf) {
    if (!dev || !buf) return PART_DEV_ERR_INVALID;
    partition_dev_t *pdev = (partition_dev_t *)dev->priv;
    if (!pdev || !pdev->bdev) return PART_DEV_ERR_INVALID;

    // 1. Validate partition-relative block number against capacity
    if (block_num >= dev->block_count) {
        return PART_DEV_ERR_BOUNDS;
    }

    // 2. Check arithmetic overflow before adding partition offset
    uint64_t rel_sector = (uint64_t)block_num * pdev->sectors_per_block;
    if (rel_sector / pdev->sectors_per_block != block_num) {
        return PART_DEV_ERR_OVERFLOW;
    }

    uint64_t abs_lba = pdev->start_lba + rel_sector;
    if (abs_lba < pdev->start_lba) {
        return PART_DEV_ERR_OVERFLOW;
    }

    // 3. Ensure the entire request lies strictly inside partition bounds
    uint64_t end_req_lba = abs_lba + (pdev->sectors_per_block - 1);
    if (end_req_lba > pdev->end_lba) {
        return PART_DEV_ERR_BOUNDS;
    }

    // 4. Ensure request lies inside underlying physical disk
    if (end_req_lba >= pdev->bdev->total_blocks) {
        return PART_DEV_ERR_BOUNDS;
    }

    // 5. Read sectors from underlying block device
    int rc = block_dev_read(pdev->bdev, abs_lba, pdev->sectors_per_block, buf);
    if (rc != BLOCK_DEV_OK) {
        return PART_DEV_ERR_IO;
    }

    return PART_DEV_OK;
}

static int part_write_block(pfs2_block_dev_t *dev, uint32_t block_num, const void *buf) {
    if (!dev || !buf) return PART_DEV_ERR_INVALID;
    partition_dev_t *pdev = (partition_dev_t *)dev->priv;
    if (!pdev || !pdev->bdev) return PART_DEV_ERR_INVALID;

    if (pdev->read_only) {
        return PART_DEV_ERR_ROFS;
    }

    if (block_num >= dev->block_count) {
        return PART_DEV_ERR_BOUNDS;
    }

    uint64_t rel_sector = (uint64_t)block_num * pdev->sectors_per_block;
    if (rel_sector / pdev->sectors_per_block != block_num) {
        return PART_DEV_ERR_OVERFLOW;
    }

    uint64_t abs_lba = pdev->start_lba + rel_sector;
    if (abs_lba < pdev->start_lba) {
        return PART_DEV_ERR_OVERFLOW;
    }

    uint64_t end_req_lba = abs_lba + (pdev->sectors_per_block - 1);
    if (end_req_lba > pdev->end_lba || end_req_lba >= pdev->bdev->total_blocks) {
        return PART_DEV_ERR_BOUNDS;
    }

    int rc = block_dev_write(pdev->bdev, abs_lba, pdev->sectors_per_block, buf);
    if (rc != BLOCK_DEV_OK) {
        return PART_DEV_ERR_IO;
    }

    return PART_DEV_OK;
}

static int part_flush(pfs2_block_dev_t *dev) {
    if (!dev) return PART_DEV_ERR_INVALID;
    partition_dev_t *pdev = (partition_dev_t *)dev->priv;
    if (!pdev || !pdev->bdev) return PART_DEV_ERR_INVALID;

    if (pdev->read_only) {
        return PART_DEV_OK;
    }

    return (block_dev_flush(pdev->bdev) == BLOCK_DEV_OK) ? PART_DEV_OK : PART_DEV_ERR_IO;
}

int partition_dev_init_bdev(partition_dev_t *pdev, block_device_t *bdev, const gpt_partition_t *part) {
    if (!pdev || !bdev || !part) return PART_DEV_ERR_INVALID;
    if (bdev->block_size == 0) return PART_DEV_ERR_INVALID;

    if (PFS2_BLOCK_SIZE % bdev->block_size != 0) {
        return PART_DEV_ERR_INVALID; // Unaligned sector size
    }

    uint32_t sectors_per_block = PFS2_BLOCK_SIZE / bdev->block_size;

    // Check bounds
    if (part->start_lba > part->end_lba || part->end_lba >= bdev->total_blocks) {
        return PART_DEV_ERR_BOUNDS;
    }

    uint64_t total_sectors = part->end_lba - part->start_lba + 1;
    if (total_sectors < sectors_per_block) {
        return PART_DEV_ERR_BOUNDS; // Partition smaller than 1 filesystem block
    }

    uint64_t total_blocks = total_sectors / sectors_per_block;
    if (total_blocks > 0xFFFFFFFFULL) {
        return PART_DEV_ERR_OVERFLOW;
    }

    pdev->bdev = bdev;
    pdev->start_lba = part->start_lba;
    pdev->end_lba = part->end_lba;
    pdev->total_sectors = total_sectors;
    pdev->sectors_per_block = sectors_per_block;
    pdev->read_only = 1;

    pdev->base.block_count = (uint32_t)total_blocks;
    pdev->base.block_size = PFS2_BLOCK_SIZE;
    pdev->base.read_block = part_read_block;
    pdev->base.write_block = part_write_block;
    pdev->base.flush = part_flush;
    pdev->base.priv = pdev;

    return PART_DEV_OK;
}

int partition_dev_init(partition_dev_t *pdev, ata_device_t *ata_dev, const gpt_partition_t *part) {
    if (!pdev || !ata_dev || !part) return PART_DEV_ERR_INVALID;
    if (!ata_dev->is_present) return PART_DEV_ERR_INVALID;
    if (ata_dev->logical_sector_size == 0) return PART_DEV_ERR_INVALID;

    memset(pdev, 0, sizeof(*pdev));
    ata_bind_block_dev(ata_dev, &pdev->embedded_bdev, "ata_part");
    int rc = partition_dev_init_bdev(pdev, &pdev->embedded_bdev, part);
    if (rc == PART_DEV_OK) {
        pdev->ata_dev = ata_dev;
    }
    return rc;
}

void partition_dev_set_writable(partition_dev_t *pdev, int writable) {
    if (pdev) {
        pdev->read_only = writable ? 0 : 1;
    }
}

int partition_dev_is_writable(const partition_dev_t *pdev) {
    return (pdev && !pdev->read_only);
}
