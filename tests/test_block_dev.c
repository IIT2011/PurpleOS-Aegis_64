#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "block_dev.h"
#include "dma.h"

/* Mock RAM block device for testing */
#define MOCK_BLOCK_COUNT 128
#define MOCK_BLOCK_SIZE  512

typedef struct {
    uint8_t storage[MOCK_BLOCK_COUNT * MOCK_BLOCK_SIZE];
    int flush_called;
} mock_ramdisk_t;

static int mock_read(block_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    mock_ramdisk_t *rd = (mock_ramdisk_t *)dev->priv;
    memcpy(buf, rd->storage + lba * MOCK_BLOCK_SIZE, count * MOCK_BLOCK_SIZE);
    return BLOCK_DEV_OK;
}

static int mock_write(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    mock_ramdisk_t *rd = (mock_ramdisk_t *)dev->priv;
    memcpy(rd->storage + lba * MOCK_BLOCK_SIZE, buf, count * MOCK_BLOCK_SIZE);
    return BLOCK_DEV_OK;
}

static int mock_flush(block_device_t *dev) {
    mock_ramdisk_t *rd = (mock_ramdisk_t *)dev->priv;
    rd->flush_called++;
    return BLOCK_DEV_OK;
}

static const block_device_ops_t mock_ops = {
    .read_blocks = mock_read,
    .write_blocks = mock_write,
    .flush = mock_flush,
};

static void test_dma_coherent(void) {
    printf("[TEST  1] Running test_dma_coherent                         ... ");
    uint64_t phys = 0;
    void *vaddr = dma_alloc_coherent(8192, &phys);
    assert(vaddr != NULL);
    assert(((uintptr_t)vaddr & 0xFFF) == 0); // 4096-byte aligned
    assert((phys & 0xFFF) == 0);
    // Verify zeroed
    uint8_t *b = (uint8_t *)vaddr;
    for (int i = 0; i < 8192; i++) {
        assert(b[i] == 0);
    }

    dma_free_coherent(vaddr, 8192);

    // Invalid size allocation
    assert(dma_alloc_coherent(0, &phys) == NULL);

    printf("PASSED\n");
}

static void test_block_dev_init_and_registration(void) {
    printf("[TEST  2] Running test_block_dev_init_and_registration      ... ");
    block_dev_reset();
    assert(block_dev_count() == 0);

    mock_ramdisk_t rd1;
    memset(&rd1, 0, sizeof(rd1));

    block_device_t dev1;
    memset(&dev1, 0, sizeof(dev1));
    strcpy(dev1.name, "ram0");
    strcpy(dev1.model, "Purple RamDisk");
    dev1.type = BLOCK_DEV_TYPE_RAMDISK;
    dev1.block_size = MOCK_BLOCK_SIZE;
    dev1.total_blocks = MOCK_BLOCK_COUNT;
    dev1.ops = &mock_ops;
    dev1.priv = &rd1;

    int rc = block_dev_register(&dev1);
    assert(rc == BLOCK_DEV_OK);
    assert(dev1.id == 1);
    assert(dev1.flags & BLOCK_DEV_FLAG_ONLINE);
    assert(block_dev_count() == 1);

    // Query tests
    assert(block_dev_get_by_name("ram0") == &dev1);
    assert(block_dev_get_by_name("nonexistent") == NULL);
    assert(block_dev_get_by_index(0) == &dev1);
    assert(block_dev_get_by_index(1) == NULL);

    // Duplicate name registration should fail
    block_device_t dev_dup;
    memcpy(&dev_dup, &dev1, sizeof(dev_dup));
    assert(block_dev_register(&dev_dup) == BLOCK_DEV_ERR_INVALID);

    // Invalid parameters registration should fail
    block_device_t dev_inv;
    memset(&dev_inv, 0, sizeof(dev_inv));
    assert(block_dev_register(&dev_inv) == BLOCK_DEV_ERR_INVALID);

    printf("PASSED\n");
}

static void test_block_dev_io_operations(void) {
    printf("[TEST  3] Running test_block_dev_io_operations              ... ");
    block_dev_reset();

    mock_ramdisk_t rd;
    memset(&rd, 0, sizeof(rd));

    block_device_t dev;
    memset(&dev, 0, sizeof(dev));
    strcpy(dev.name, "sata0");
    dev.type = BLOCK_DEV_TYPE_AHCI;
    dev.block_size = MOCK_BLOCK_SIZE;
    dev.total_blocks = MOCK_BLOCK_COUNT;
    dev.ops = &mock_ops;
    dev.priv = &rd;

    assert(block_dev_register(&dev) == BLOCK_DEV_OK);

    // Write data to LBA 5
    uint8_t wbuf[MOCK_BLOCK_SIZE * 2];
    for (size_t i = 0; i < sizeof(wbuf); i++) wbuf[i] = (uint8_t)(i ^ 0xA5);

    int wrc = block_dev_write(&dev, 5, 2, wbuf);
    assert(wrc == BLOCK_DEV_OK);

    // Read back data from LBA 5
    uint8_t rbuf[MOCK_BLOCK_SIZE * 2];
    memset(rbuf, 0, sizeof(rbuf));
    int rrc = block_dev_read(&dev, 5, 2, rbuf);
    assert(rrc == BLOCK_DEV_OK);
    assert(memcmp(wbuf, rbuf, sizeof(wbuf)) == 0);

    // Flush
    assert(rd.flush_called == 0);
    assert(block_dev_flush(&dev) == BLOCK_DEV_OK);
    assert(rd.flush_called == 1);

    // Test zero-count I/O
    assert(block_dev_read(&dev, 0, 0, rbuf) == BLOCK_DEV_OK);
    assert(block_dev_write(&dev, 0, 0, wbuf) == BLOCK_DEV_OK);

    printf("PASSED\n");
}

static void test_block_dev_bounds_and_readonly(void) {
    printf("[TEST  4] Running test_block_dev_bounds_and_readonly        ... ");
    block_dev_reset();

    mock_ramdisk_t rd;
    memset(&rd, 0, sizeof(rd));

    block_device_t dev;
    memset(&dev, 0, sizeof(dev));
    strcpy(dev.name, "nvme0n1");
    dev.type = BLOCK_DEV_TYPE_NVME;
    dev.block_size = MOCK_BLOCK_SIZE;
    dev.total_blocks = MOCK_BLOCK_COUNT;
    dev.ops = &mock_ops;
    dev.priv = &rd;

    assert(block_dev_register(&dev) == BLOCK_DEV_OK);

    uint8_t buf[MOCK_BLOCK_SIZE];

    // Out of bounds: LBA past capacity
    assert(block_dev_read(&dev, MOCK_BLOCK_COUNT, 1, buf) == BLOCK_DEV_ERR_BOUNDS);
    assert(block_dev_write(&dev, MOCK_BLOCK_COUNT, 1, buf) == BLOCK_DEV_ERR_BOUNDS);

    // Out of bounds: LBA + count past capacity
    assert(block_dev_read(&dev, MOCK_BLOCK_COUNT - 1, 2, buf) == BLOCK_DEV_ERR_BOUNDS);
    assert(block_dev_write(&dev, MOCK_BLOCK_COUNT - 1, 2, buf) == BLOCK_DEV_ERR_BOUNDS);

    // 64-bit arithmetic overflow resistance
    assert(block_dev_read(&dev, 0xFFFFFFFFFFFFFFFFULL, 1, buf) == BLOCK_DEV_ERR_BOUNDS);
    assert(block_dev_read(&dev, 10, 0xFFFFFFFFU, buf) == BLOCK_DEV_ERR_BOUNDS);
    assert(block_dev_write(&dev, 0xFFFFFFFFFFFFFFFFULL, 1, buf) == BLOCK_DEV_ERR_BOUNDS);
    assert(block_dev_write(&dev, 10, 0xFFFFFFFFU, buf) == BLOCK_DEV_ERR_BOUNDS);

    // Read-only enforcement
    dev.flags |= BLOCK_DEV_FLAG_READONLY;
    assert(block_dev_write(&dev, 0, 1, buf) == BLOCK_DEV_ERR_ROFS);
    // Read should still succeed
    assert(block_dev_read(&dev, 0, 1, buf) == BLOCK_DEV_OK);

    printf("PASSED\n");
}

static void test_block_dev_capacity_and_unregister(void) {
    printf("[TEST  5] Running test_block_dev_capacity_and_unregister    ... ");
    block_dev_reset();

    mock_ramdisk_t rd;
    memset(&rd, 0, sizeof(rd));

    block_device_t devs[MAX_BLOCK_DEVICES + 1];
    for (int i = 0; i < MAX_BLOCK_DEVICES; i++) {
        memset(&devs[i], 0, sizeof(devs[i]));
        snprintf(devs[i].name, sizeof(devs[i].name), "disk%d", i);
        devs[i].type = BLOCK_DEV_TYPE_AHCI;
        devs[i].block_size = 512;
        devs[i].total_blocks = 100;
        devs[i].ops = &mock_ops;
        devs[i].priv = &rd;
        assert(block_dev_register(&devs[i]) == BLOCK_DEV_OK);
    }

    assert(block_dev_count() == MAX_BLOCK_DEVICES);

    // Overflow registration
    memset(&devs[MAX_BLOCK_DEVICES], 0, sizeof(devs[0]));
    strcpy(devs[MAX_BLOCK_DEVICES].name, "overflow");
    devs[MAX_BLOCK_DEVICES].type = BLOCK_DEV_TYPE_NVME;
    devs[MAX_BLOCK_DEVICES].block_size = 512;
    devs[MAX_BLOCK_DEVICES].total_blocks = 100;
    devs[MAX_BLOCK_DEVICES].ops = &mock_ops;
    assert(block_dev_register(&devs[MAX_BLOCK_DEVICES]) == BLOCK_DEV_ERR_NOMEM);

    // Unregister middle element
    assert(block_dev_unregister(&devs[5]) == BLOCK_DEV_OK);
    assert(block_dev_count() == MAX_BLOCK_DEVICES - 1);
    assert(block_dev_get_by_name("disk5") == NULL);
    assert(!(devs[5].flags & BLOCK_DEV_FLAG_ONLINE));

    // Now slot is freed, can register new one
    assert(block_dev_register(&devs[MAX_BLOCK_DEVICES]) == BLOCK_DEV_OK);
    assert(block_dev_count() == MAX_BLOCK_DEVICES);

    printf("PASSED\n");
}

int main(void) {
    printf("=== Aegis64 Unified Block Device & DMA Test Suite ===\n");
    test_dma_coherent();
    test_block_dev_init_and_registration();
    test_block_dev_io_operations();
    test_block_dev_bounds_and_readonly();
    test_block_dev_capacity_and_unregister();
    printf("=== All Block Device Host Tests Passed Successfully! ===\n");
    return 0;
}
