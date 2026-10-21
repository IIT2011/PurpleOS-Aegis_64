#include "block_dev.h"
#include "string.h"
#include "serial.h"

static block_device_t *g_devices[MAX_BLOCK_DEVICES];
static uint32_t g_device_count = 0;
static uint32_t g_next_id = 1;

int block_dev_init(void) {
    memset(g_devices, 0, sizeof(g_devices));
    g_device_count = 0;
    g_next_id = 1;
    return BLOCK_DEV_OK;
}

void block_dev_reset(void) {
    memset(g_devices, 0, sizeof(g_devices));
    g_device_count = 0;
    g_next_id = 1;
}

int block_dev_register(block_device_t *dev) {
    if (!dev || !dev->ops || dev->block_size == 0 || dev->total_blocks == 0) {
        return BLOCK_DEV_ERR_INVALID;
    }
    if (g_device_count >= MAX_BLOCK_DEVICES) {
        return BLOCK_DEV_ERR_NOMEM;
    }

    // Check if name already registered
    for (uint32_t i = 0; i < g_device_count; i++) {
        if (g_devices[i] && strcmp(g_devices[i]->name, dev->name) == 0) {
            return BLOCK_DEV_ERR_INVALID;
        }
    }

    dev->id = g_next_id++;
    dev->flags |= BLOCK_DEV_FLAG_ONLINE;
    g_devices[g_device_count++] = dev;

    kinfo("BlockDev: Registered '%s' [%s] id=%u, size=%u bytes/blk, total_blocks=%u (%u MiB)",
          dev->name, block_dev_type_to_str(dev->type), dev->id,
          dev->block_size, (uint32_t)dev->total_blocks,
          (uint32_t)((dev->total_blocks * dev->block_size) / (1024 * 1024)));

    return BLOCK_DEV_OK;
}

int block_dev_unregister(block_device_t *dev) {
    if (!dev) return BLOCK_DEV_ERR_INVALID;

    for (uint32_t i = 0; i < g_device_count; i++) {
        if (g_devices[i] == dev) {
            kinfo("BlockDev: Unregistered '%s' id=%u", dev->name, dev->id);
            dev->flags &= ~BLOCK_DEV_FLAG_ONLINE;
            // Shift remaining down
            for (uint32_t j = i; j < g_device_count - 1; j++) {
                g_devices[j] = g_devices[j + 1];
            }
            g_devices[--g_device_count] = NULL;
            return BLOCK_DEV_OK;
        }
    }

    return BLOCK_DEV_ERR_INVALID;
}

uint32_t block_dev_count(void) {
    return g_device_count;
}

block_device_t *block_dev_get_by_name(const char *name) {
    if (!name) return NULL;
    for (uint32_t i = 0; i < g_device_count; i++) {
        if (g_devices[i] && strcmp(g_devices[i]->name, name) == 0) {
            return g_devices[i];
        }
    }
    return NULL;
}

block_device_t *block_dev_get_by_index(uint32_t index) {
    if (index >= g_device_count) return NULL;
    return g_devices[index];
}

const char *block_dev_type_to_str(block_dev_type_t type) {
    switch (type) {
        case BLOCK_DEV_TYPE_ATA:     return "ATA/IDE";
        case BLOCK_DEV_TYPE_AHCI:    return "AHCI/SATA";
        case BLOCK_DEV_TYPE_NVME:    return "NVMe PCIe";
        case BLOCK_DEV_TYPE_RAMDISK: return "RAM Disk";
        default:                     return "Unknown";
    }
}

int block_dev_read(block_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    if (!dev || !dev->ops || !dev->ops->read_blocks || !buf) {
        return BLOCK_DEV_ERR_INVALID;
    }
    if (count == 0) return BLOCK_DEV_OK;

    // Bounds checking
    if (lba >= dev->total_blocks || lba + count > dev->total_blocks || lba + count < lba) {
        return BLOCK_DEV_ERR_BOUNDS;
    }

    return dev->ops->read_blocks(dev, lba, count, buf);
}

int block_dev_write(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    if (!dev || !dev->ops || !dev->ops->write_blocks || !buf) {
        return BLOCK_DEV_ERR_INVALID;
    }
    if (dev->flags & BLOCK_DEV_FLAG_READONLY) {
        return BLOCK_DEV_ERR_ROFS;
    }
    if (count == 0) return BLOCK_DEV_OK;

    // Bounds checking
    if (lba >= dev->total_blocks || lba + count > dev->total_blocks || lba + count < lba) {
        return BLOCK_DEV_ERR_BOUNDS;
    }

    return dev->ops->write_blocks(dev, lba, count, buf);
}

int block_dev_flush(block_device_t *dev) {
    if (!dev || !dev->ops) {
        return BLOCK_DEV_ERR_INVALID;
    }
    if (dev->ops->flush) {
        return dev->ops->flush(dev);
    }
    return BLOCK_DEV_OK;
}
