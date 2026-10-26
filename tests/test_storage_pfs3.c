#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <stdint.h>

#include "purplefs_v3.h"
#include "kernel_storage.h"

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
/* Mock Block Device for Host Testing                                        */
/* ========================================================================= */

typedef struct {
    uint8_t *storage;
    uint64_t total_sectors;
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
    (void)dev;
    return BLOCK_DEV_OK;
}

static const block_device_ops_t mem_ops = {
    .read_blocks = mem_read_blocks,
    .write_blocks = mem_write_blocks,
    .flush = mem_flush,
};

static block_device_t *create_mock_disk(uint64_t total_sectors) {
    block_device_t *dev = (block_device_t *)calloc(1, sizeof(block_device_t));
    mem_bdev_priv_t *priv = (mem_bdev_priv_t *)calloc(1, sizeof(mem_bdev_priv_t));
    priv->storage = (uint8_t *)calloc(total_sectors, 512);
    priv->total_sectors = total_sectors;

    dev->block_size = 512;
    dev->total_blocks = total_sectors;
    dev->ops = &mem_ops;
    dev->priv = priv;
    strncpy(dev->name, "mock_sata0", sizeof(dev->name) - 1);
    return dev;
}

static void destroy_mock_disk(block_device_t *dev) {
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

/* Test 1: Format mock partition with PurpleFS v3 and mount via pfs3_mount */
static int test_pfs3_format_and_mount(void) {
    /* 10,000 sectors = ~5 MiB (1250 blocks) */
    block_device_t *bdev = create_mock_disk(10000);
    int rc = pfs3_format(bdev, 0, 10000, "PurpleSystem", NULL);
    assert(rc == PFS3_OK);

    pfs3_fs_t fs;
    rc = pfs3_mount(&fs, bdev, 0, 10000);
    assert(rc == PFS3_OK);
    assert(fs.sb.magic == PFS3_MAGIC);
    assert(fs.sb.version == PFS3_VERSION);
    assert(strcmp(fs.sb.volume_label, "PurpleSystem") == 0);
    assert(fs.total_fs_blocks == 1250);

    rc = pfs3_unmount(&fs);
    assert(rc == PFS3_OK);
    destroy_mock_disk(bdev);
    return 1;
}

/* Test 2: Directive 4 Logical Hierarchy Construction */
static int test_directive4_hierarchy_creation(void) {
    block_device_t *bdev = create_mock_disk(20000);
    pfs3_format(bdev, 0, 20000, "PurpleSystem", NULL);

    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, 20000);
    assert(rc == PFS3_OK);

    /* 1. Create top-level directories: /System, /Users, /Data, /Boot */
    uint64_t sys_ino = 0, users_ino = 0, data_ino = 0, boot_ino = 0;
    assert(pfs3_mkdir(&fs, PFS3_ROOT_INODE, "System", &sys_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, PFS3_ROOT_INODE, "Users", &users_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, PFS3_ROOT_INODE, "Data", &data_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, PFS3_ROOT_INODE, "Boot", &boot_ino) == PFS3_OK);

    /* 2. Subdirectories under /System */
    uint64_t apps_ino = 0, drvs_ino = 0, cfg_ino = 0, libs_ino = 0;
    assert(pfs3_mkdir(&fs, sys_ino, "Applications", &apps_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, sys_ino, "Drivers", &drvs_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, sys_ino, "Configuration", &cfg_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, sys_ino, "Libraries", &libs_ino) == PFS3_OK);

    /* 3. Subdirectories under /Users/aegis */
    uint64_t aegis_ino = 0;
    assert(pfs3_mkdir(&fs, users_ino, "aegis", &aegis_ino) == PFS3_OK);

    uint64_t desktop_ino = 0, docs_ino = 0, vids_ino = 0;
    assert(pfs3_mkdir(&fs, aegis_ino, "Desktop", &desktop_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, aegis_ino, "Documents", &docs_ino) == PFS3_OK);
    assert(pfs3_mkdir(&fs, aegis_ino, "Videos", &vids_ino) == PFS3_OK);

    /* 4. Verify path resolution */
    uint64_t found_ino = 0;
    assert(pfs3_lookup_path(&fs, "/System/Applications", &found_ino) == PFS3_OK);
    assert(found_ino == apps_ino);

    assert(pfs3_lookup_path(&fs, "/System/Drivers", &found_ino) == PFS3_OK);
    assert(found_ino == drvs_ino);

    assert(pfs3_lookup_path(&fs, "/Users/aegis/Desktop", &found_ino) == PFS3_OK);
    assert(found_ino == desktop_ino);

    pfs3_unmount(&fs);
    destroy_mock_disk(bdev);
    return 1;
}

/* Test 3: File Creation & Multi-Directory Traversal */
static int test_file_creation_in_subdirectories(void) {
    block_device_t *bdev = create_mock_disk(20000);
    pfs3_format(bdev, 0, 20000, "PurpleSystem", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 20000);

    /* Build /Users/aegis/Desktop */
    uint64_t u_ino = 0, a_ino = 0, d_ino = 0;
    pfs3_mkdir(&fs, PFS3_ROOT_INODE, "Users", &u_ino);
    pfs3_mkdir(&fs, u_ino, "aegis", &a_ino);
    pfs3_mkdir(&fs, a_ino, "Desktop", &d_ino);

    /* Create welcome.txt in /Users/aegis/Desktop */
    uint64_t f_ino = 0;
    assert(pfs3_create_entry(&fs, d_ino, "welcome.txt", PFS3_FT_REG, &f_ino) == PFS3_OK);

    const char *payload = "Welcome to Purple OS / Aegis64 Persistent Storage!";
    size_t plen = strlen(payload);
    assert(pfs3_write_file(&fs, f_ino, payload, plen, 0) == PFS3_OK);

    /* Lookup via full path */
    uint64_t resolved = 0;
    assert(pfs3_lookup_path(&fs, "/Users/aegis/Desktop/welcome.txt", &resolved) == PFS3_OK);
    assert(resolved == f_ino);

    /* Read and verify content */
    char buf[128];
    memset(buf, 0, sizeof(buf));
    uint64_t br = 0;
    assert(pfs3_read_file(&fs, resolved, buf, sizeof(buf) - 1, 0, &br) == PFS3_OK);
    assert(br == plen);
    assert(strcmp(buf, payload) == 0);

    pfs3_unmount(&fs);
    destroy_mock_disk(bdev);
    return 1;
}

/* Test 4: Readdir Enumeration Across Directories */
static int test_readdir_enumeration(void) {
    block_device_t *bdev = create_mock_disk(20000);
    pfs3_format(bdev, 0, 20000, "PurpleSystem", NULL);

    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 20000);

    /* Create entries in root */
    uint64_t dir1 = 0, file1 = 0, file2 = 0;
    pfs3_mkdir(&fs, PFS3_ROOT_INODE, "System", &dir1);
    pfs3_create_entry(&fs, PFS3_ROOT_INODE, "config.sys", PFS3_FT_REG, &file1);
    pfs3_create_entry(&fs, PFS3_ROOT_INODE, "motd", PFS3_FT_REG, &file2);

    /* Enumerate root directory */
    pfs3_dirent_t de;
    int entries_found = 0;
    uint32_t idx = 0;
    while (pfs3_readdir(&fs, PFS3_ROOT_INODE, idx++, &de) == PFS3_OK) {
        entries_found++;
    }
    /* Root should have ., .., System, config.sys, motd = 5 */
    assert(entries_found == 5);

    pfs3_unmount(&fs);
    destroy_mock_disk(bdev);
    return 1;
}

/* Test 5: Persistence Across Remount */
static int test_persistence_across_remount(void) {
    block_device_t *bdev = create_mock_disk(20000);
    pfs3_format(bdev, 0, 20000, "PurpleSystem", NULL);

    /* Mount 1 */
    pfs3_fs_t fs;
    pfs3_mount(&fs, bdev, 0, 20000);

    uint64_t s_ino = 0, d_ino = 0, f_ino = 0;
    pfs3_mkdir(&fs, PFS3_ROOT_INODE, "System", &s_ino);
    pfs3_mkdir(&fs, s_ino, "Drivers", &d_ino);
    pfs3_create_entry(&fs, d_ino, "Network.apdrv", PFS3_FT_REG, &f_ino);

    const char *drv_magic = "APDRV100_NET_V1";
    pfs3_write_file(&fs, f_ino, drv_magic, strlen(drv_magic), 0);
    pfs3_unmount(&fs);

    /* Mount 2 (Remount) */
    pfs3_fs_t fs2;
    assert(pfs3_mount(&fs2, bdev, 0, 20000) == PFS3_OK);

    uint64_t found_drv = 0;
    assert(pfs3_lookup_path(&fs2, "/System/Drivers/Network.apdrv", &found_drv) == PFS3_OK);

    char buf[64];
    memset(buf, 0, sizeof(buf));
    uint64_t br = 0;
    assert(pfs3_read_file(&fs2, found_drv, buf, sizeof(buf) - 1, 0, &br) == PFS3_OK);
    assert(strcmp(buf, drv_magic) == 0);

    pfs3_unmount(&fs2);
    destroy_mock_disk(bdev);
    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running PurpleFS v3 Hierarchical Storage Unit Tests\n");
    printf("=====================================================\n");

    RUN_TEST(test_pfs3_format_and_mount);
    RUN_TEST(test_directive4_hierarchy_creation);
    RUN_TEST(test_file_creation_in_subdirectories);
    RUN_TEST(test_readdir_enumeration);
    RUN_TEST(test_persistence_across_remount);

    printf("=====================================================\n");
    printf(" Storage & PFS3 Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
