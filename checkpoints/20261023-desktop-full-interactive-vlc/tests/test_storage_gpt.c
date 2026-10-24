#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <assert.h>

#include "ata_pio.h"
#include "gpt.h"
#include "partition_dev.h"
#include "purplefs_v2.h"
#include "host_purplefs_v2.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %2d] Running %-45s ... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        tests_passed++; \
        printf("PASSED\n"); \
    } else { \
        printf("FAILED\n"); \
    } \
} while(0)

/* Mock ATA Controller State */
#define MOCK_DISK_SECTORS 65536 // 32 MiB disk image
static uint8_t g_mock_disk[MOCK_DISK_SECTORS][512];

typedef struct {
    uint8_t  reg_drive_head;
    uint8_t  reg_sec_count;
    uint8_t  reg_lba_lo;
    uint8_t  reg_lba_mid;
    uint8_t  reg_lba_hi;
    uint8_t  reg_cmd;
    uint8_t  reg_status;
    uint8_t  reg_error;
    uint16_t identify_words[256];
    uint32_t current_lba;
    uint32_t sectors_remaining;
    uint32_t word_offset_in_sector;
    int      in_identify;
    int      mock_always_busy;
    int      mock_error_on_read;
    int      mock_floating_bus;
    int      mock_non_existent;
    int      mock_atapi;
} mock_ata_state_t;

static mock_ata_state_t g_mock;

static void mock_reset(void) {
    memset(&g_mock, 0, sizeof(g_mock));
    memset(g_mock_disk, 0, sizeof(g_mock_disk));
    g_mock.reg_status = ATA_SR_DRDY;

    // Fill standard synthetic IDENTIFY data
    g_mock.identify_words[0] = 0x0040; // ATA device
    // Serial: 20 characters (10 words)
    const char *serial = "MOCK-SERIAL-001     ";
    for (int i = 0; i < 10; i++) {
        g_mock.identify_words[10 + i] = ((uint16_t)(uint8_t)serial[i * 2] << 8) | (uint8_t)serial[i * 2 + 1];
    }
    // Model: 40 characters (20 words)
    const char *model = "Aegis64 Mock Disk QEMU-Virtual          ";
    for (int i = 0; i < 20; i++) {
        g_mock.identify_words[27 + i] = ((uint16_t)(uint8_t)model[i * 2] << 8) | (uint8_t)model[i * 2 + 1];
    }
    // Capabilities
    g_mock.identify_words[49] = (1 << 9); // LBA supported
    // Total sectors LBA28
    g_mock.identify_words[60] = (uint16_t)(MOCK_DISK_SECTORS & 0xFFFF);
    g_mock.identify_words[61] = (uint16_t)((MOCK_DISK_SECTORS >> 16) & 0xFFFF);
    // Command sets (LBA48 support in word 83)
    g_mock.identify_words[83] = (1 << 10);
    // 48-bit total sectors
    g_mock.identify_words[100] = (uint16_t)(MOCK_DISK_SECTORS & 0xFFFF);
    g_mock.identify_words[101] = (uint16_t)((MOCK_DISK_SECTORS >> 16) & 0xFFFF);
    // Sector size: 512 bytes (word 106 bit 14=1, bit 15=0, bit 13=0)
    g_mock.identify_words[106] = 0x4000;
}

static void mock_outb(uint16_t port, uint8_t val) {
    if (port == ATA_PRIMARY_IO_BASE + 2) {
        g_mock.reg_sec_count = val;
    } else if (port == ATA_PRIMARY_IO_BASE + 3) {
        g_mock.reg_lba_lo = val;
    } else if (port == ATA_PRIMARY_IO_BASE + 4) {
        g_mock.reg_lba_mid = val;
    } else if (port == ATA_PRIMARY_IO_BASE + 5) {
        g_mock.reg_lba_hi = val;
    } else if (port == ATA_PRIMARY_IO_BASE + 6) {
        g_mock.reg_drive_head = val;
    } else if (port == ATA_PRIMARY_IO_BASE + 7) {
        g_mock.reg_cmd = val;
        if (val == ATA_CMD_IDENTIFY) {
            g_mock.in_identify = 1;
            g_mock.word_offset_in_sector = 0;
            g_mock.reg_status = ATA_SR_DRDY | ATA_SR_DRQ;
        } else if (val == ATA_CMD_READ_PIO) {
            g_mock.in_identify = 0;
            uint32_t count = (g_mock.reg_sec_count == 0) ? 256 : g_mock.reg_sec_count;
            uint32_t lba = (uint32_t)g_mock.reg_lba_lo |
                           ((uint32_t)g_mock.reg_lba_mid << 8) |
                           ((uint32_t)g_mock.reg_lba_hi << 16) |
                           (((uint32_t)g_mock.reg_drive_head & 0x0F) << 24);
            g_mock.current_lba = lba;
            g_mock.sectors_remaining = count;
            g_mock.word_offset_in_sector = 0;

            if (g_mock.mock_error_on_read) {
                g_mock.reg_status = ATA_SR_ERR;
                g_mock.reg_error = 0x04; // Aborted command
            } else {
                g_mock.reg_status = ATA_SR_DRDY | ATA_SR_DRQ;
            }
        } else if (val == ATA_CMD_WRITE_PIO) {
            g_mock.in_identify = 0;
            uint32_t count = (g_mock.reg_sec_count == 0) ? 256 : g_mock.reg_sec_count;
            uint32_t lba = (uint32_t)g_mock.reg_lba_lo |
                           ((uint32_t)g_mock.reg_lba_mid << 8) |
                           ((uint32_t)g_mock.reg_lba_hi << 16) |
                           (((uint32_t)g_mock.reg_drive_head & 0x0F) << 24);
            g_mock.current_lba = lba;
            g_mock.sectors_remaining = count;
            g_mock.word_offset_in_sector = 0;
            g_mock.reg_status = ATA_SR_DRDY | ATA_SR_DRQ;
        } else if (val == ATA_CMD_CACHE_FLUSH || val == ATA_CMD_CACHE_FLUSH_EXT) {
            g_mock.reg_status = ATA_SR_DRDY;
        }
    }
}

static uint8_t mock_inb(uint16_t port) {
    if (g_mock.mock_floating_bus) return 0xFF;
    if (g_mock.mock_non_existent) return 0x00;

    if (port == ATA_PRIMARY_IO_BASE + 4) {
        if (g_mock.mock_atapi) return 0x14; // ATAPI cylinder low
        return g_mock.reg_lba_mid;
    }
    if (port == ATA_PRIMARY_IO_BASE + 5) {
        if (g_mock.mock_atapi) return 0xEB; // ATAPI cylinder high
        return g_mock.reg_lba_hi;
    }
    if (port == ATA_PRIMARY_IO_BASE + 7 || port == ATA_PRIMARY_CTRL_BASE) {
        if (g_mock.mock_always_busy) return ATA_SR_BSY;
        return g_mock.reg_status;
    }
    return 0;
}

static void mock_outw(uint16_t port, uint16_t val) {
    if (port != ATA_PRIMARY_IO_BASE) return;
    if (g_mock.sectors_remaining > 0 && g_mock.current_lba < MOCK_DISK_SECTORS) {
        uint8_t *sec = g_mock_disk[g_mock.current_lba];
        sec[g_mock.word_offset_in_sector * 2]     = (uint8_t)(val & 0xFF);
        sec[g_mock.word_offset_in_sector * 2 + 1] = (uint8_t)((val >> 8) & 0xFF);
        g_mock.word_offset_in_sector++;
        if (g_mock.word_offset_in_sector >= 256) {
            g_mock.word_offset_in_sector = 0;
            g_mock.current_lba++;
            g_mock.sectors_remaining--;
            if (g_mock.sectors_remaining == 0) {
                g_mock.reg_status = ATA_SR_DRDY;
            }
        }
    }
}

static uint16_t mock_inw(uint16_t port) {
    if (port != ATA_PRIMARY_IO_BASE) return 0;

    if (g_mock.in_identify) {
        uint16_t w = g_mock.identify_words[g_mock.word_offset_in_sector++];
        if (g_mock.word_offset_in_sector >= 256) {
            g_mock.reg_status = ATA_SR_DRDY; // DRQ cleared
            g_mock.in_identify = 0;
        }
        return w;
    }

    if (g_mock.sectors_remaining > 0 && g_mock.current_lba < MOCK_DISK_SECTORS) {
        uint8_t *sec = g_mock_disk[g_mock.current_lba];
        uint16_t w = (uint16_t)sec[g_mock.word_offset_in_sector * 2] |
                     ((uint16_t)sec[g_mock.word_offset_in_sector * 2 + 1] << 8);
        g_mock.word_offset_in_sector++;
        if (g_mock.word_offset_in_sector >= 256) {
            g_mock.word_offset_in_sector = 0;
            g_mock.current_lba++;
            g_mock.sectors_remaining--;
            if (g_mock.sectors_remaining == 0) {
                g_mock.reg_status = ATA_SR_DRDY;
            }
        }
        return w;
    }

    return 0;
}

static void mock_io_wait(void) {}

static const ata_io_ops_t g_mock_ops = {
    .outb = mock_outb,
    .inb = mock_inb,
    .outw = mock_outw,
    .inw = mock_inw,
    .io_wait = mock_io_wait,
};

/* Helper: Build a minimal valid GPT image in g_mock_disk */
static void setup_valid_gpt_disk(uint64_t part_start_lba, uint64_t part_end_lba, const gpt_guid_t *type_guid) {
    // 1. Protective MBR at LBA 0
    mbr_sector_t *mbr = (mbr_sector_t *)g_mock_disk[0];
    mbr->signature = 0xAA55;
    mbr->partitions[0].os_type = 0xEE;
    mbr->partitions[0].starting_lba = 1;
    mbr->partitions[0].size_in_lba = MOCK_DISK_SECTORS - 1;

    // 2. Primary GPT Header at LBA 1
    gpt_header_t *hdr = (gpt_header_t *)g_mock_disk[1];
    hdr->signature = GPT_SIGNATURE;
    hdr->revision = GPT_REVISION_1_0;
    hdr->header_size = 92;
    hdr->my_lba = 1;
    hdr->alternate_lba = MOCK_DISK_SECTORS - 1;
    hdr->first_usable_lba = 34;
    hdr->last_usable_lba = MOCK_DISK_SECTORS - 34;
    hdr->partition_entry_lba = 2;
    hdr->num_partition_entries = 128;
    hdr->size_of_partition_entry = 128;

    // 3. Partition entries starting at LBA 2
    // Entry 0
    gpt_entry_t *entry = (gpt_entry_t *)g_mock_disk[2];
    memcpy(&entry->partition_type_guid, type_guid, sizeof(gpt_guid_t));
    entry->unique_partition_guid.data1 = 0x11112222;
    entry->starting_lba = part_start_lba;
    entry->ending_lba = part_end_lba;
    const char *name = "PurpleData";
    for (size_t i = 0; i < strlen(name); i++) {
        entry->partition_name[i] = (uint16_t)name[i];
    }

    // 4. Calculate partition entry array CRC32 (128 entries * 128 bytes = 16384 bytes = 32 sectors)
    uint32_t array_crc = pfs2_crc32(g_mock_disk[2], 128 * 128);
    hdr->partition_entry_array_crc32 = array_crc;

    // 5. Calculate header CRC32 with header_crc32 field zeroed
    hdr->header_crc32 = 0;
    hdr->header_crc32 = pfs2_crc32(hdr, hdr->header_size);
}

/* ========================================================================= */
/* Test Cases                                                                */
/* ========================================================================= */

static int test_ata_identify_synthetic(void) {
    mock_reset();
    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);

    if (ata_identify(&dev) != ATA_OK) return 0;
    if (!dev.is_present) return 0;
    if (dev.logical_sector_size != 512) return 0;
    if (dev.total_sectors != MOCK_DISK_SECTORS) return 0;
    if (strncmp(dev.model, "Aegis64 Mock Disk QEMU-Virtual", 30) != 0) return 0;
    if (strncmp(dev.serial, "MOCK-SERIAL-001", 15) != 0) return 0;

    return 1;
}

static int test_ata_absent_and_atapi(void) {
    // 1. Floating bus (0xFF)
    mock_reset();
    g_mock.mock_floating_bus = 1;
    ata_device_t dev1;
    ata_device_init(&dev1, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev1) != ATA_ERR_NO_DEV) return 0;
    if (dev1.is_present) return 0;

    // 2. Non-existent drive (0x00)
    mock_reset();
    g_mock.mock_non_existent = 1;
    ata_device_t dev2;
    ata_device_init(&dev2, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev2) != ATA_ERR_NO_DEV) return 0;

    // 3. ATAPI Packet Device
    mock_reset();
    g_mock.mock_atapi = 1;
    ata_device_t dev3;
    ata_device_init(&dev3, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    int rc = ata_identify(&dev3);
    if (rc != ATA_ERR_UNSUPPORTED || !dev3.is_atapi) return 0;

    return 1;
}

static int test_ata_timeout_and_error(void) {
    // 1. BSY timeout
    mock_reset();
    g_mock.mock_always_busy = 1;
    ata_device_t dev1;
    ata_device_init(&dev1, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev1) != ATA_ERR_TIMEOUT) return 0;

    // 2. Read error
    mock_reset();
    ata_device_t dev2;
    ata_device_init(&dev2, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev2) != ATA_OK) return 0;

    g_mock.mock_error_on_read = 1;
    uint8_t buf[512];
    if (ata_read_sectors(&dev2, 10, 1, buf) != ATA_ERR_FAULT) return 0;

    return 1;
}

static int test_ata_sector_reads_and_bounds(void) {
    mock_reset();
    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev) != ATA_OK) return 0;

    // Write known pattern to sector 100
    for (int i = 0; i < 512; i++) {
        g_mock_disk[100][i] = (uint8_t)(i ^ 0x5A);
    }

    uint8_t read_buf[512];
    if (ata_read_sectors(&dev, 100, 1, read_buf) != ATA_OK) return 0;
    if (memcmp(read_buf, g_mock_disk[100], 512) != 0) return 0;

    // Test out of bounds
    if (ata_read_sectors(&dev, MOCK_DISK_SECTORS - 1, 2, read_buf) != ATA_ERR_RANGE) return 0;
    // 64-bit overflow attempt
    if (ata_read_sectors(&dev, 0xFFFFFFFFFFFFFFFFULL, 1, read_buf) != ATA_ERR_RANGE) return 0;

    return 1;
}

static int test_gpt_parsing_valid(void) {
    mock_reset();
    setup_valid_gpt_disk(2048, 40000, &GPT_GUID_PURPLEFS_V2);

    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev) != ATA_OK) return 0;

    gpt_info_t gpt;
    if (gpt_parse(&dev, &gpt) != GPT_OK) return 0;

    if (!gpt.valid_protective_mbr || !gpt.valid_header || !gpt.valid_entries) return 0;
    if (gpt.partition_count != 1) return 0;
    if (gpt.purplefs_part_index != 0) return 0;

    const gpt_partition_t *part = &gpt.partitions[0];
    if (!part->is_purplefs) return 0;
    if (part->start_lba != 2048 || part->end_lba != 40000) return 0;
    if (part->sector_count != (40000 - 2048 + 1)) return 0;
    if (strcmp(part->name, "PurpleData") != 0) return 0;

    return 1;
}

static int test_gpt_invalid_signature_and_crc(void) {
    // 1. Bad MBR signature
    mock_reset();
    setup_valid_gpt_disk(2048, 10000, &GPT_GUID_PURPLEFS_V2);
    g_mock_disk[0][510] = 0x00; // corrupt MBR signature

    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    ata_identify(&dev);

    gpt_info_t gpt;
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_MBR) return 0;

    // 2. Bad GPT signature
    mock_reset();
    setup_valid_gpt_disk(2048, 10000, &GPT_GUID_PURPLEFS_V2);
    g_mock_disk[1][0] = 'B'; // "BFI PART"
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_SIGNATURE) return 0;

    // 3. Corrupt GPT Header CRC
    mock_reset();
    setup_valid_gpt_disk(2048, 10000, &GPT_GUID_PURPLEFS_V2);
    gpt_header_t *hdr = (gpt_header_t *)g_mock_disk[1];
    hdr->header_crc32 ^= 0xFFFFFFFF; // Invert CRC
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_CRC) return 0;

    // 4. Corrupt Partition Entry Array CRC
    mock_reset();
    setup_valid_gpt_disk(2048, 10000, &GPT_GUID_PURPLEFS_V2);
    hdr = (gpt_header_t *)g_mock_disk[1];
    hdr->partition_entry_array_crc32 ^= 0x12345678;
    // Recompute header CRC since we altered partition_entry_array_crc32 inside header
    hdr->header_crc32 = 0;
    hdr->header_crc32 = pfs2_crc32(hdr, hdr->header_size);
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_CRC) return 0;

    return 1;
}

static int test_gpt_invalid_bounds_and_overlap(void) {
    // 1. Inverted bounds: start > end
    mock_reset();
    setup_valid_gpt_disk(10000, 2048, &GPT_GUID_PURPLEFS_V2);
    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    ata_identify(&dev);

    gpt_info_t gpt;
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_BOUNDS) return 0;

    // 2. Beyond disk capacity
    mock_reset();
    setup_valid_gpt_disk(2048, MOCK_DISK_SECTORS + 100, &GPT_GUID_PURPLEFS_V2);
    if (gpt_parse(&dev, &gpt) != GPT_ERR_BAD_BOUNDS) return 0;

    return 1;
}

static int test_partition_dev_boundary_and_overflow(void) {
    mock_reset();
    setup_valid_gpt_disk(2048, 2048 + 800 - 1, &GPT_GUID_PURPLEFS_V2); // 800 sectors = 100 blocks of 4096B

    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    ata_identify(&dev);

    gpt_info_t gpt;
    gpt_parse(&dev, &gpt);

    partition_dev_t pdev;
    if (partition_dev_init(&pdev, &dev, &gpt.partitions[0]) != PART_DEV_OK) return 0;
    if (pdev.base.block_count != 100) return 0;

    // Test first block (block 0 -> LBA 2048..2055)
    uint8_t block_buf[4096];
    memset(g_mock_disk[2048], 0x77, 512);
    if (pdev.base.read_block(&pdev.base, 0, block_buf) != PART_DEV_OK) return 0;
    if (block_buf[0] != 0x77) return 0;

    // Test last valid block (block 99 -> LBA 2048 + 99*8 = 2840..2847)
    memset(g_mock_disk[2840], 0x88, 512);
    if (pdev.base.read_block(&pdev.base, 99, block_buf) != PART_DEV_OK) return 0;
    if (block_buf[0] != 0x88) return 0;

    // Test out of bounds (block 100)
    if (pdev.base.read_block(&pdev.base, 100, block_buf) != PART_DEV_ERR_BOUNDS) return 0;

    // Test 32-bit overflow attempt (block 0xFFFFFFFF)
    if (pdev.base.read_block(&pdev.base, 0xFFFFFFFF, block_buf) != PART_DEV_ERR_BOUNDS) return 0;

    // Test write block rejection (read-only enforcement)
    if (pdev.base.write_block(&pdev.base, 0, block_buf) != PART_DEV_ERR_ROFS) return 0;

    return 1;
}

static int test_purplefs_mount_through_partition_adapter(void) {
    mock_reset();
    uint64_t part_start = 2048;
    uint64_t part_blocks = 2048; // 2048 blocks * 8 = 16384 sectors
    uint64_t part_end = part_start + (part_blocks * 8) - 1;

    setup_valid_gpt_disk(part_start, part_end, &GPT_GUID_PURPLEFS_V2);

    // Format a PurpleFS v2 image using host-side memory device
    pfs2_block_dev_t *mem_dev = pfs2_mem_dev_create((uint32_t)part_blocks);
    if (!mem_dev) return 0;
    if (pfs2_format(mem_dev, (uint32_t)part_blocks, "PurpleRoot") != PFS2_OK) {
        pfs2_mem_dev_destroy(mem_dev);
        return 0;
    }

    // Populate a test file into the formatted image
    pfs2_fs_t mem_fs;
    if (pfs2_mount(&mem_fs, mem_dev) != PFS2_OK) {
        pfs2_mem_dev_destroy(mem_dev);
        return 0;
    }
    uint32_t ino = 0;
    pfs2_create_file(&mem_fs, "readme.txt", 0644, &ino);
    const char *msg = "PurpleFS v2 mounted from GPT partition!";
    size_t written = 0;
    pfs2_write_file(&mem_fs, ino, msg, strlen(msg), 0, &written);
    pfs2_unmount(&mem_fs);

    // Copy formatted blocks into mock disk partition sectors
    for (uint32_t b = 0; b < part_blocks; b++) {
        uint8_t block_buf[PFS2_BLOCK_SIZE];
        mem_dev->read_block(mem_dev, b, block_buf);
        for (int s = 0; s < 8; s++) {
            memcpy(g_mock_disk[part_start + b * 8 + s], block_buf + s * 512, 512);
        }
    }
    pfs2_mem_dev_destroy(mem_dev);

    // Now test discovery and mounting using read-only ATA + GPT + partition_dev stack
    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev) != ATA_OK) return 0;

    gpt_info_t gpt;
    if (gpt_parse(&dev, &gpt) != GPT_OK) return 0;
    if (gpt.purplefs_part_index < 0) return 0;

    partition_dev_t pdev;
    if (partition_dev_init(&pdev, &dev, &gpt.partitions[gpt.purplefs_part_index]) != PART_DEV_OK) return 0;

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, &pdev.base) != PFS2_OK) return 0;

    // Verify superblock properties
    if (fs.sb.block_size != 4096) return 0;
    if (fs.sb.total_blocks != part_blocks) return 0;
    if (strcmp(fs.sb.volume_label, "PurpleRoot") != 0) return 0;

    // Lookup and read the file
    uint32_t found_ino = 0;
    if (pfs2_lookup(&fs, "readme.txt", &found_ino) != PFS2_OK || found_ino != ino) {
        pfs2_unmount(&fs);
        return 0;
    }

    char read_buf[128];
    size_t bytes_read = 0;
    if (pfs2_read_file(&fs, found_ino, read_buf, sizeof(read_buf) - 1, 0, &bytes_read) != PFS2_OK) {
        pfs2_unmount(&fs);
        return 0;
    }
    read_buf[bytes_read] = '\0';
    if (strcmp(read_buf, msg) != 0) {
        pfs2_unmount(&fs);
        return 0;
    }

    // Clean unmount
    if (pfs2_unmount(&fs) != PFS2_OK) return 0;

    return 1;
}

static int test_rejection_of_invalid_partition_no_format(void) {
    mock_reset();
    // Valid GPT, but partition is filled with unformatted raw garbage
    setup_valid_gpt_disk(2048, 2048 + 1024 * 8 - 1, &GPT_GUID_PURPLEFS_V2);

    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    ata_identify(&dev);

    gpt_info_t gpt;
    gpt_parse(&dev, &gpt);

    partition_dev_t pdev;
    partition_dev_init(&pdev, &dev, &gpt.partitions[0]);

    // Save snapshot of sector 2048 (Block 0 of partition)
    uint8_t snap[512];
    memcpy(snap, g_mock_disk[2048], 512);

    // Attempt to mount: must return PFS2_ERR_BAD_MAGIC
    pfs2_fs_t fs;
    int rc = pfs2_mount(&fs, &pdev.base);
    if (rc != PFS2_ERR_BAD_MAGIC) return 0;

    // Verify disk was NOT formatted or modified
    if (memcmp(snap, g_mock_disk[2048], 512) != 0) return 0;

    return 1;
}

static int test_ata_sector_writes_and_flush(void) {
    mock_reset();
    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    if (ata_identify(&dev) != ATA_OK) return 0;

    // Test writing 2 sectors to LBA 100
    uint8_t write_buf[1024];
    for (int i = 0; i < 1024; i++) write_buf[i] = (uint8_t)(i ^ 0x5A);

    int rc = ata_write_sectors(&dev, 100, 2, write_buf);
    if (rc != ATA_OK) return 0;

    // Check mock disk directly
    if (memcmp(g_mock_disk[100], write_buf, 512) != 0) return 0;
    if (memcmp(g_mock_disk[101], write_buf + 512, 512) != 0) return 0;

    // Read back via ata_read_sectors
    uint8_t read_back[1024];
    memset(read_back, 0, sizeof(read_back));
    if (ata_read_sectors(&dev, 100, 2, read_back) != ATA_OK) return 0;
    if (memcmp(read_back, write_buf, 1024) != 0) return 0;

    // Test 4K block write to block 15 (LBA 120..127)
    uint8_t block4k[4096];
    for (int i = 0; i < 4096; i++) block4k[i] = (uint8_t)((i * 7) & 0xFF);
    if (ata_write_blocks_4k(&dev, 15, 1, block4k) != ATA_OK) return 0;

    uint8_t block4k_read[4096];
    if (ata_read_blocks_4k(&dev, 15, 1, block4k_read) != ATA_OK) return 0;
    if (memcmp(block4k_read, block4k, 4096) != 0) return 0;

    // Out of bounds write rejection
    if (ata_write_sectors(&dev, MOCK_DISK_SECTORS, 1, write_buf) != ATA_ERR_RANGE) return 0;
    if (ata_write_sectors(&dev, MOCK_DISK_SECTORS - 1, 2, write_buf) != ATA_ERR_RANGE) return 0;

    // Cache flush check
    if (ata_flush_cache(&dev) != ATA_OK) return 0;

    return 1;
}

static int test_partition_dev_writable_operations(void) {
    mock_reset();
    setup_valid_gpt_disk(2048, 2048 + 1024 * 8 - 1, &GPT_GUID_PURPLEFS_V2);

    ata_device_t dev;
    ata_device_init(&dev, ATA_CHANNEL_PRIMARY, ATA_DRIVE_MASTER, &g_mock_ops);
    ata_identify(&dev);

    gpt_info_t gpt;
    gpt_parse(&dev, &gpt);

    partition_dev_t pdev;
    partition_dev_init(&pdev, &dev, &gpt.partitions[0]);

    // Initially read-only
    if (partition_dev_is_writable(&pdev) != 0) return 0;

    uint8_t blk[4096];
    memset(blk, 0xAA, sizeof(blk));
    if (pdev.base.write_block(&pdev.base, 0, blk) != PART_DEV_ERR_ROFS) return 0;

    // Set writable
    partition_dev_set_writable(&pdev, 1);
    if (partition_dev_is_writable(&pdev) != 1) return 0;

    // Now write block 5
    memset(blk, 0x77, sizeof(blk));
    if (pdev.base.write_block(&pdev.base, 5, blk) != PART_DEV_OK) return 0;

    // Read back block 5
    uint8_t read_blk[4096];
    if (pdev.base.read_block(&pdev.base, 5, read_blk) != PART_DEV_OK) return 0;
    if (memcmp(read_blk, blk, 4096) != 0) return 0;

    // Out of bounds block write rejection
    if (pdev.base.write_block(&pdev.base, pdev.base.block_count, blk) != PART_DEV_ERR_BOUNDS) return 0;

    // Flush
    if (pdev.base.flush(&pdev.base) != PART_DEV_OK) return 0;

    // Disable writable again
    partition_dev_set_writable(&pdev, 0);
    if (pdev.base.write_block(&pdev.base, 5, blk) != PART_DEV_ERR_ROFS) return 0;

    return 1;
}

static int mock_bdev_read(block_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    (void)dev;
    if (lba + count > MOCK_DISK_SECTORS) return BLOCK_DEV_ERR_BOUNDS;
    for (uint32_t i = 0; i < count; i++) {
        memcpy((uint8_t *)buf + i * 512, g_mock_disk[lba + i], 512);
    }
    return BLOCK_DEV_OK;
}

static int mock_bdev_write(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    (void)dev;
    if (lba + count > MOCK_DISK_SECTORS) return BLOCK_DEV_ERR_BOUNDS;
    for (uint32_t i = 0; i < count; i++) {
        memcpy(g_mock_disk[lba + i], (const uint8_t *)buf + i * 512, 512);
    }
    return BLOCK_DEV_OK;
}

static int mock_bdev_flush(block_device_t *dev) {
    (void)dev;
    return BLOCK_DEV_OK;
}

static const block_device_ops_t s_mock_bdev_ops = {
    .read_blocks = mock_bdev_read,
    .write_blocks = mock_bdev_write,
    .flush = mock_bdev_flush,
};

static int test_generic_block_dev_gpt_and_partition(void) {
    mock_reset();
    uint64_t part_start = 2048;
    uint64_t part_blocks = 512;
    uint64_t part_end = part_start + (part_blocks * 8) - 1;

    setup_valid_gpt_disk(part_start, part_end, &GPT_GUID_PURPLEFS_V2);

    // Format PurpleFS v2 into mock disk partition
    pfs2_block_dev_t *mem_dev = pfs2_mem_dev_create((uint32_t)part_blocks);
    if (!mem_dev) return 0;
    if (pfs2_format(mem_dev, (uint32_t)part_blocks, "PurpleGeneric") != PFS2_OK) {
        pfs2_mem_dev_destroy(mem_dev);
        return 0;
    }
    for (uint32_t b = 0; b < part_blocks; b++) {
        uint8_t block_buf[PFS2_BLOCK_SIZE];
        mem_dev->read_block(mem_dev, b, block_buf);
        for (int s = 0; s < 8; s++) {
            memcpy(g_mock_disk[part_start + b * 8 + s], block_buf + s * 512, 512);
        }
    }
    pfs2_mem_dev_destroy(mem_dev);

    // Create a mock generic block_device_t (simulating SATA or NVMe)
    block_device_t mock_bdev;
    memset(&mock_bdev, 0, sizeof(mock_bdev));
    strcpy(mock_bdev.name, "sata0");
    mock_bdev.type = BLOCK_DEV_TYPE_AHCI;
    mock_bdev.block_size = 512;
    mock_bdev.total_blocks = MOCK_DISK_SECTORS;
    mock_bdev.ops = &s_mock_bdev_ops;

    // 1. Parse GPT directly via block_device_t
    gpt_info_t gpt;
    if (gpt_parse_block_dev(&mock_bdev, &gpt) != GPT_OK) return 0;
    if (!gpt.valid_protective_mbr || !gpt.valid_header || !gpt.valid_entries) return 0;
    if (gpt.partition_count != 1 || gpt.purplefs_part_index != 0) return 0;

    // 2. Initialize partition device directly via block_device_t
    partition_dev_t pdev;
    if (partition_dev_init_bdev(&pdev, &mock_bdev, &gpt.partitions[0]) != PART_DEV_OK) return 0;
    if (pdev.base.block_count != (uint32_t)part_blocks) return 0;

    // 3. Mount PurpleFS v2 over partition_dev
    pfs2_fs_t fs;
    if (pfs2_mount(&fs, &pdev.base) != PFS2_OK) return 0;
    if (strcmp(fs.sb.volume_label, "PurpleGeneric") != 0) return 0;
    if (fs.read_only != 1) return 0;

    // 4. Test writable partition operations
    partition_dev_set_writable(&pdev, 1);
    fs.read_only = 0;
    uint32_t new_ino = 0;
    if (pfs2_create_file(&fs, "generic_test.txt", 0644, &new_ino) != PFS2_OK) return 0;
    const char *test_data = "Hello Generic Block Device!";
    size_t written = 0;
    if (pfs2_write_file(&fs, new_ino, test_data, strlen(test_data), 0, &written) != PFS2_OK) return 0;
    if (written != strlen(test_data)) return 0;

    pfs2_unmount(&fs);
    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Storage, ATA & GPT Unit Test Suite          \n");
    printf("=====================================================\n");

    RUN_TEST(test_ata_identify_synthetic);
    RUN_TEST(test_ata_absent_and_atapi);
    RUN_TEST(test_ata_timeout_and_error);
    RUN_TEST(test_ata_sector_reads_and_bounds);
    RUN_TEST(test_ata_sector_writes_and_flush);
    RUN_TEST(test_gpt_parsing_valid);
    RUN_TEST(test_gpt_invalid_signature_and_crc);
    RUN_TEST(test_gpt_invalid_bounds_and_overlap);
    RUN_TEST(test_partition_dev_boundary_and_overflow);
    RUN_TEST(test_partition_dev_writable_operations);
    RUN_TEST(test_purplefs_mount_through_partition_adapter);
    RUN_TEST(test_rejection_of_invalid_partition_no_format);
    RUN_TEST(test_generic_block_dev_gpt_and_partition);

    printf("=====================================================\n");
    printf(" Storage & GPT Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
