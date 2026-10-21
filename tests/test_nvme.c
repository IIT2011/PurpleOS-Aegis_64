#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "nvme.h"
#include "dma.h"

// 16 KiB buffer representing NVMe BAR0 MMIO space
static uint8_t s_mock_bar0[16384];
static uint8_t s_mock_nvme_storage[256 * 512]; // 256 sectors mock drive

static void mock_nvme_hardware_hook(nvme_controller_t *ctrl, nvme_queue_t *q, nvme_sq_entry_t *sqe) {
    (void)ctrl;
    // Find next completion entry in CQ
    uint16_t c_idx = q->cq_head;
    nvme_cq_entry_t *cqe = &q->cq[c_idx];
    memset(cqe, 0, sizeof(*cqe));
    cqe->cid = sqe->cid;
    cqe->sq_id = q->qid;

    if (q->qid == 0) {
        // Admin Queue
        if (sqe->opcode == NVME_ADMIN_IDENTIFY) {
            uint8_t *buf = ctrl->dma_buf;
            memset(buf, 0, 4096);
            if (sqe->cdw10 == NVME_IDENTIFY_CNS_CTRL) {
                // Controller ID
                memcpy(buf + 4, "NVMETEST12345678    ", 20); // Serial
                memcpy(buf + 24, "QEMU NVME SSD                           ", 40); // Model
                memcpy(buf + 64, "1.0.0   ", 8); // Firmware
                *(uint32_t *)(buf + 516) = 1; // 1 namespace
            } else if (sqe->cdw10 == NVME_IDENTIFY_CNS_NS) {
                // Namespace ID (NSID 1)
                *(uint64_t *)(buf + 0) = 256; // 256 blocks
                buf[26] = 0; // flbas = 0
                // LBA format 0: ds = 9 (2^9 = 512 bytes)
                buf[128 + 2] = 9;
            }
        }
        // Success status (bit 16/0 is phase bit, matching q->cq_phase)
        cqe->status = (0 << 1) | (q->cq_phase & 1);
    } else {
        // I/O Queue
        if (sqe->opcode == NVME_CMD_READ) {
            uint64_t lba = (uint64_t)sqe->cdw10 | ((uint64_t)sqe->cdw11 << 32);
            uint32_t count = sqe->cdw12 + 1;
            assert(lba + count <= 256);
            memcpy(ctrl->dma_buf, s_mock_nvme_storage + lba * 512, count * 512);
        } else if (sqe->opcode == NVME_CMD_WRITE) {
            uint64_t lba = (uint64_t)sqe->cdw10 | ((uint64_t)sqe->cdw11 << 32);
            uint32_t count = sqe->cdw12 + 1;
            assert(lba + count <= 256);
            memcpy(s_mock_nvme_storage + lba * 512, ctrl->dma_buf, count * 512);
        }
        cqe->status = (0 << 1) | (q->cq_phase & 1);
    }
}

static void test_nvme_init_and_identify(void) {
    printf("[TEST  1] Running test_nvme_init_and_identify              ... ");
    memset(s_mock_bar0, 0, sizeof(s_mock_bar0));
    memset(s_mock_nvme_storage, 0, sizeof(s_mock_nvme_storage));
    block_dev_reset();

    // Setup CAP in BAR0: MQES = 63 (64 entries), DSTRD = 0
    *(uint64_t *)(s_mock_bar0 + NVME_REG_CAP) = 0x000000000000003FULL;

    // Hook hardware processing
    nvme_set_command_hook(mock_nvme_hardware_hook);

    // Make CSTS.RDY be 1 when enabled
    // CSTS is at offset 0x1C
    // In our test, we pre-set RDY so init loop succeeds
    *(uint32_t *)(s_mock_bar0 + NVME_REG_CSTS) = NVME_CSTS_RDY;

    nvme_controller_t ctrl;
    pci_device_t pdev;
    memset(&pdev, 0, sizeof(pdev));
    pdev.vendor_id = 0x1B36;
    pdev.device_id = 0x0010;

    int rc = nvme_init_controller(&ctrl, &pdev, s_mock_bar0);
    assert(rc == 0);
    assert(ctrl.ns_active == 1);
    assert(ctrl.ns_size_blocks == 256);
    assert(ctrl.ns_block_size == 512);
    assert(strstr(ctrl.model, "QEMU NVME") != NULL);
    assert(strstr(ctrl.serial, "NVMETEST") != NULL);

    // Verify block device registered
    block_device_t *bdev = block_dev_get_by_name("nvme0n1");
    assert(bdev != NULL);
    assert(bdev->total_blocks == 256);
    assert(bdev->block_size == 512);
    assert(bdev->type == BLOCK_DEV_TYPE_NVME);

    nvme_controller_free(&ctrl);
    printf("PASSED\n");
}

static void test_nvme_io_operations(void) {
    printf("[TEST  2] Running test_nvme_io_operations                  ... ");
    memset(s_mock_bar0, 0, sizeof(s_mock_bar0));
    memset(s_mock_nvme_storage, 0, sizeof(s_mock_nvme_storage));
    block_dev_reset();

    *(uint64_t *)(s_mock_bar0 + NVME_REG_CAP) = 0x000000000000003FULL;
    *(uint32_t *)(s_mock_bar0 + NVME_REG_CSTS) = NVME_CSTS_RDY;
    nvme_set_command_hook(mock_nvme_hardware_hook);

    nvme_controller_t ctrl;
    pci_device_t pdev;
    memset(&pdev, 0, sizeof(pdev));
    assert(nvme_init_controller(&ctrl, &pdev, s_mock_bar0) == 0);

    // Test write to LBA 5
    uint8_t wbuf[1024];
    for (int i = 0; i < 1024; i++) wbuf[i] = (uint8_t)(i ^ 0x99);

    int wrc = nvme_write_blocks(&ctrl, 5, 2, wbuf);
    assert(wrc == 0);

    // Verify written data in mock storage
    assert(memcmp(s_mock_nvme_storage + 5 * 512, wbuf, 1024) == 0);

    // Test read from LBA 5
    uint8_t rbuf[1024];
    memset(rbuf, 0, sizeof(rbuf));
    int rrc = nvme_read_blocks(&ctrl, 5, 2, rbuf);
    assert(rrc == 0);
    assert(memcmp(rbuf, wbuf, 1024) == 0);

    // Test flush
    assert(nvme_flush(&ctrl) == 0);

    nvme_controller_free(&ctrl);
    printf("PASSED\n");
}

static void test_nvme_bounds_and_errors(void) {
    printf("[TEST  3] Running test_nvme_bounds_and_errors              ... ");
    memset(s_mock_bar0, 0, sizeof(s_mock_bar0));
    memset(s_mock_nvme_storage, 0, sizeof(s_mock_nvme_storage));
    block_dev_reset();

    *(uint64_t *)(s_mock_bar0 + NVME_REG_CAP) = 0x000000000000003FULL;
    *(uint32_t *)(s_mock_bar0 + NVME_REG_CSTS) = NVME_CSTS_RDY;
    nvme_set_command_hook(mock_nvme_hardware_hook);

    nvme_controller_t ctrl;
    pci_device_t pdev;
    memset(&pdev, 0, sizeof(pdev));
    assert(nvme_init_controller(&ctrl, &pdev, s_mock_bar0) == 0);

    uint8_t buf[512];
    // Out of bounds: LBA >= total_blocks
    assert(nvme_read_blocks(&ctrl, 256, 1, buf) == -2);
    assert(nvme_write_blocks(&ctrl, 256, 1, buf) == -2);

    // Out of bounds: LBA + count > total_blocks
    assert(nvme_read_blocks(&ctrl, 250, 10, buf) == -2);
    assert(nvme_write_blocks(&ctrl, 250, 10, buf) == -2);

    // Invalid parameters
    assert(nvme_read_blocks(NULL, 0, 1, buf) == -1);
    assert(nvme_read_blocks(&ctrl, 0, 0, buf) == -1);
    assert(nvme_read_blocks(&ctrl, 0, 1, NULL) == -1);

    nvme_controller_free(&ctrl);
    printf("PASSED\n");
}

int main(void) {
    printf("=== Aegis64 NVMe PCIe Controller Host Test Suite ===\n");
    test_nvme_init_and_identify();
    test_nvme_io_operations();
    test_nvme_bounds_and_errors();
    printf("=== All NVMe Host Tests Passed Successfully! ===\n");
    return 0;
}
