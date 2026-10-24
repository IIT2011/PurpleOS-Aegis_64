#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <stdint.h>

#include "purplefs_v3.h"

/* Test runner macros */
static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %2d] Running %-50s... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        printf("PASSED\n"); \
        tests_passed++; \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while (0)

/* ========================================================================= */
/* Mock Block Device for Testing                                             */
/* ========================================================================= */

typedef struct {
    uint8_t *storage;
    uint64_t total_sectors;
    int flush_count;
} mem_bdev_priv_t;

static int mem_read_blocks(block_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)dev->priv;
    if (lba + count > dev->total_blocks) return BLOCK_DEV_ERR_BOUNDS;
    memcpy(buf, priv->storage + (lba * 512ULL), count * 512ULL);
    return BLOCK_DEV_OK;
}

static int mem_write_blocks(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)dev->priv;
    if (lba + count > dev->total_blocks) return BLOCK_DEV_ERR_BOUNDS;
    memcpy(priv->storage + (lba * 512ULL), buf, count * 512ULL);
    return BLOCK_DEV_OK;
}

static int mem_flush(block_device_t *dev) {
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)dev->priv;
    priv->flush_count++;
    return BLOCK_DEV_OK;
}

static const block_device_ops_t mem_ops = {
    .read_blocks = mem_read_blocks,
    .write_blocks = mem_write_blocks,
    .flush = mem_flush,
};

static block_device_t *create_mem_bdev(uint64_t total_sectors) {
    block_device_t *dev = (block_device_t *)calloc(1, sizeof(block_device_t));
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)calloc(1, sizeof(mem_bdev_priv_t));
    priv->storage = (uint8_t *)calloc(total_sectors, 512);
    priv->total_sectors = total_sectors;

    dev->block_size = 512;
    dev->total_blocks = total_sectors;
    dev->ops = &mem_ops;
    dev->priv = priv;
    strncpy(dev->name, "memdisk0", sizeof(dev->name) - 1);
    return dev;
}

static void destroy_mem_bdev(block_device_t *dev) {
    if (dev) {
        mem_bdev_priv_t *priv = (mem_bdev_priv_t *)dev->priv;
        if (priv) {
            free(priv->storage);
            free(priv);
        }
        free(dev);
    }
}

/* ========================================================================= */
/* Test Cases                                                                */
/* ========================================================================= */

/* Test 1: Static assertions on on-disk structures */
static int test_static_assertions(void) {
    assert(sizeof(pfs3_superblock_t) == 4096);
    assert(sizeof(pfs3_inode_t) == 256);
    assert(sizeof(pfs3_dirent_t) == 64);
    assert(sizeof(pfs3_indirect_block_t) == 4096);
    assert(sizeof(pfs3_journal_header_t) == 4096);
    assert(offsetof(pfs3_inode_t, checksum) == 0xA8);
    assert(offsetof(pfs3_dirent_t, name) == 0x10);
    return 1;
}

/* Test 2: Format and mount valid volume */
static int test_format_and_mount(void) {
    /* 10,000 sectors = 1250 blocks (4 KiB) */
    block_device_t *bdev = create_mem_bdev(10000);
    int rc = pfs3_format(bdev, 0, 10000, "PurpleSystem", NULL);
    if (rc != PFS3_OK) { destroy_mem_bdev(bdev); return 0; }

    pfs3_fs_t fs;
    rc = pfs3_mount(&fs, bdev, 0, 10000);
    if (rc != PFS3_OK) { destroy_mem_bdev(bdev); return 0; }

    if (fs.sb.magic != PFS3_MAGIC) return 0;
    if (fs.sb.version != PFS3_VERSION) return 0;
    if (fs.sb.block_size != 4096) return 0;
    if (fs.sb.total_blocks != 1250) return 0;
    if (strcmp(fs.sb.volume_label, "PurpleSystem") != 0) return 0;
    if (fs.sb.root_inode != PFS3_ROOT_INODE) return 0;

    rc = pfs3_unmount(&fs);
    if (rc != PFS3_OK) { destroy_mem_bdev(bdev); return 0; }

    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 3: Overflow-safe partition bounds checks */
static int test_overflow_safe_partition_validation(void) {
    block_device_t *bdev = create_mem_bdev(5000);

    pfs3_fs_t fs;
    /* Device sectors = 5000 */
    /* P_start >= D_sectors */
    if (pfs3_mount(&fs, bdev, 5000, 100) != PFS3_ERR_INVALID_PARTITION) return 0;
    if (pfs3_mount(&fs, bdev, 6000, 100) != PFS3_ERR_INVALID_PARTITION) return 0;

    /* P_sectors > D_sectors - P_start */
    if (pfs3_mount(&fs, bdev, 4000, 1001) != PFS3_ERR_INVALID_PARTITION) return 0;

    /* Overflow attempt: start + sectors wraps UINT64_MAX */
    if (pfs3_mount(&fs, bdev, 1000, UINT64_MAX - 500) != PFS3_ERR_INVALID_PARTITION) return 0;

    /* Less than 8 sectors */
    if (pfs3_mount(&fs, bdev, 0, 7) != PFS3_ERR_INVALID_PARTITION) return 0;

    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 4: Corrupt primary superblock falls back to backup superblock */
static int test_backup_superblock_fallback(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "BackupTest", NULL);

    /* Corrupt Block 1 (sectors 8..15) */
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)bdev->priv;
    memset(priv->storage + (1 * PFS3_BLOCK_SIZE), 0xFF, PFS3_BLOCK_SIZE);

    /* Mount must succeed using Backup Superblock at Block 2 */
    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, 10000);
    if (rc != PFS3_OK) { destroy_mem_bdev(bdev); return 0; }
    if (strcmp(fs.sb.volume_label, "BackupTest") != 0) { destroy_mem_bdev(bdev); return 0; }

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 5: Block 0 sentinel handling */
static int test_block_zero_sentinel(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "SentinelTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    /* Reading block 0 through pfs3_read_block must succeed and return all zeros */
    uint8_t buf[PFS3_BLOCK_SIZE];
    memset(buf, 0xAA, sizeof(buf));
    int rc = pfs3_read_block(&fs, 0, buf);
    if (rc != PFS3_OK) return 0;
    for (uint32_t i = 0; i < PFS3_BLOCK_SIZE; i++) {
        if (buf[i] != 0) return 0;
    }

    /* Writing block 0 through pfs3_write_block must be rejected */
    rc = pfs3_write_block(&fs, 0, buf);
    if (rc != PFS3_ERR_OUT_OF_BOUNDS) return 0;

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 6: File creation, write, and read (small direct block file) */
static int test_file_create_read_write(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "FileRWTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    /* Create file in root directory */
    uint64_t file_ino = 0;
    int rc = pfs3_create_entry(&fs, PFS3_ROOT_INODE, "hello.txt", PFS3_FT_REG, &file_ino);
    if (rc != PFS3_OK || file_ino < 3) return 0;

    /* Write data */
    const char *payload = "Hello Purple OS / Aegis64 from PurpleFS v3!";
    size_t plen = strlen(payload);
    rc = pfs3_write_file(&fs, file_ino, payload, plen, 0);
    if (rc != PFS3_OK) return 0;

    /* Read back */
    char read_buf[128];
    memset(read_buf, 0, sizeof(read_buf));
    uint64_t bytes_read = 0;
    rc = pfs3_read_file(&fs, file_ino, read_buf, sizeof(read_buf) - 1, 0, &bytes_read);
    if (rc != PFS3_OK || bytes_read != plen) return 0;
    if (strcmp(read_buf, payload) != 0) return 0;

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 7: Multi-block file spanning direct and single indirect pointers (>48 KiB) */
static int test_multiblock_indirect_file(void) {
    block_device_t *bdev = create_mem_bdev(20000);
    pfs3_format(bdev, 0, 20000, "IndirectTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 20000);

    uint64_t file_ino = 0;
    int rc = pfs3_create_entry(&fs, PFS3_ROOT_INODE, "kernel_img.bin", PFS3_FT_REG, &file_ino);
    if (rc != PFS3_OK) return 0;

    /* Write 64 KiB: exceeds 12 direct blocks (48 KiB) into indirect block */
    uint32_t payload_size = 64 * 1024;
    uint8_t *large_payload = (uint8_t *)malloc(payload_size);
    for (uint32_t i = 0; i < payload_size; i++) {
        large_payload[i] = (uint8_t)(i & 0xFF);
    }

    rc = pfs3_write_file(&fs, file_ino, large_payload, payload_size, 0);
    if (rc != PFS3_OK) { free(large_payload); return 0; }

    /* Verify inode has single indirect pointer allocated */
    pfs3_inode_t ino;
    rc = pfs3_read_inode(&fs, file_ino, &ino);
    if (rc != PFS3_OK || ino.size_bytes != payload_size || ino.indirect == 0) {
        free(large_payload);
        return 0;
    }

    /* Read back and compare */
    uint8_t *read_back = (uint8_t *)malloc(payload_size);
    uint64_t bytes_read = 0;
    rc = pfs3_read_file(&fs, file_ino, read_back, payload_size, 0, &bytes_read);
    if (rc != PFS3_OK || bytes_read != payload_size) {
        free(large_payload);
        free(read_back);
        return 0;
    }

    if (memcmp(large_payload, read_back, payload_size) != 0) {
        free(large_payload);
        free(read_back);
        return 0;
    }

    free(large_payload);
    free(read_back);
    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 8: File truncation */
static int test_file_truncate(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "TruncateTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    uint64_t file_ino = 0;
    pfs3_create_entry(&fs, PFS3_ROOT_INODE, "data.txt", PFS3_FT_REG, &file_ino);

    uint8_t buf[8192];
    memset(buf, 0xCC, sizeof(buf));
    pfs3_write_file(&fs, file_ino, buf, sizeof(buf), 0);

    /* Truncate from 8 KiB down to 100 bytes */
    int rc = pfs3_truncate_file(&fs, file_ino, 100);
    if (rc != PFS3_OK) return 0;

    pfs3_inode_t ino;
    pfs3_read_inode(&fs, file_ino, &ino);
    if (ino.size_bytes != 100) return 0;
    if (ino.direct[1] != 0) return 0; /* Second block freed */

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 9: Hierarchical directory creation and traversal */
static int test_hierarchical_directories(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "HierTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    /* Create /System */
    uint64_t sys_ino = 0;
    int rc = pfs3_mkdir(&fs, PFS3_ROOT_INODE, "System", &sys_ino);
    if (rc != PFS3_OK) return 0;

    /* Create /System/Drivers */
    uint64_t drv_ino = 0;
    rc = pfs3_mkdir(&fs, sys_ino, "Drivers", &drv_ino);
    if (rc != PFS3_OK) return 0;

    /* Create file /System/Drivers/audio.apdrv */
    uint64_t drv_file_ino = 0;
    rc = pfs3_create_entry(&fs, drv_ino, "audio.apdrv", PFS3_FT_REG, &drv_file_ino);
    if (rc != PFS3_OK) return 0;

    /* Lookup via path resolution: /System/Drivers/audio.apdrv */
    uint64_t resolved_ino = 0;
    rc = pfs3_lookup_path(&fs, "/System/Drivers/audio.apdrv", &resolved_ino);
    if (rc != PFS3_OK || resolved_ino != drv_file_ino) return 0;

    /* Lookup with relative path and . / .. */
    rc = pfs3_lookup_path(&fs, "System/../System/./Drivers/audio.apdrv", &resolved_ino);
    if (rc != PFS3_OK || resolved_ino != drv_file_ino) return 0;

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 10: Directory unlinking and safety checks */
static int test_directory_unlinking(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "UnlinkTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    uint64_t folder_ino = 0;
    pfs3_mkdir(&fs, PFS3_ROOT_INODE, "Folder", &folder_ino);

    uint64_t file_ino = 0;
    pfs3_create_entry(&fs, folder_ino, "item.txt", PFS3_FT_REG, &file_ino);

    /* Deleting non-empty folder must fail with PFS3_ERR_DIR_NOT_EMPTY */
    int rc = pfs3_rmdir(&fs, PFS3_ROOT_INODE, "Folder");
    if (rc != PFS3_ERR_DIR_NOT_EMPTY) return 0;

    /* Delete file item.txt */
    rc = pfs3_unlink_entry(&fs, folder_ino, "item.txt");
    if (rc != PFS3_OK) return 0;

    /* Now deleting empty folder succeeds */
    rc = pfs3_rmdir(&fs, PFS3_ROOT_INODE, "Folder");
    if (rc != PFS3_OK) return 0;

    /* Verify Folder no longer exists */
    uint64_t check = 0;
    rc = pfs3_lookup_path(&fs, "/Folder", &check);
    if (rc != PFS3_ERR_NOT_FOUND) return 0;

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 11: Remount persistence */
static int test_remount_persistence(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "PersistTest", NULL);

    pfs3_fs_t fs1;
    pfs3_mount(&fs1, bdev, 0, 10000);

    uint64_t boot_ino = 0;
    pfs3_mkdir(&fs1, PFS3_ROOT_INODE, "Boot", &boot_ino);

    uint64_t cfg_ino = 0;
    pfs3_create_entry(&fs1, boot_ino, "grub.cfg", PFS3_FT_REG, &cfg_ino);
    const char *cfg_text = "set default=0\ntimeout=5\nmenuentry 'Purple OS' { ... }";
    pfs3_write_file(&fs1, cfg_ino, cfg_text, strlen(cfg_text), 0);

    pfs3_unmount(&fs1);

    /* Second Mount: verify everything survived unmount and remount */
    pfs3_fs_t fs2;
    int rc = pfs3_mount(&fs2, bdev, 0, 10000);
    if (rc != PFS3_OK) return 0;

    uint64_t read_cfg_ino = 0;
    rc = pfs3_lookup_path(&fs2, "/Boot/grub.cfg", &read_cfg_ino);
    if (rc != PFS3_OK) return 0;

    char read_text[128];
    memset(read_text, 0, sizeof(read_text));
    uint64_t br = 0;
    rc = pfs3_read_file(&fs2, read_cfg_ino, read_text, sizeof(read_text) - 1, 0, &br);
    if (rc != PFS3_OK || strcmp(read_text, cfg_text) != 0) return 0;

    pfs3_unmount(&fs2);
    destroy_mem_bdev(bdev);
    return 1;
}

/* Test 12: Invalid filename rejection */
static int test_invalid_filename_rejection(void) {
    block_device_t *bdev = create_mem_bdev(10000);
    pfs3_format(bdev, 0, 10000, "NameTest", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 10000);

    uint64_t ino = 0;
    /* Empty name */
    if (pfs3_create_entry(&fs, PFS3_ROOT_INODE, "", PFS3_FT_REG, &ino) != PFS3_ERR_INVALID_NAME) return 0;

    /* Slashes in filename */
    if (pfs3_create_entry(&fs, PFS3_ROOT_INODE, "bad/name", PFS3_FT_REG, &ino) != PFS3_ERR_INVALID_NAME) return 0;

    /* Control characters */
    if (pfs3_create_entry(&fs, PFS3_ROOT_INODE, "bad\x05name", PFS3_FT_REG, &ino) != PFS3_ERR_INVALID_NAME) return 0;

    /* Overlong name (>48 chars) */
    char long_name[60];
    memset(long_name, 'a', 52);
    long_name[52] = '\0';
    if (pfs3_create_entry(&fs, PFS3_ROOT_INODE, long_name, PFS3_FT_REG, &ino) != PFS3_ERR_INVALID_NAME) return 0;

    pfs3_unmount(&fs);
    destroy_mem_bdev(bdev);
    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running PurpleFS v3 Host Test Suite                 \n");
    printf("=====================================================\n");

    RUN_TEST(test_static_assertions);
    RUN_TEST(test_format_and_mount);
    RUN_TEST(test_overflow_safe_partition_validation);
    RUN_TEST(test_backup_superblock_fallback);
    RUN_TEST(test_block_zero_sentinel);
    RUN_TEST(test_file_create_read_write);
    RUN_TEST(test_multiblock_indirect_file);
    RUN_TEST(test_file_truncate);
    RUN_TEST(test_hierarchical_directories);
    RUN_TEST(test_directory_unlinking);
    RUN_TEST(test_remount_persistence);
    RUN_TEST(test_invalid_filename_rejection);

    printf("=====================================================\n");
    printf(" PurpleFS v3 Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
