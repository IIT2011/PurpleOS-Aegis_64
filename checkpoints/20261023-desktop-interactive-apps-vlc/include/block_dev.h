#ifndef BLOCK_DEV_H
#define BLOCK_DEV_H

#include "types.h"

#define BLOCK_DEV_OK                 0
#define BLOCK_DEV_ERR_IO            -1
#define BLOCK_DEV_ERR_BOUNDS        -2
#define BLOCK_DEV_ERR_ROFS          -3
#define BLOCK_DEV_ERR_BUSY          -4
#define BLOCK_DEV_ERR_TIMEOUT       -5
#define BLOCK_DEV_ERR_INVALID       -6
#define BLOCK_DEV_ERR_NOMEM         -7

#define MAX_BLOCK_DEVICES           16

typedef enum {
    BLOCK_DEV_TYPE_UNKNOWN = 0,
    BLOCK_DEV_TYPE_ATA     = 1,
    BLOCK_DEV_TYPE_AHCI    = 2,
    BLOCK_DEV_TYPE_NVME    = 3,
    BLOCK_DEV_TYPE_RAMDISK = 4
} block_dev_type_t;

#define BLOCK_DEV_FLAG_READONLY   (1U << 0)
#define BLOCK_DEV_FLAG_REMOVABLE  (1U << 1)
#define BLOCK_DEV_FLAG_ONLINE     (1U << 2)

struct block_device;

typedef struct block_device_ops {
    int (*read_blocks)(struct block_device *dev, uint64_t lba, uint32_t count, void *buf);
    int (*write_blocks)(struct block_device *dev, uint64_t lba, uint32_t count, const void *buf);
    int (*flush)(struct block_device *dev);
} block_device_ops_t;

typedef struct block_device {
    char                    name[32];
    char                    model[48];
    char                    serial[32];
    block_dev_type_t        type;
    uint32_t                block_size;     /* Usually 512 or 4096 */
    uint64_t                total_blocks;   /* Total addressable blocks */
    uint32_t                flags;
    uint32_t                id;
    const block_device_ops_t *ops;
    void                    *priv;          /* Driver-specific controller/drive instance */
} block_device_t;

/* Management & Discovery API */
int  block_dev_init(void);
void block_dev_reset(void);
int  block_dev_register(block_device_t *dev);
int  block_dev_unregister(block_device_t *dev);
uint32_t block_dev_count(void);
block_device_t *block_dev_get_by_name(const char *name);
block_device_t *block_dev_get_by_index(uint32_t index);
const char *block_dev_type_to_str(block_dev_type_t type);

/* Unified Block I/O Operations */
int block_dev_read(block_device_t *dev, uint64_t lba, uint32_t count, void *buf);
int block_dev_write(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf);
int block_dev_flush(block_device_t *dev);

#endif /* BLOCK_DEV_H */
