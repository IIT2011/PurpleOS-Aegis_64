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

int main(void) {
    printf("=====================================================\n");
    printf(" Running PurpleFS v2 Host Test Suite (13 Test Cases) \n");
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
    RUN_TEST(test_fault_injection_failures);
    RUN_TEST(test_repeated_mount_unmount);

    printf("=====================================================\n");
    printf(" PurpleFS v2 Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
