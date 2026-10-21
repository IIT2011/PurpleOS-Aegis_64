#include "ata_pio.h"
#include "io.h"
#include "string.h"

#define ATA_TIMEOUT_CYCLES 200000

/* Default Port I/O Operations for Kernel */
static void default_outb(uint16_t port, uint8_t val) { outb(port, val); }
static uint8_t default_inb(uint16_t port) { return inb(port); }
static void default_outw(uint16_t port, uint16_t val) { outw(port, val); }
static uint16_t default_inw(uint16_t port) { return inw(port); }
static void default_io_wait(void) { io_wait(); }

static const ata_io_ops_t g_default_ops = {
    .outb = default_outb,
    .inb = default_inb,
    .outw = default_outw,
    .inw = default_inw,
    .io_wait = default_io_wait,
};

const ata_io_ops_t *ata_get_default_ops(void) {
    return &g_default_ops;
}

void ata_device_init(ata_device_t *dev, uint8_t channel, uint8_t drive, const ata_io_ops_t *ops) {
    if (!dev) return;
    memset(dev, 0, sizeof(*dev));
    dev->channel = channel;
    dev->drive = drive;
    dev->io_base = (channel == ATA_CHANNEL_PRIMARY) ? ATA_PRIMARY_IO_BASE : ATA_SECONDARY_IO_BASE;
    dev->ctrl_base = (channel == ATA_CHANNEL_PRIMARY) ? ATA_PRIMARY_CTRL_BASE : ATA_SECONDARY_CTRL_BASE;
    dev->ops = ops ? ops : &g_default_ops;
    dev->logical_sector_size = 512;
}

static int ata_wait_not_busy(const ata_device_t *dev) {
    for (uint32_t i = 0; i < ATA_TIMEOUT_CYCLES; i++) {
        uint8_t status = dev->ops->inb(dev->ctrl_base);
        if (!(status & ATA_SR_BSY)) {
            return ATA_OK;
        }
    }
    return ATA_ERR_TIMEOUT;
}

static int ata_wait_drq(const ata_device_t *dev) {
    for (uint32_t i = 0; i < ATA_TIMEOUT_CYCLES; i++) {
        uint8_t status = dev->ops->inb(dev->ctrl_base);
        if (status & ATA_SR_ERR) {
            return ATA_ERR_FAULT;
        }
        if (status & ATA_SR_DF) {
            return ATA_ERR_FAULT;
        }
        if (!(status & ATA_SR_BSY) && (status & ATA_SR_DRQ)) {
            return ATA_OK;
        }
    }
    return ATA_ERR_TIMEOUT;
}

static void swap_string_words(char *dst, const uint16_t *src, size_t word_count) {
    for (size_t i = 0; i < word_count; i++) {
        dst[i * 2]     = (char)((src[i] >> 8) & 0xFF);
        dst[i * 2 + 1] = (char)(src[i] & 0xFF);
    }
    dst[word_count * 2] = '\0';
    // Trim trailing whitespace
    for (int i = (int)(word_count * 2 - 1); i >= 0 && dst[i] == ' '; i--) {
        dst[i] = '\0';
    }
}

int ata_identify(ata_device_t *dev) {
    if (!dev || !dev->ops) return ATA_ERR_NO_DEV;

    // 1. Select Drive
    dev->ops->outb(dev->io_base + 6, (uint8_t)(0xA0 | (dev->drive << 4)));
    for (int i = 0; i < 4; i++) dev->ops->io_wait();

    // 2. Clear Sector Count and LBA registers
    dev->ops->outb(dev->io_base + 2, 0);
    dev->ops->outb(dev->io_base + 3, 0);
    dev->ops->outb(dev->io_base + 4, 0);
    dev->ops->outb(dev->io_base + 5, 0);

    // 3. Send IDENTIFY Command (0xEC)
    dev->ops->outb(dev->io_base + 7, ATA_CMD_IDENTIFY);
    dev->ops->io_wait();

    // 4. Check initial status
    uint8_t status = dev->ops->inb(dev->io_base + 7);
    if (status == 0x00 || status == 0xFF) {
        // 0x00: Non-existent drive; 0xFF: Floating bus (no hardware)
        return ATA_ERR_NO_DEV;
    }

    // 5. Wait for BSY to clear
    if (ata_wait_not_busy(dev) != ATA_OK) {
        return ATA_ERR_TIMEOUT;
    }

    // 6. Check for ATAPI devices (CD/DVD drives)
    uint8_t cl = dev->ops->inb(dev->io_base + 4);
    uint8_t ch = dev->ops->inb(dev->io_base + 5);
    if ((cl == 0x14 && ch == 0xEB) || (cl == 0x69 && ch == 0x96)) {
        dev->is_atapi = 1;
        dev->is_present = 0;
        return ATA_ERR_UNSUPPORTED; // ATAPI not supported in this stage
    }

    // 7. Wait for DRQ
    if (ata_wait_drq(dev) != ATA_OK) {
        return ATA_ERR_FAULT;
    }

    // 8. Read 256 words (512 bytes) of IDENTIFY data
    uint16_t id_buf[256];
    for (int i = 0; i < 256; i++) {
        id_buf[i] = dev->ops->inw(dev->io_base);
    }

    // 9. Parse IDENTIFY fields
    // Extract Serial Number (Words 10-19)
    swap_string_words(dev->serial, &id_buf[10], 10);

    // Extract Model String (Words 27-46)
    swap_string_words(dev->model, &id_buf[27], 20);

    // Check logical sector size (Word 106)
    if ((id_buf[106] & 0xC000) == 0x4000) {
        if (id_buf[106] & (1 << 12)) {
            // Multiple logical sectors per physical sector
        }
        if (id_buf[106] & (1 << 13)) {
            // Logical sector size is specified in words 117-118
            uint32_t words_per_sec = (uint32_t)id_buf[117] | ((uint32_t)id_buf[118] << 16);
            dev->logical_sector_size = words_per_sec * 2;
        } else {
            dev->logical_sector_size = 512;
        }
    } else {
        dev->logical_sector_size = 512;
    }

    if (dev->logical_sector_size == 0) {
        dev->logical_sector_size = 512;
    }

    // Check LBA48 support (Word 83 bit 10)
    if (id_buf[83] & (1 << 10)) {
        dev->lba48_supported = 1;
        dev->total_sectors = ((uint64_t)id_buf[100]) |
                             (((uint64_t)id_buf[101]) << 16) |
                             (((uint64_t)id_buf[102]) << 32) |
                             (((uint64_t)id_buf[103]) << 48);
    } else {
        dev->lba48_supported = 0;
        // LBA28 total sectors (Words 60-61)
        dev->total_sectors = ((uint32_t)id_buf[60]) | (((uint32_t)id_buf[61]) << 16);
    }

    if (dev->total_sectors == 0) {
        return ATA_ERR_FAULT;
    }

    dev->is_present = 1;
    return ATA_OK;
}

int ata_read_sectors(ata_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    if (!dev || !dev->is_present || !dev->ops) return ATA_ERR_NO_DEV;
    if (count == 0) return ATA_OK;
    if (!buf) return ATA_ERR_RANGE;

    // Check overflow and bounds
    if (lba + count < lba || lba + count > dev->total_sectors) {
        return ATA_ERR_RANGE;
    }

    // In this stage, we support LBA28 reading (up to 2^28 sectors = 128 GiB with 512-byte sectors)
    if (lba + count > 0x0FFFFFFF) {
        return ATA_ERR_RANGE;
    }

    uint8_t *byte_buf = (uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t current_lba = lba;

    while (remaining > 0) {
        uint32_t chunk = (remaining > 256) ? 256 : remaining;

        if (ata_wait_not_busy(dev) != ATA_OK) {
            return ATA_ERR_TIMEOUT;
        }

        // Select drive and bits 24..27 of LBA
        uint8_t drive_head = (uint8_t)(0xE0 | (dev->drive << 4) | ((current_lba >> 24) & 0x0F));
        dev->ops->outb(dev->io_base + 6, drive_head);
        for (int i = 0; i < 4; i++) dev->ops->io_wait();

        // Write sector count (0 means 256 sectors in ATA PIO)
        dev->ops->outb(dev->io_base + 2, (uint8_t)(chunk == 256 ? 0 : chunk));
        dev->ops->outb(dev->io_base + 3, (uint8_t)(current_lba & 0xFF));
        dev->ops->outb(dev->io_base + 4, (uint8_t)((current_lba >> 8) & 0xFF));
        dev->ops->outb(dev->io_base + 5, (uint8_t)((current_lba >> 16) & 0xFF));

        // Issue Read Sectors command
        dev->ops->outb(dev->io_base + 7, ATA_CMD_READ_PIO);

        for (uint32_t s = 0; s < chunk; s++) {
            if (ata_wait_drq(dev) != ATA_OK) {
                return ATA_ERR_FAULT;
            }

            // Transfer 256 words (512 bytes)
            uint16_t *word_ptr = (uint16_t *)byte_buf;
            for (int w = 0; w < 256; w++) {
                word_ptr[w] = dev->ops->inw(dev->io_base);
            }
            byte_buf += 512;
        }

        remaining -= chunk;
        current_lba += chunk;
    }

    return ATA_OK;
}

int ata_read_blocks_4k(ata_device_t *dev, uint32_t block_num, uint32_t block_count, void *buf) {
    if (!dev || !dev->is_present) return ATA_ERR_NO_DEV;
    if (block_count == 0) return ATA_OK;
    if (!buf) return ATA_ERR_RANGE;

    // Check logical sector size
    if (dev->logical_sector_size == 512) {
        uint64_t start_lba = (uint64_t)block_num * 8;
        uint64_t sector_count = (uint64_t)block_count * 8;

        if (start_lba / 8 != block_num || sector_count / 8 != block_count) {
            return ATA_ERR_RANGE; // Arithmetic overflow
        }

        return ata_read_sectors(dev, start_lba, (uint32_t)sector_count, buf);
    } else if (dev->logical_sector_size == 4096) {
        return ata_read_sectors(dev, block_num, block_count, buf);
    } else {
        return ATA_ERR_UNSUPPORTED;
    }
}

int ata_flush_cache(ata_device_t *dev) {
    if (!dev || !dev->is_present || !dev->ops) return ATA_ERR_NO_DEV;
    if (ata_wait_not_busy(dev) != ATA_OK) return ATA_ERR_TIMEOUT;

    uint8_t drive_head = (uint8_t)(0xE0 | (dev->drive << 4));
    dev->ops->outb(dev->io_base + 6, drive_head);
    for (int i = 0; i < 4; i++) dev->ops->io_wait();

    dev->ops->outb(dev->io_base + 7, dev->lba48_supported ? ATA_CMD_CACHE_FLUSH_EXT : ATA_CMD_CACHE_FLUSH);

    if (ata_wait_not_busy(dev) != ATA_OK) {
        return ATA_ERR_TIMEOUT;
    }
    return ATA_OK;
}

int ata_write_sectors(ata_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    if (!dev || !dev->is_present || !dev->ops) return ATA_ERR_NO_DEV;
    if (count == 0) return ATA_OK;
    if (!buf) return ATA_ERR_RANGE;

    // Check overflow and bounds
    if (lba + count < lba || lba + count > dev->total_sectors) {
        return ATA_ERR_RANGE;
    }

    if (lba + count > 0x0FFFFFFF) {
        return ATA_ERR_RANGE;
    }

    const uint8_t *byte_buf = (const uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t current_lba = lba;

    while (remaining > 0) {
        uint32_t chunk = (remaining > 256) ? 256 : remaining;

        if (ata_wait_not_busy(dev) != ATA_OK) {
            return ATA_ERR_TIMEOUT;
        }

        // Select drive and bits 24..27 of LBA
        uint8_t drive_head = (uint8_t)(0xE0 | (dev->drive << 4) | ((current_lba >> 24) & 0x0F));
        dev->ops->outb(dev->io_base + 6, drive_head);
        for (int i = 0; i < 4; i++) dev->ops->io_wait();

        // Write sector count (0 means 256 sectors in ATA PIO)
        dev->ops->outb(dev->io_base + 2, (uint8_t)(chunk == 256 ? 0 : chunk));
        dev->ops->outb(dev->io_base + 3, (uint8_t)(current_lba & 0xFF));
        dev->ops->outb(dev->io_base + 4, (uint8_t)((current_lba >> 8) & 0xFF));
        dev->ops->outb(dev->io_base + 5, (uint8_t)((current_lba >> 16) & 0xFF));

        // Issue Write Sectors command
        dev->ops->outb(dev->io_base + 7, ATA_CMD_WRITE_PIO);

        for (uint32_t s = 0; s < chunk; s++) {
            if (ata_wait_drq(dev) != ATA_OK) {
                return ATA_ERR_FAULT;
            }

            // Transfer 256 words (512 bytes)
            const uint16_t *word_ptr = (const uint16_t *)byte_buf;
            for (int w = 0; w < 256; w++) {
                dev->ops->outw(dev->io_base, word_ptr[w]);
            }
            byte_buf += 512;
        }

        if (ata_wait_not_busy(dev) != ATA_OK) {
            return ATA_ERR_TIMEOUT;
        }

        remaining -= chunk;
        current_lba += chunk;
    }

    return ata_flush_cache(dev);
}

int ata_write_blocks_4k(ata_device_t *dev, uint32_t block_num, uint32_t block_count, const void *buf) {
    if (!dev || !dev->is_present) return ATA_ERR_NO_DEV;
    if (block_count == 0) return ATA_OK;
    if (!buf) return ATA_ERR_RANGE;

    if (dev->logical_sector_size == 512) {
        uint64_t start_lba = (uint64_t)block_num * 8;
        uint64_t sector_count = (uint64_t)block_count * 8;

        if (start_lba / 8 != block_num || sector_count / 8 != block_count) {
            return ATA_ERR_RANGE; // Arithmetic overflow
        }

        return ata_write_sectors(dev, start_lba, (uint32_t)sector_count, buf);
    } else if (dev->logical_sector_size == 4096) {
        return ata_write_sectors(dev, block_num, block_count, buf);
    } else {
        return ATA_ERR_UNSUPPORTED;
    }
}
