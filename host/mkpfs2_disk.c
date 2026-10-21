#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>

#include "purplefs_v2.h"
#include "host_purplefs_v2.h"
#include "gpt.h"

#define TOTAL_SECTORS 65536 // 32 MiB disk
#define SECTOR_SIZE 512
#define PART_START_LBA 2048
#define PART_BLOCKS 4096    // 4096 blocks * 8 sectors = 32768 sectors = 16 MiB
#define PART_END_LBA (PART_START_LBA + (PART_BLOCKS * 8) - 1)

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <output_disk_image>\n", argv[0]);
        return 1;
    }

    const char *img_path = argv[1];
    int fd = open(img_path, O_RDWR | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        perror("open disk image");
        return 1;
    }

    off_t total_bytes = (off_t)TOTAL_SECTORS * SECTOR_SIZE;
    if (ftruncate(fd, total_bytes) != 0) {
        perror("ftruncate disk image");
        close(fd);
        return 1;
    }

    // 1. Write Protective MBR (LBA 0)
    mbr_sector_t mbr;
    memset(&mbr, 0, sizeof(mbr));
    mbr.signature = 0xAA55;
    mbr.partitions[0].os_type = 0xEE;
    mbr.partitions[0].starting_lba = 1;
    mbr.partitions[0].size_in_lba = TOTAL_SECTORS - 1;
    if (pwrite(fd, &mbr, sizeof(mbr), 0) != sizeof(mbr)) {
        perror("pwrite MBR");
        close(fd);
        return 1;
    }

    // 2. Prepare Partition Entry (LBA 2)
    uint8_t entries_buf[128 * 128];
    memset(entries_buf, 0, sizeof(entries_buf));
    gpt_entry_t *entry = (gpt_entry_t *)entries_buf;
    memcpy(&entry->partition_type_guid, &GPT_GUID_PURPLEFS_V2, sizeof(gpt_guid_t));
    entry->unique_partition_guid.data1 = 0xA1B2C3D4;
    entry->starting_lba = PART_START_LBA;
    entry->ending_lba = PART_END_LBA;
    const char *pname = "PurpleRootFS";
    for (size_t i = 0; i < strlen(pname); i++) {
        entry->partition_name[i] = (uint16_t)pname[i];
    }

    if (pwrite(fd, entries_buf, sizeof(entries_buf), 2 * SECTOR_SIZE) != sizeof(entries_buf)) {
        perror("pwrite partition entries");
        close(fd);
        return 1;
    }

    // 3. Prepare Primary GPT Header (LBA 1)
    gpt_header_t hdr;
    memset(&hdr, 0, sizeof(hdr));
    hdr.signature = GPT_SIGNATURE;
    hdr.revision = GPT_REVISION_1_0;
    hdr.header_size = 92;
    hdr.my_lba = 1;
    hdr.alternate_lba = TOTAL_SECTORS - 1;
    hdr.first_usable_lba = 34;
    hdr.last_usable_lba = TOTAL_SECTORS - 34;
    hdr.partition_entry_lba = 2;
    hdr.num_partition_entries = 128;
    hdr.size_of_partition_entry = 128;
    hdr.partition_entry_array_crc32 = pfs2_crc32(entries_buf, sizeof(entries_buf));
    hdr.header_crc32 = 0;
    hdr.header_crc32 = pfs2_crc32(&hdr, hdr.header_size);

    if (pwrite(fd, &hdr, sizeof(hdr), 1 * SECTOR_SIZE) != sizeof(hdr)) {
        perror("pwrite GPT header");
        close(fd);
        return 1;
    }

    // 4. Format PurpleFS v2 filesystem using host memory device
    pfs2_block_dev_t *mem_dev = pfs2_mem_dev_create(PART_BLOCKS);
    if (!mem_dev) {
        fprintf(stderr, "Failed to create mem dev\n");
        close(fd);
        return 1;
    }

    if (pfs2_format(mem_dev, PART_BLOCKS, "PurpleSystem") != PFS2_OK) {
        fprintf(stderr, "Failed to format PurpleFS\n");
        pfs2_mem_dev_destroy(mem_dev);
        close(fd);
        return 1;
    }

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, mem_dev) != PFS2_OK) {
        fprintf(stderr, "Failed to mount PurpleFS\n");
        pfs2_mem_dev_destroy(mem_dev);
        close(fd);
        return 1;
    }

    // Create sample files
    uint32_t ino1 = 0, ino2 = 0, ino3 = 0;
    pfs2_create_file(&fs, "welcome.txt", 0644, &ino1);
    const char *msg1 = "Welcome to Purple OS / Aegis64 Native Filesystem!\nArchitecture: x86-64 | Kernel: Aegis64\nFilesystem: PurpleFS v2 Persistent Storage\n";
    size_t w1 = 0;
    pfs2_write_file(&fs, ino1, msg1, strlen(msg1), 0, &w1);

    pfs2_create_file(&fs, "version.txt", 0644, &ino2);
    const char *msg2 = "Aegis64 Kernel v0.4.0 (Stage 4B Safe Kernel Storage Integration)\n";
    size_t w2 = 0;
    pfs2_write_file(&fs, ino2, msg2, strlen(msg2), 0, &w2);

    pfs2_create_file(&fs, "motd", 0644, &ino3);
    const char *msg3 = "Purple OS: Correctness, Memory Safety, and Recoverability.\n";
    size_t w3 = 0;
    pfs2_write_file(&fs, ino3, msg3, strlen(msg3), 0, &w3);

    pfs2_unmount(&fs);

    // 5. Copy formatted blocks from mem_dev into partition offset of disk image
    off_t part_byte_offset = (off_t)PART_START_LBA * SECTOR_SIZE;
    uint8_t *fs_data = pfs2_mem_dev_get_data(mem_dev);
    size_t fs_total_bytes = (size_t)PART_BLOCKS * PFS2_BLOCK_SIZE;

    if (pwrite(fd, fs_data, fs_total_bytes, part_byte_offset) != (ssize_t)fs_total_bytes) {
        perror("pwrite filesystem partition data");
        pfs2_mem_dev_destroy(mem_dev);
        close(fd);
        return 1;
    }

    pfs2_mem_dev_destroy(mem_dev);
    fdatasync(fd);
    close(fd);

    printf("Successfully created disposable GPT disk with PurpleFS v2 partition: %s\n", img_path);
    return 0;
}
