#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <assert.h>

#include "purplefs_v2.h"
#include "host_purplefs_v2.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %2d] Running %-45s... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        printf("PASSED\n"); \
        tests_passed++; \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while (0)

static int test_format_and_mount(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;

    int res = pfs2_format(dev, 1024, "test_vol");
    if (res != PFS2_OK) { pfs2_mem_dev_destroy(dev); return 0; }

    pfs2_fs_t fs;
    res = pfs2_mount(&fs, dev);
    if (res != PFS2_OK) { pfs2_mem_dev_destroy(dev); return 0; }

    if (fs.sb.version != 2) return 0;
    if (fs.sb.block_size != 4096) return 0;
    if (fs.sb.total_blocks != 1024) return 0;
    if (strcmp(fs.sb.volume_label, "test_vol") != 0) return 0;
    if (fs.sb.clean_unmount != 0) return 0; // Active mount is dirty

    res = pfs2_unmount(&fs);
    if (res != PFS2_OK) { pfs2_mem_dev_destroy(dev); return 0; }

    // Re-read superblock to check clean_unmount
    uint8_t sb_buf[PFS2_BLOCK_SIZE];
    dev->read_block(dev, 0, sb_buf);
    pfs2_superblock_t *sb = (pfs2_superblock_t *)sb_buf;
    if (sb->clean_unmount != 1) return 0;

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_invalid_signature_and_corrupt_sb(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "corrupt_test");

    // Test bad signature
    uint8_t sb_buf[PFS2_BLOCK_SIZE];
    dev->read_block(dev, 0, sb_buf);
    sb_buf[0] = 'X'; // Corrupt signature
    dev->write_block(dev, 0, sb_buf);

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, dev) != PFS2_ERR_BAD_MAGIC) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Restore signature, corrupt CRC
    pfs2_format(dev, 1024, "corrupt_test");
    dev->read_block(dev, 0, sb_buf);
    pfs2_superblock_t *sb = (pfs2_superblock_t *)sb_buf;
    sb->checksum ^= 0xFFFFFFFF; // Corrupt CRC
    dev->write_block(dev, 0, sb_buf);

    if (pfs2_mount(&fs, dev) != PFS2_ERR_CORRUPT) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_volume_size_limits(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(65536);

    // Too small (< 32 blocks)
    if (pfs2_format(dev, 16, "small") != PFS2_ERR_INVALID) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Too large (> 32768 blocks limit for v2 baseline)
    if (pfs2_format(dev, 32769, "large") != PFS2_ERR_INVALID) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Exact limit 32768 blocks (128 MiB)
    if (pfs2_format(dev, 32768, "max_v2") != PFS2_OK) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_file_create_and_lookup(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "fs_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino1 = 0;
    int res = pfs2_create_file(&fs, "test.txt", 0644, &ino1);
    if (res != PFS2_OK || ino1 < 2) { pfs2_mem_dev_destroy(dev); return 0; }

    uint32_t found_ino = 0;
    res = pfs2_lookup(&fs, "test.txt", &found_ino);
    if (res != PFS2_OK || found_ino != ino1) { pfs2_mem_dev_destroy(dev); return 0; }

    // Duplicate create must fail
    res = pfs2_create_file(&fs, "test.txt", 0644, &found_ino);
    if (res != PFS2_ERR_EXISTS) { pfs2_mem_dev_destroy(dev); return 0; }

    // Non-existent lookup must fail
    res = pfs2_lookup(&fs, "nonexistent.txt", &found_ino);
    if (res != PFS2_ERR_NOT_FOUND) { pfs2_mem_dev_destroy(dev); return 0; }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_file_read_write_small(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "fs_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    pfs2_create_file(&fs, "hello.txt", 0644, &ino);

    const char *msg = "Hello, PurpleFS v2 native filesystem!";
    size_t msg_len = strlen(msg);
    size_t written = 0;
    int res = pfs2_write_file(&fs, ino, msg, msg_len, 0, &written);
    if (res != PFS2_OK || written != msg_len) { pfs2_mem_dev_destroy(dev); return 0; }

    char read_buf[64];
    memset(read_buf, 0, sizeof(read_buf));
    size_t read_bytes = 0;
    res = pfs2_read_file(&fs, ino, read_buf, sizeof(read_buf), 0, &read_bytes);
    if (res != PFS2_OK || read_bytes != msg_len) { pfs2_mem_dev_destroy(dev); return 0; }
    if (strcmp(read_buf, msg) != 0) { pfs2_mem_dev_destroy(dev); return 0; }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_empty_and_multiblock_files(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(2048);
    pfs2_format(dev, 2048, "fs_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    // 1. Empty file
    uint32_t empty_ino = 0;
    pfs2_create_file(&fs, "empty.bin", 0644, &empty_ino);
    pfs2_inode_t ino_st;
    pfs2_read_inode(&fs, empty_ino, &ino_st);
    if (ino_st.size_bytes != 0) return 0;

    char dummy[16];
    size_t bytes_read = 0;
    pfs2_read_file(&fs, empty_ino, dummy, sizeof(dummy), 0, &bytes_read);
    if (bytes_read != 0) return 0;

    // 2. Multi-block file: 40,960 bytes (10 blocks = direct blocks)
    uint32_t direct_ino = 0;
    pfs2_create_file(&fs, "direct10.bin", 0644, &direct_ino);
    size_t direct_size = 40960;
    uint8_t *direct_data = (uint8_t *)malloc(direct_size);
    for (size_t i = 0; i < direct_size; i++) direct_data[i] = (uint8_t)(i & 0xFF);

    size_t written = 0;
    pfs2_write_file(&fs, direct_ino, direct_data, direct_size, 0, &written);
    if (written != direct_size) { free(direct_data); return 0; }

    uint8_t *read_direct = (uint8_t *)malloc(direct_size);
    pfs2_read_file(&fs, direct_ino, read_direct, direct_size, 0, &bytes_read);
    if (bytes_read != direct_size || memcmp(direct_data, read_direct, direct_size) != 0) {
        free(direct_data); free(read_direct); return 0;
    }
    free(direct_data); free(read_direct);

    // 3. Indirect block file: 65,536 bytes (16 blocks = 12 direct + 4 indirect)
    uint32_t ind_ino = 0;
    pfs2_create_file(&fs, "indirect.bin", 0644, &ind_ino);
    size_t ind_size = 65536;
    uint8_t *ind_data = (uint8_t *)malloc(ind_size);
    for (size_t i = 0; i < ind_size; i++) ind_data[i] = (uint8_t)((i * 7) & 0xFF);

    pfs2_write_file(&fs, ind_ino, ind_data, ind_size, 0, &written);
    if (written != ind_size) { free(ind_data); return 0; }

    uint8_t *read_ind = (uint8_t *)malloc(ind_size);
    pfs2_read_file(&fs, ind_ino, read_ind, ind_size, 0, &bytes_read);
    if (bytes_read != ind_size || memcmp(ind_data, read_ind, ind_size) != 0) {
        free(ind_data); free(read_ind); return 0;
    }
    free(ind_data); free(read_ind);

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_truncate_file(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "trunc_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    pfs2_create_file(&fs, "trunc.dat", 0644, &ino);

    // Write 65,536 bytes
    size_t sz = 65536;
    uint8_t *buf = (uint8_t *)malloc(sz);
    memset(buf, 0x5A, sz);
    size_t written = 0;
    pfs2_write_file(&fs, ino, buf, sz, 0, &written);

    uint32_t free_before = fs.sb.free_blocks;

    // Truncate to 20,480 bytes (5 blocks instead of 16 blocks + 1 indirect block)
    int res = pfs2_truncate_file(&fs, ino, 20480);
    if (res != PFS2_OK) { free(buf); return 0; }

    pfs2_inode_t ino_st;
    pfs2_read_inode(&fs, ino, &ino_st);
    if (ino_st.size_bytes != 20480) { free(buf); return 0; }

    // Freed blocks must have returned to pool
    if (fs.sb.free_blocks <= free_before) { free(buf); return 0; }

    // Verify first 20,480 bytes are still intact
    uint8_t *check_buf = (uint8_t *)malloc(20480);
    size_t read_bytes = 0;
    pfs2_read_file(&fs, ino, check_buf, 20480, 0, &read_bytes);
    if (read_bytes != 20480 || memcmp(check_buf, buf, 20480) != 0) {
        free(buf); free(check_buf); return 0;
    }
    free(buf); free(check_buf);

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_regular_file_persistence(void) {
    const char *img_path = "build/test_persistence.img";
    unlink(img_path);

    pfs2_block_dev_t *fdev = pfs2_file_dev_create(img_path, 1024);
    if (!fdev) return 0;

    pfs2_format(fdev, 1024, "persistent");

    pfs2_fs_t fs1;
    pfs2_mount(&fs1, fdev);

    uint32_t ino = 0;
    pfs2_create_file(&fs1, "persist.dat", 0644, &ino);

    // Write 9,000 bytes
    size_t data_len = 9000;
    uint8_t *data = (uint8_t *)malloc(data_len);
    for (size_t i = 0; i < data_len; i++) data[i] = (uint8_t)((i * 13) & 0xFF);

    size_t written = 0;
    pfs2_write_file(&fs1, ino, data, data_len, 0, &written);
    if (written != data_len) { free(data); return 0; }

    pfs2_unmount(&fs1);
    pfs2_file_dev_destroy(fdev); // Close file completely

    // Re-open image file with a completely separate dev & fs instance!
    pfs2_block_dev_t *reopened_dev = pfs2_file_dev_create(img_path, 1024);
    if (!reopened_dev) { free(data); return 0; }

    pfs2_fs_t fs2;
    if (pfs2_mount(&fs2, reopened_dev) != PFS2_OK) {
        free(data); pfs2_file_dev_destroy(reopened_dev); return 0;
    }

    uint32_t found_ino = 0;
    if (pfs2_lookup(&fs2, "persist.dat", &found_ino) != PFS2_OK) {
        free(data); return 0;
    }

    uint8_t *read_back = (uint8_t *)malloc(data_len);
    size_t bytes_read = 0;
    pfs2_read_file(&fs2, found_ino, read_back, data_len, 0, &bytes_read);
    if (bytes_read != data_len || memcmp(data, read_back, data_len) != 0) {
        free(data); free(read_back); return 0;
    }
    free(data); free(read_back);

    pfs2_unmount(&fs2);
    pfs2_file_dev_destroy(reopened_dev);
    unlink(img_path);
    return 1;
}

static int test_allocation_exhaustion(void) {
    // 32-block small volume: data starts at block 10, journal occupies 24..31
    // Available data blocks = 32 - 10 - 8 - 1 (root dir) = 13 blocks = 53,248 bytes
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(32);
    pfs2_format(dev, 32, "tiny_fs");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    pfs2_create_file(&fs, "exhaust.dat", 0644, &ino);

    // Try writing 100,000 bytes (exceeds total capacity)
    uint8_t *big_buf = (uint8_t *)malloc(100000);
    memset(big_buf, 0x33, 100000);

    size_t written = 0;
    int res = pfs2_write_file(&fs, ino, big_buf, 100000, 0, &written);
    free(big_buf);

    // Must safely report no space without crashing or corrupting
    if (res != PFS2_ERR_NO_SPACE) return 0;
    if (fs.sb.free_blocks != 0) return 0;

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_corrupt_inode_rejection(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "crc_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    pfs2_create_file(&fs, "check.bin", 0644, &ino);

    // Corrupt inode checksum on disk
    uint8_t ino_block[PFS2_BLOCK_SIZE];
    dev->read_block(dev, fs.sb.inode_start_block, ino_block);
    pfs2_inode_t *disk_ino = (pfs2_inode_t *)(ino_block + ino * PFS2_INODE_SIZE);
    disk_ino->checksum ^= 0x12345678;
    dev->write_block(dev, fs.sb.inode_start_block, ino_block);

    pfs2_inode_t test_ino;
    int res = pfs2_read_inode(&fs, ino, &test_ino);
    if (res != PFS2_ERR_CORRUPT) return 0;

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_journal_crash_recovery(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "journal_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    // Simulate an interrupted commit:
    // Create an after-image that modifies root dir entry with "recovered.txt"
    uint32_t j_start = fs.sb.journal_start_block;

    // 1. Write modified root directory data block to journal slot 1
    uint8_t new_dir_block[PFS2_BLOCK_SIZE];
    dev->read_block(dev, PFS2_DATA_START_BLOCK, new_dir_block);

    pfs2_dir_entry_t *rec_de = (pfs2_dir_entry_t *)(new_dir_block + 64);
    rec_de->inode = 5;
    rec_de->rec_len = 32;
    rec_de->name_len = 13;
    rec_de->file_type = 1;
    strcpy(rec_de->name, "recovered.txt");

    dev->write_block(dev, j_start + 1, new_dir_block);

    // 2. Write COMMITTED journal header
    pfs2_journal_header_t j_hdr;
    memset(&j_hdr, 0, sizeof(j_hdr));
    j_hdr.magic = PFS2_JOURNAL_MAGIC;
    j_hdr.journal_state = PFS2_JOURNAL_STATE_COMMITTED; // Interrupted here!
    j_hdr.sequence_number = 42;
    j_hdr.num_after_images = 1;
    j_hdr.target_blocks[0] = PFS2_DATA_START_BLOCK;
    j_hdr.after_image_crcs[0] = pfs2_crc32(new_dir_block, PFS2_BLOCK_SIZE);
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24);

    uint8_t hdr_block[PFS2_BLOCK_SIZE];
    memset(hdr_block, 0, PFS2_BLOCK_SIZE);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));
    dev->write_block(dev, j_start, hdr_block);
    dev->flush(dev);

    // Clean unmount memory state without modifying disk
    fs.is_mounted = 0;

    // Mount anew! Journal replay MUST detect COMMITTED state and apply it!
    pfs2_fs_t fs2;
    if (pfs2_mount(&fs2, dev) != PFS2_OK) return 0;

    // Verify "recovered.txt" was restored onto disk
    uint32_t recovered_ino = 0;
    if (pfs2_lookup(&fs2, "recovered.txt", &recovered_ino) != PFS2_OK || recovered_ino != 5) {
        return 0;
    }

    // Verify journal header state was transitioned to APPLIED
    dev->read_block(dev, j_start, hdr_block);
    pfs2_journal_header_t *chk_hdr = (pfs2_journal_header_t *)hdr_block;
    if (chk_hdr->journal_state != PFS2_JOURNAL_STATE_APPLIED) return 0;

    pfs2_unmount(&fs2);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_journal_corrupt_after_image_rejection(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "crc_fail_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);
    uint32_t j_start = fs.sb.journal_start_block;

    // Write garbage in journal slot 1
    uint8_t garbage[PFS2_BLOCK_SIZE];
    memset(garbage, 0xEE, PFS2_BLOCK_SIZE);
    dev->write_block(dev, j_start + 1, garbage);

    // Write COMMITTED header with WRONG CRC
    pfs2_journal_header_t j_hdr;
    memset(&j_hdr, 0, sizeof(j_hdr));
    j_hdr.magic = PFS2_JOURNAL_MAGIC;
    j_hdr.journal_state = PFS2_JOURNAL_STATE_COMMITTED;
    j_hdr.sequence_number = 99;
    j_hdr.num_after_images = 1;
    j_hdr.target_blocks[0] = PFS2_DATA_START_BLOCK;
    j_hdr.after_image_crcs[0] = 0x12345678; // Incorrect CRC!
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24);

    uint8_t hdr_block[PFS2_BLOCK_SIZE];
    memset(hdr_block, 0, PFS2_BLOCK_SIZE);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));
    dev->write_block(dev, j_start, hdr_block);
    dev->flush(dev);

    fs.is_mounted = 0;

    // Mount MUST detect CRC corruption and reject replay!
    pfs2_fs_t fs2;
    int mount_res = pfs2_mount(&fs2, dev);
    if (mount_res != PFS2_ERR_CORRUPT) return 0;

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_fault_injection_failures(void) {
    pfs2_block_dev_t *base_dev = pfs2_mem_dev_create(1024);
    pfs2_format(base_dev, 1024, "fault_test");

    pfs2_block_dev_t *fault_dev = pfs2_fault_dev_create(base_dev);
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)fault_dev;

    // Test read failure during mount
    fdev->fail_reads = 1;
    pfs2_fs_t fs;
    if (pfs2_mount(&fs, fault_dev) != PFS2_ERR_IO) {
        pfs2_fault_dev_destroy(fault_dev);
        pfs2_mem_dev_destroy(base_dev);
        return 0;
    }

    // Reset and mount successfully
    pfs2_fault_dev_reset(fault_dev);
    if (pfs2_mount(&fs, fault_dev) != PFS2_OK) return 0;

    // Test write failure during file write
    uint32_t ino = 0;
    pfs2_create_file(&fs, "fail.dat", 0644, &ino);

    fdev->fail_writes = 1;
    char data[64];
    size_t written = 0;
    if (pfs2_write_file(&fs, ino, data, sizeof(data), 0, &written) != PFS2_ERR_IO) {
        return 0;
    }

    pfs2_fault_dev_destroy(fault_dev);
    pfs2_mem_dev_destroy(base_dev);
    return 1;
}

static int test_journal_corrupt_header_crc(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "hdr_crc_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);
    uint32_t j_start = fs.sb.journal_start_block;

    // Write a valid after image
    uint8_t dummy[PFS2_BLOCK_SIZE];
    memset(dummy, 0xAA, PFS2_BLOCK_SIZE);
    dev->write_block(dev, j_start + 1, dummy);

    // Prepare header with valid magic and state, but deliberate bit-flip in header_crc
    pfs2_journal_header_t j_hdr;
    memset(&j_hdr, 0, sizeof(j_hdr));
    j_hdr.magic = PFS2_JOURNAL_MAGIC;
    j_hdr.journal_state = PFS2_JOURNAL_STATE_COMMITTED;
    j_hdr.sequence_number = 7;
    j_hdr.num_after_images = 1;
    j_hdr.target_blocks[0] = PFS2_DATA_START_BLOCK;
    j_hdr.after_image_crcs[0] = pfs2_crc32(dummy, PFS2_BLOCK_SIZE);
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24) ^ 0xFFFFFFFF; // Corrupt!

    uint8_t hdr_block[PFS2_BLOCK_SIZE];
    memset(hdr_block, 0, PFS2_BLOCK_SIZE);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));
    dev->write_block(dev, j_start, hdr_block);
    dev->flush(dev);

    fs.is_mounted = 0;

    // Mounting MUST detect corrupt header CRC and refuse replay
    pfs2_fs_t fs2;
    int rc = pfs2_mount(&fs2, dev);
    if (rc != PFS2_ERR_CORRUPT) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_journal_torn_header_write(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "torn_hdr_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);
    uint32_t j_start = fs.sb.journal_start_block;

    // Simulate torn write: invalid magic bytes in header block
    uint8_t torn_block[PFS2_BLOCK_SIZE];
    memset(torn_block, 0xCD, PFS2_BLOCK_SIZE); // Random unwritten/torn data
    dev->write_block(dev, j_start, torn_block);
    dev->flush(dev);

    fs.is_mounted = 0;

    // Mounting MUST reject bad journal magic and return error
    pfs2_fs_t fs2;
    int rc = pfs2_mount(&fs2, dev);
    if (rc != PFS2_ERR_JOURNAL) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_journal_crash_during_prep(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "crash_prep_test");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);
    uint32_t j_start = fs.sb.journal_start_block;

    // Create an initial file
    uint32_t ino = 0;
    pfs2_create_file(&fs, "original.txt", 0644, &ino);
    const char *orig_msg = "Original Content";
    size_t written = 0;
    pfs2_write_file(&fs, ino, orig_msg, strlen(orig_msg), 0, &written);

    // Simulate crash during journal prep:
    // Write staged after-image containing new directory data to j_start + 1
    uint8_t staged_dir[PFS2_BLOCK_SIZE];
    memset(staged_dir, 0, PFS2_BLOCK_SIZE);
    pfs2_dir_entry_t *de = (pfs2_dir_entry_t *)staged_dir;
    de->inode = 99;
    de->name_len = 11;
    strcpy(de->name, "phantom.txt");
    dev->write_block(dev, j_start + 1, staged_dir);

    // But journal header remains in state EMPTY! (Crash before commit header written)
    pfs2_journal_header_t j_hdr;
    memset(&j_hdr, 0, sizeof(j_hdr));
    j_hdr.magic = PFS2_JOURNAL_MAGIC;
    j_hdr.journal_state = PFS2_JOURNAL_STATE_EMPTY;
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24);

    uint8_t hdr_block[PFS2_BLOCK_SIZE];
    memset(hdr_block, 0, PFS2_BLOCK_SIZE);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));
    dev->write_block(dev, j_start, hdr_block);
    dev->flush(dev);

    fs.is_mounted = 0;

    // Remount: Mount MUST succeed because state is EMPTY, and must NOT replay phantom.txt!
    pfs2_fs_t fs2;
    if (pfs2_mount(&fs2, dev) != PFS2_OK) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    uint32_t phantom_ino = 0;
    if (pfs2_lookup(&fs2, "phantom.txt", &phantom_ino) == PFS2_OK) {
        // Phantom file was erroneously applied!
        pfs2_unmount(&fs2);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Original file must remain intact
    uint32_t orig_ino = 0;
    if (pfs2_lookup(&fs2, "original.txt", &orig_ino) != PFS2_OK) {
        pfs2_unmount(&fs2);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_unmount(&fs2);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_journal_flush_failure(void) {
    pfs2_block_dev_t *base_dev = pfs2_mem_dev_create(1024);
    pfs2_format(base_dev, 1024, "flush_fail_test");

    pfs2_block_dev_t *fault_dev = pfs2_fault_dev_create(base_dev);
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)fault_dev;

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, fault_dev) != PFS2_OK) {
        pfs2_fault_dev_destroy(fault_dev);
        pfs2_mem_dev_destroy(base_dev);
        return 0;
    }

    // Fail flushes during transaction commit
    fdev->fail_flushes = 1;
    uint32_t ino = 0;
    int rc = pfs2_create_file(&fs, "will_fail.txt", 0644, &ino);
    if (rc != PFS2_ERR_IO) {
        pfs2_fault_dev_destroy(fault_dev);
        pfs2_mem_dev_destroy(base_dev);
        return 0;
    }

    pfs2_fault_dev_destroy(fault_dev);
    pfs2_mem_dev_destroy(base_dev);
    return 1;
}

static int test_repeated_mount_unmount(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "repeat_test");

    for (int i = 0; i < 25; i++) {
        pfs2_fs_t fs;
        if (pfs2_mount(&fs, dev) != PFS2_OK) {
            pfs2_mem_dev_destroy(dev);
            return 0;
        }
        if (fs.sb.mount_count != (uint32_t)(i + 1)) {
            pfs2_mem_dev_destroy(dev);
            return 0;
        }
        if (pfs2_unmount(&fs) != PFS2_OK) {
            pfs2_mem_dev_destroy(dev);
            return 0;
        }
    }

    pfs2_mem_dev_destroy(dev);
    return 1;
}

/* ========================================================================= */
/* Focused Verification Tests (Audit Phase)                                 */
/* ========================================================================= */

static int test_read_write_block_boundary_spans(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    if (pfs2_format(dev, 1024, "boundary_vol") != PFS2_OK) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, dev) != PFS2_OK) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // 1. Spanning exact 1-block boundary (offset 4090, 20 bytes: 6 in blk 0, 14 in blk 1)
    uint32_t ino1 = 0;
    if (pfs2_create_file(&fs, "span1.bin", 0644, &ino1) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    uint8_t span_data[20];
    for (int i = 0; i < 20; i++) span_data[i] = (uint8_t)(0xA0 + i);

    size_t written = 0;
    int res = pfs2_write_file(&fs, ino1, span_data, sizeof(span_data), 4090, &written);
    if (res != PFS2_OK || written != 20) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_inode_t ino_st;
    pfs2_read_inode(&fs, ino1, &ino_st);
    if (ino_st.size_bytes != 4110) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Read full span back
    uint8_t read_back[20];
    size_t bytes_read = 0;
    res = pfs2_read_file(&fs, ino1, read_back, sizeof(read_back), 4090, &bytes_read);
    if (res != PFS2_OK || bytes_read != 20 || memcmp(span_data, read_back, 20) != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Read unaligned subset across the boundary (offset 4094, 10 bytes: 2 in blk 0, 8 in blk 1)
    uint8_t read_sub[10];
    res = pfs2_read_file(&fs, ino1, read_sub, sizeof(read_sub), 4094, &bytes_read);
    if (res != PFS2_OK || bytes_read != 10 || memcmp(read_sub, span_data + 4, 10) != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 2. Spanning 3 blocks (offset 4000, 9000 bytes: blk 0 [4000..4095], blk 1 [4096 bytes], blk 2 [4808 bytes])
    uint32_t ino2 = 0;
    if (pfs2_create_file(&fs, "span3.bin", 0644, &ino2) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    size_t big_sz = 9000;
    uint8_t *big_buf = (uint8_t *)malloc(big_sz);
    for (size_t i = 0; i < big_sz; i++) big_buf[i] = (uint8_t)((i * 31 + 7) & 0xFF);

    res = pfs2_write_file(&fs, ino2, big_buf, big_sz, 4000, &written);
    if (res != PFS2_OK || written != big_sz) {
        free(big_buf); pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    uint8_t *read_big = (uint8_t *)malloc(big_sz);
    res = pfs2_read_file(&fs, ino2, read_big, big_sz, 4000, &bytes_read);
    if (res != PFS2_OK || bytes_read != big_sz || memcmp(big_buf, read_big, big_sz) != 0) {
        free(big_buf); free(read_big); pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    free(big_buf);
    free(read_big);

    // 3. Sparse file block hole read/write (write at offset 16384, verify blocks 0..3 read zeroes)
    uint32_t ino3 = 0;
    if (pfs2_create_file(&fs, "sparse.bin", 0644, &ino3) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    uint8_t sparse_pat[128];
    for (int i = 0; i < 128; i++) sparse_pat[i] = (uint8_t)(0xC0 + (i & 0x0F));
    res = pfs2_write_file(&fs, ino3, sparse_pat, sizeof(sparse_pat), 16384, &written);
    if (res != PFS2_OK || written != 128) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Verify hole reading: first 4096 bytes must be zero
    uint8_t zero_check[PFS2_BLOCK_SIZE];
    res = pfs2_read_file(&fs, ino3, zero_check, sizeof(zero_check), 0, &bytes_read);
    if (res != PFS2_OK || bytes_read != PFS2_BLOCK_SIZE) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    for (size_t i = 0; i < PFS2_BLOCK_SIZE; i++) {
        if (zero_check[i] != 0) {
            pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
        }
    }

    // Verify sparse payload
    uint8_t payload_check[128];
    res = pfs2_read_file(&fs, ino3, payload_check, sizeof(payload_check), 16384, &bytes_read);
    if (res != PFS2_OK || bytes_read != 128 || memcmp(sparse_pat, payload_check, 128) != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_empty_file_comprehensive(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    pfs2_format(dev, 1024, "empty_vol");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    if (pfs2_create_file(&fs, "zero.txt", 0644, &ino) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_inode_t ino_st;
    pfs2_read_inode(&fs, ino, &ino_st);
    if (ino_st.size_bytes != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Read 0 bytes at offset 0
    char buf[32];
    size_t bytes_read = 99;
    int res = pfs2_read_file(&fs, ino, buf, 0, 0, &bytes_read);
    if (res != PFS2_OK || bytes_read != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Read non-zero count at offset 0 (EOF)
    bytes_read = 99;
    res = pfs2_read_file(&fs, ino, buf, sizeof(buf), 0, &bytes_read);
    if (res != PFS2_OK || bytes_read != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Read at positive offset
    bytes_read = 99;
    res = pfs2_read_file(&fs, ino, buf, sizeof(buf), 100, &bytes_read);
    if (res != PFS2_OK || bytes_read != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Write 0 bytes to empty file
    size_t written = 99;
    res = pfs2_write_file(&fs, ino, buf, 0, 0, &written);
    if (res != PFS2_OK || written != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Truncate empty file to 0
    res = pfs2_truncate_file(&fs, ino, 0);
    if (res != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    pfs2_read_inode(&fs, ino, &ino_st);
    if (ino_st.size_bytes != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_missing_files_and_invalid_paths(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    pfs2_format(dev, 1024, "path_vol");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    // 1. Missing file lookup
    uint32_t out_ino = 0;
    if (pfs2_lookup(&fs, "does_not_exist.txt", &out_ino) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 2. NULL checks on lookup
    if (pfs2_lookup(&fs, NULL, &out_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_lookup(NULL, "test", &out_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 3. Lookup on unmounted fs
    pfs2_fs_t unmounted_fs;
    memset(&unmounted_fs, 0, sizeof(unmounted_fs));
    if (pfs2_lookup(&unmounted_fs, "test", &out_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 4. Dot and dot-dot lookup
    uint32_t dot_ino = 0, dotdot_ino = 0;
    if (pfs2_lookup(&fs, ".", &dot_ino) != PFS2_OK || dot_ino != PFS2_ROOT_INODE) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_lookup(&fs, "..", &dotdot_ino) != PFS2_OK || dotdot_ino != PFS2_ROOT_INODE) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 5. Nested path lookups (confirming subdirectories/paths are unsupported)
    if (pfs2_lookup(&fs, "dir/file.txt", &out_ino) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_lookup(&fs, "/root", &out_ino) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 6. Invalid file creation parameters
    uint32_t create_ino = 0;
    // Empty name
    if (pfs2_create_file(&fs, "", 0644, &create_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    // Name too long (24 chars)
    if (pfs2_create_file(&fs, "123456789012345678901234", 0644, &create_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    // Name much too long
    if (pfs2_create_file(&fs, "very_long_file_name_exceeding_capacity_by_far.txt", 0644, &create_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    // NULL name
    if (pfs2_create_file(&fs, NULL, 0644, &create_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    // NULL fs
    if (pfs2_create_file(NULL, "valid.txt", 0644, &create_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_eof_and_out_of_bounds_rw(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    pfs2_format(dev, 1024, "oob_vol");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    uint32_t ino = 0;
    pfs2_create_file(&fs, "sample.dat", 0644, &ino);

    uint8_t payload[500];
    for (int i = 0; i < 500; i++) payload[i] = (uint8_t)(i & 0xFF);
    size_t written = 0;
    pfs2_write_file(&fs, ino, payload, sizeof(payload), 0, &written);

    // 1. Partial read at EOF boundary: offset 450, count 100 -> only 50 bytes available
    uint8_t buf[100];
    size_t bytes_read = 0;
    int res = pfs2_read_file(&fs, ino, buf, 100, 450, &bytes_read);
    if (res != PFS2_OK || bytes_read != 50 || memcmp(buf, payload + 450, 50) != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 2. Exact EOF read: offset 500, count 10 -> returns 0 bytes read
    bytes_read = 99;
    res = pfs2_read_file(&fs, ino, buf, 10, 500, &bytes_read);
    if (res != PFS2_OK || bytes_read != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 3. Past EOF read: offset 600, count 10 -> returns 0 bytes read
    bytes_read = 99;
    res = pfs2_read_file(&fs, ino, buf, 10, 600, &bytes_read);
    if (res != PFS2_OK || bytes_read != 0) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 4. Invalid buffer pointer
    if (pfs2_read_file(&fs, ino, NULL, 10, 0, &bytes_read) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_write_file(&fs, ino, NULL, 10, 0, &written) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 5. Invalid inode indices (0 and >= 256)
    if (pfs2_read_file(&fs, 0, buf, 10, 0, &bytes_read) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_read_file(&fs, 256, buf, 10, 0, &bytes_read) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 6. Unallocated inode (mode == 0)
    if (pfs2_read_file(&fs, 100, buf, 10, 0, &bytes_read) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 7. Directory inode read/write rejection
    if (pfs2_read_file(&fs, PFS2_ROOT_INODE, buf, 10, 0, &bytes_read) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_write_file(&fs, PFS2_ROOT_INODE, buf, 10, 0, &written) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 8. Write exceeding double indirect limit (> 1036 blocks = 4,243,456 bytes)
    uint64_t exceed_offset = (uint64_t)(12 + 1024) * PFS2_BLOCK_SIZE;
    if (pfs2_write_file(&fs, ino, "test", 4, exceed_offset, &written) != PFS2_ERR_TOO_BIG) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_boundary_filename_and_dir_limits(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    pfs2_format(dev, 1024, "limits_vol");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    // 1. Exact maximum filename length: 23 characters
    const char *max_name = "12345678901234567890123";
    uint32_t max_ino = 0;
    if (pfs2_create_file(&fs, max_name, 0644, &max_ino) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    uint32_t lookup_ino = 0;
    if (pfs2_lookup(&fs, max_name, &lookup_ino) != PFS2_OK || lookup_ino != max_ino) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 2. Exact 24 character filename (must be rejected)
    const char *too_long = "123456789012345678901234";
    uint32_t fail_ino = 0;
    if (pfs2_create_file(&fs, too_long, 0644, &fail_ino) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 3. Fill root directory to capacity:
    // Root directory block has 128 entries total:
    // Entry 0 = '.', Entry 1 = '..'
    // Entry 2 = max_name (created above)
    // There are 125 slots remaining (entries 3..127).
    for (int i = 1; i <= 125; i++) {
        char fname[24];
        snprintf(fname, sizeof(fname), "f_%03d.dat", i);
        uint32_t cur_ino = 0;
        int res = pfs2_create_file(&fs, fname, 0644, &cur_ino);
        if (res != PFS2_OK) {
            pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
        }
    }

    // Root directory is now full (128/128 entries occupied: 2 system + 126 user files)
    // Creating one more file must fail with PFS2_ERR_NO_SPACE
    uint32_t overflow_ino = 0;
    if (pfs2_create_file(&fs, "overflow.dat", 0644, &overflow_ino) != PFS2_ERR_NO_SPACE) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // Verify all 125 files plus max_name exist
    for (int i = 1; i <= 125; i++) {
        char fname[24];
        snprintf(fname, sizeof(fname), "f_%03d.dat", i);
        uint32_t chk_ino = 0;
        if (pfs2_lookup(&fs, fname, &chk_ino) != PFS2_OK) {
            pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
        }
    }

    // Verify superblock free inodes counter:
    // Total 256 inodes; allocated: root (1) + 126 user files = 127 inodes.
    // Remaining free inodes: 256 - 127 = 129.
    if (fs.sb.free_inodes != 129) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_directory_creation_and_traversal_unsupported(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    if (!dev) return 0;
    pfs2_format(dev, 1024, "dir_unsupp_vol");

    pfs2_fs_t fs;
    pfs2_mount(&fs, dev);

    // 1. Confirm root directory inode properties
    pfs2_inode_t root_st;
    if (pfs2_read_inode(&fs, PFS2_ROOT_INODE, &root_st) != PFS2_OK) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (!(root_st.mode & PFS2_MODE_DIR)) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (root_st.links_count != 2) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 2. Directory cannot be read via file read
    char buf[64];
    size_t bytes_read = 0;
    if (pfs2_read_file(&fs, PFS2_ROOT_INODE, buf, sizeof(buf), 0, &bytes_read) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 3. Directory cannot be written via file write
    size_t written = 0;
    if (pfs2_write_file(&fs, PFS2_ROOT_INODE, buf, sizeof(buf), 0, &written) != PFS2_ERR_INVALID) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    // 4. Nested paths return NOT_FOUND (traversal unsupported)
    uint32_t out_ino = 0;
    if (pfs2_lookup(&fs, "subdir/file.txt", &out_ino) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }
    if (pfs2_lookup(&fs, "a/b/c", &out_ino) != PFS2_ERR_NOT_FOUND) {
        pfs2_unmount(&fs); pfs2_mem_dev_destroy(dev); return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

static int test_persistence_multiblock_remount(void) {
    const char *img_path = "/tmp/test_persist_audit.img";
    unlink(img_path);

    pfs2_block_dev_t *fdev = pfs2_file_dev_create(img_path, 1024);
    if (!fdev) return 0;

    if (pfs2_format(fdev, 1024, "remount_audit") != PFS2_OK) {
        pfs2_file_dev_destroy(fdev);
        unlink(img_path);
        return 0;
    }

    pfs2_fs_t fs1;
    if (pfs2_mount(&fs1, fdev) != PFS2_OK) {
        pfs2_file_dev_destroy(fdev);
        unlink(img_path);
        return 0;
    }

    // Create 5 test files
    // File 1: small text file (45 bytes)
    uint32_t ino1 = 0;
    pfs2_create_file(&fs1, "small.txt", 0644, &ino1);
    const char *msg1 = "Audit Test: PurpleFS v2 persistence verification!";
    size_t written = 0;
    pfs2_write_file(&fs1, ino1, msg1, strlen(msg1), 0, &written);

    // File 2: exact 1 block (4096 bytes)
    uint32_t ino2 = 0;
    pfs2_create_file(&fs1, "exact1blk.bin", 0644, &ino2);
    uint8_t blk1_data[PFS2_BLOCK_SIZE];
    for (size_t i = 0; i < PFS2_BLOCK_SIZE; i++) blk1_data[i] = (uint8_t)((i * 17 + 3) & 0xFF);
    pfs2_write_file(&fs1, ino2, blk1_data, PFS2_BLOCK_SIZE, 0, &written);

    // File 3: exact 2 blocks (8192 bytes)
    uint32_t ino3 = 0;
    pfs2_create_file(&fs1, "exact2blk.bin", 0644, &ino3);
    uint8_t blk2_data[8192];
    for (size_t i = 0; i < 8192; i++) blk2_data[i] = (uint8_t)((i * 19 + 5) & 0xFF);
    pfs2_write_file(&fs1, ino3, blk2_data, 8192, 0, &written);

    // File 4: 10000 bytes (spans 3 blocks)
    uint32_t ino4 = 0;
    pfs2_create_file(&fs1, "span3blk.bin", 0644, &ino4);
    uint8_t span3_data[10000];
    for (size_t i = 0; i < 10000; i++) span3_data[i] = (uint8_t)((i * 23 + 11) & 0xFF);
    pfs2_write_file(&fs1, ino4, span3_data, 10000, 0, &written);

    // File 5: empty file (0 bytes)
    uint32_t ino5 = 0;
    pfs2_create_file(&fs1, "empty.dat", 0644, &ino5);

    // Sync and clean unmount
    if (pfs2_sync(&fs1) != PFS2_OK) {
        pfs2_unmount(&fs1); pfs2_file_dev_destroy(fdev); unlink(img_path); return 0;
    }
    if (pfs2_unmount(&fs1) != PFS2_OK) {
        pfs2_file_dev_destroy(fdev); unlink(img_path); return 0;
    }
    pfs2_file_dev_destroy(fdev);

    // =========================================================================
    // Remount Phase with totally fresh device & filesystem structures
    // =========================================================================
    pfs2_block_dev_t *reopen_dev = pfs2_file_dev_create(img_path, 1024);
    if (!reopen_dev) { unlink(img_path); return 0; }

    pfs2_fs_t fs2;
    if (pfs2_mount(&fs2, reopen_dev) != PFS2_OK) {
        pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // Verify mount count was incremented (2)
    if (fs2.sb.mount_count != 2) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // Check all 5 files persist byte-for-byte
    // 1. small.txt
    uint32_t chk_ino1 = 0;
    if (pfs2_lookup(&fs2, "small.txt", &chk_ino1) != PFS2_OK || chk_ino1 != ino1) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }
    char read_msg1[64];
    size_t bytes_read = 0;
    pfs2_read_file(&fs2, chk_ino1, read_msg1, sizeof(read_msg1), 0, &bytes_read);
    if (bytes_read != strlen(msg1) || memcmp(read_msg1, msg1, bytes_read) != 0) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // 2. exact1blk.bin
    uint32_t chk_ino2 = 0;
    if (pfs2_lookup(&fs2, "exact1blk.bin", &chk_ino2) != PFS2_OK || chk_ino2 != ino2) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }
    uint8_t read_blk1[PFS2_BLOCK_SIZE];
    pfs2_read_file(&fs2, chk_ino2, read_blk1, PFS2_BLOCK_SIZE, 0, &bytes_read);
    if (bytes_read != PFS2_BLOCK_SIZE || memcmp(read_blk1, blk1_data, PFS2_BLOCK_SIZE) != 0) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // 3. exact2blk.bin
    uint32_t chk_ino3 = 0;
    if (pfs2_lookup(&fs2, "exact2blk.bin", &chk_ino3) != PFS2_OK || chk_ino3 != ino3) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }
    uint8_t read_blk2[8192];
    pfs2_read_file(&fs2, chk_ino3, read_blk2, 8192, 0, &bytes_read);
    if (bytes_read != 8192 || memcmp(read_blk2, blk2_data, 8192) != 0) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // 4. span3blk.bin
    uint32_t chk_ino4 = 0;
    if (pfs2_lookup(&fs2, "span3blk.bin", &chk_ino4) != PFS2_OK || chk_ino4 != ino4) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }
    uint8_t read_span3[10000];
    pfs2_read_file(&fs2, chk_ino4, read_span3, 10000, 0, &bytes_read);
    if (bytes_read != 10000 || memcmp(read_span3, span3_data, 10000) != 0) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    // 5. empty.dat
    uint32_t chk_ino5 = 0;
    if (pfs2_lookup(&fs2, "empty.dat", &chk_ino5) != PFS2_OK || chk_ino5 != ino5) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }
    char empty_buf[16];
    pfs2_read_file(&fs2, chk_ino5, empty_buf, sizeof(empty_buf), 0, &bytes_read);
    if (bytes_read != 0) {
        pfs2_unmount(&fs2); pfs2_file_dev_destroy(reopen_dev); unlink(img_path); return 0;
    }

    pfs2_unmount(&fs2);
    pfs2_file_dev_destroy(reopen_dev);
    unlink(img_path);
    return 1;
}

static int test_unlink_readdir_rename(void) {
    pfs2_block_dev_t *dev = pfs2_mem_dev_create(1024);
    pfs2_format(dev, 1024, "ops_test");

    pfs2_fs_t fs;
    if (pfs2_mount(&fs, dev) != PFS2_OK) {
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    uint32_t ino = 0;
    if (pfs2_create_file(&fs, "file_a.txt", 0644, &ino) != PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    const char *msg = "Sample Content";
    size_t bw = 0;
    if (pfs2_write_file(&fs, ino, msg, strlen(msg), 0, &bw) != PFS2_OK || bw != strlen(msg)) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Readdir test
    pfs2_dir_entry_t ent;
    pfs2_inode_t ino_read;
    int found = 0;
    for (uint32_t i = 0; i < 10; i++) {
        if (pfs2_readdir(&fs, i, &ent, &ino_read) == PFS2_OK) {
            if (strcmp(ent.name, "file_a.txt") == 0) {
                found = 1;
                break;
            }
        }
    }
    if (!found) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Rename test
    if (pfs2_rename(&fs, "file_a.txt", "file_b.txt") != PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }
    uint32_t lookup_ino = 0;
    if (pfs2_lookup(&fs, "file_a.txt", &lookup_ino) == PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }
    if (pfs2_lookup(&fs, "file_b.txt", &lookup_ino) != PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    // Unlink test
    uint32_t free_before = fs.sb.free_inodes;
    if (pfs2_unlink(&fs, "file_b.txt") != PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }
    if (pfs2_lookup(&fs, "file_b.txt", &lookup_ino) == PFS2_OK) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }
    if (fs.sb.free_inodes != free_before + 1) {
        pfs2_unmount(&fs);
        pfs2_mem_dev_destroy(dev);
        return 0;
    }

    pfs2_unmount(&fs);
    pfs2_mem_dev_destroy(dev);
    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running PurpleFS v2 Host Test Suite (26 Test Cases) \n");
    printf("=====================================================\n");

    RUN_TEST(test_format_and_mount);
    RUN_TEST(test_invalid_signature_and_corrupt_sb);
    RUN_TEST(test_volume_size_limits);
    RUN_TEST(test_file_create_and_lookup);
    RUN_TEST(test_file_read_write_small);
    RUN_TEST(test_empty_and_multiblock_files);
    RUN_TEST(test_truncate_file);
    RUN_TEST(test_regular_file_persistence);
    RUN_TEST(test_allocation_exhaustion);
    RUN_TEST(test_corrupt_inode_rejection);
    RUN_TEST(test_journal_crash_recovery);
    RUN_TEST(test_journal_corrupt_after_image_rejection);
    RUN_TEST(test_journal_corrupt_header_crc);
    RUN_TEST(test_journal_torn_header_write);
    RUN_TEST(test_journal_crash_during_prep);
    RUN_TEST(test_journal_flush_failure);
    RUN_TEST(test_fault_injection_failures);
    RUN_TEST(test_repeated_mount_unmount);

    /* Focused Verification Tests (Audit Phase) */
    RUN_TEST(test_read_write_block_boundary_spans);
    RUN_TEST(test_empty_file_comprehensive);
    RUN_TEST(test_missing_files_and_invalid_paths);
    RUN_TEST(test_eof_and_out_of_bounds_rw);
    RUN_TEST(test_boundary_filename_and_dir_limits);
    RUN_TEST(test_directory_creation_and_traversal_unsupported);
    RUN_TEST(test_persistence_multiblock_remount);
    RUN_TEST(test_unlink_readdir_rename);

    printf("=====================================================\n");
    printf(" PurpleFS v2 Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
