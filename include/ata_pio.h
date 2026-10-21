#ifndef ATA_PIO_H
#define ATA_PIO_H

#include "types.h"

/* Legacy ATA I/O Ports */
#define ATA_PRIMARY_IO_BASE        0x1F0
#define ATA_PRIMARY_CTRL_BASE      0x3F6
#define ATA_SECONDARY_IO_BASE      0x170
#define ATA_SECONDARY_CTRL_BASE    0x376

/* ATA Channel and Drive Identifiers */
#define ATA_CHANNEL_PRIMARY        0
#define ATA_CHANNEL_SECONDARY      1
#define ATA_DRIVE_MASTER           0
#define ATA_DRIVE_SLAVE            1

/* ATA Status Register Flags (Offset 7 / Control Offset 0) */
#define ATA_SR_BSY                 0x80  /* Busy */
#define ATA_SR_DRDY                0x40  /* Drive Ready */
#define ATA_SR_DF                  0x20  /* Drive Fault */
#define ATA_SR_DSC                 0x10  /* Drive Seek Complete */
#define ATA_SR_DRQ                 0x08  /* Data Request Ready */
#define ATA_SR_CORR                0x04  /* Corrected Data */
#define ATA_SR_IDX                 0x02  /* Index */
#define ATA_SR_ERR                 0x01  /* Error */

/* ATA Commands */
#define ATA_CMD_READ_PIO           0x20
#define ATA_CMD_READ_PIO_EXT       0x24
#define ATA_CMD_IDENTIFY           0xEC
#define ATA_CMD_IDENTIFY_PACKET    0xA1

/* ATA Error Codes */
#define ATA_OK                      0
#define ATA_ERR_NO_DEV             -1
#define ATA_ERR_TIMEOUT            -2
#define ATA_ERR_FAULT              -3
#define ATA_ERR_RANGE              -4
#define ATA_ERR_UNSUPPORTED        -5
#define ATA_ERR_ROFS               -6

/* Forward declaration */
struct ata_device;

/* Port I/O Operations Table (enables mocking in host unit tests) */
typedef struct ata_io_ops {
    void (*outb)(uint16_t port, uint8_t val);
    uint8_t (*inb)(uint16_t port);
    void (*outw)(uint16_t port, uint16_t val);
    uint16_t (*inw)(uint16_t port);
    void (*io_wait)(void);
} ata_io_ops_t;

/* ATA Device Structure */
typedef struct ata_device {
    uint8_t  channel;             /* 0 = Primary, 1 = Secondary */
    uint8_t  drive;               /* 0 = Master, 1 = Slave */
    uint16_t io_base;             /* e.g. 0x1F0 or 0x170 */
    uint16_t ctrl_base;           /* e.g. 0x3F6 or 0x376 */
    int      is_present;          /* 1 if valid ATA device identified */
    int      is_atapi;            /* 1 if ATAPI packet device (CD/DVD) */
    int      lba48_supported;     /* 1 if 48-bit LBA supported */
    uint32_t logical_sector_size; /* Typically 512 bytes */
    uint64_t total_sectors;       /* Total user addressable sectors */
    char     model[41];           /* ASCII Model name (null-terminated) */
    char     serial[21];          /* ASCII Serial number (null-terminated) */
    const ata_io_ops_t *ops;      /* Port I/O operations */
} ata_device_t;

/* Public API */
const ata_io_ops_t *ata_get_default_ops(void);
void ata_device_init(ata_device_t *dev, uint8_t channel, uint8_t drive, const ata_io_ops_t *ops);
int ata_identify(ata_device_t *dev);
int ata_read_sectors(ata_device_t *dev, uint64_t lba, uint32_t count, void *buf);
int ata_read_blocks_4k(ata_device_t *dev, uint32_t block_num, uint32_t block_count, void *buf);

#endif /* ATA_PIO_H */
