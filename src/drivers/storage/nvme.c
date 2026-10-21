#include "nvme.h"
#include "dma.h"
#include "io.h"
#include "string.h"
#include "serial.h"

static nvme_controller_t g_nvme_ctrl;
static int g_nvme_detected = 0;

#ifdef __KERNEL_HOST_TEST__
static void (*s_nvme_cmd_hook)(nvme_controller_t *ctrl, nvme_queue_t *q, nvme_sq_entry_t *sqe) = NULL;
void nvme_set_command_hook(void (*hook)(nvme_controller_t *ctrl, nvme_queue_t *q, nvme_sq_entry_t *sqe)) {
    s_nvme_cmd_hook = hook;
}
#endif

static int nvme_bdev_read(block_device_t *bdev, uint64_t lba, uint32_t count, void *buf) {
    nvme_controller_t *ctrl = (nvme_controller_t *)bdev->priv;
    return nvme_read_blocks(ctrl, lba, count, buf) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int nvme_bdev_write(block_device_t *bdev, uint64_t lba, uint32_t count, const void *buf) {
    nvme_controller_t *ctrl = (nvme_controller_t *)bdev->priv;
    return nvme_write_blocks(ctrl, lba, count, buf) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int nvme_bdev_flush(block_device_t *bdev) {
    nvme_controller_t *ctrl = (nvme_controller_t *)bdev->priv;
    return nvme_flush(ctrl) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static const block_device_ops_t nvme_bdev_ops = {
    .read_blocks = nvme_bdev_read,
    .write_blocks = nvme_bdev_write,
    .flush = nvme_bdev_flush,
};

static inline uint32_t nvme_read32(nvme_controller_t *ctrl, uint32_t reg) {
    return *(volatile uint32_t *)(ctrl->bar0 + reg);
}

static inline void nvme_write32(nvme_controller_t *ctrl, uint32_t reg, uint32_t val) {
    *(volatile uint32_t *)(ctrl->bar0 + reg) = val;
}

static inline uint64_t nvme_read64(nvme_controller_t *ctrl, uint32_t reg) {
    uint32_t low = *(volatile uint32_t *)(ctrl->bar0 + reg);
    uint32_t high = *(volatile uint32_t *)(ctrl->bar0 + reg + 4);
    return ((uint64_t)high << 32) | low;
}

static inline void nvme_write64(nvme_controller_t *ctrl, uint32_t reg, uint64_t val) {
    *(volatile uint32_t *)(ctrl->bar0 + reg) = (uint32_t)val;
    *(volatile uint32_t *)(ctrl->bar0 + reg + 4) = (uint32_t)(val >> 32);
}

static int nvme_init_queue(nvme_controller_t *ctrl, nvme_queue_t *q, uint16_t qid, uint16_t size) {
    memset(q, 0, sizeof(*q));
    q->qid = qid;
    q->size = size;
    q->sq_tail = 0;
    q->cq_head = 0;
    q->cq_phase = 1; // Phase tag starts at 1 in NVMe specification

    q->sq = (nvme_sq_entry_t *)dma_alloc_coherent(size * sizeof(nvme_sq_entry_t), &q->sq_phys);
    if (!q->sq) {
        kwarn("NVMe: Failed to allocate SQ memory for QID %u", qid);
        return -1;
    }

    q->cq = (nvme_cq_entry_t *)dma_alloc_coherent(size * sizeof(nvme_cq_entry_t), &q->cq_phys);
    if (!q->cq) {
        kwarn("NVMe: Failed to allocate CQ memory for QID %u", qid);
        return -1;
    }

    uint32_t db_offset = NVME_REG_DBS + (2 * qid + 0) * (4 << ctrl->doorbell_stride);
    q->sq_doorbell = (volatile uint32_t *)(ctrl->bar0 + db_offset);

    db_offset = NVME_REG_DBS + (2 * qid + 1) * (4 << ctrl->doorbell_stride);
    q->cq_doorbell = (volatile uint32_t *)(ctrl->bar0 + db_offset);

    return 0;
}

static void nvme_free_queue(nvme_queue_t *q) {
    if (q->sq) {
        dma_free_coherent(q->sq, q->size * sizeof(nvme_sq_entry_t));
        q->sq = NULL;
    }
    if (q->cq) {
        dma_free_coherent(q->cq, q->size * sizeof(nvme_cq_entry_t));
        q->cq = NULL;
    }
}

static int nvme_submit_cmd(nvme_controller_t *ctrl, nvme_queue_t *q, const nvme_sq_entry_t *cmd) {
    if (!ctrl || !q || !cmd) return -1;

    uint16_t tail = q->sq_tail;
    memcpy(&q->sq[tail], cmd, sizeof(nvme_sq_entry_t));

    // Advance tail
    q->sq_tail = (uint16_t)((tail + 1) % q->size);

    // Memory barrier: ensure SQE writes to system memory complete before doorbell write
    mfence();

    // Ring submission queue doorbell
    *q->sq_doorbell = q->sq_tail;

#ifdef __KERNEL_HOST_TEST__
    if (s_nvme_cmd_hook) {
        s_nvme_cmd_hook(ctrl, q, &q->sq[tail]);
    }
#endif

    // Bounded spin poll on completion queue
    for (uint32_t spin = 0; spin < 1000000; spin++) {
        volatile nvme_cq_entry_t *cqe = &q->cq[q->cq_head];
        uint16_t status = cqe->status;
        uint8_t phase = (uint8_t)(status & 1);

        if (phase == q->cq_phase) {
            // Ensure CQE read is ordered after hardware phase write
            mfence();

            // Completion entry received!
            uint16_t status_code = (status >> 1) & 0x7FFF;

            q->cq_head = (uint16_t)((q->cq_head + 1) % q->size);
            if (q->cq_head == 0) {
                q->cq_phase ^= 1; // Invert phase bit when wrapping around 0
            }

            // Ring completion queue doorbell to inform hardware
            *q->cq_doorbell = q->cq_head;

            if (status_code != 0) {
                kwarn("NVMe: Command failed with status 0x%x (QID=%u, CID=%u)",
                      status_code, q->qid, cqe->cid);
                return -1;
            }
            return 0;
        }
    }

    kwarn("NVMe: Command timeout (QID=%u, CID=%u, head=%u, expected_phase=%u)",
          q->qid, cmd->cid, q->cq_head, q->cq_phase);
    return -2;
}

int nvme_identify_controller(nvme_controller_t *ctrl) {
    if (!ctrl || !ctrl->dma_buf) return -1;

    memset(ctrl->dma_buf, 0, 4096);

    nvme_sq_entry_t sqe;
    memset(&sqe, 0, sizeof(sqe));
    sqe.opcode = NVME_ADMIN_IDENTIFY;
    sqe.cid = 1;
    sqe.nsid = 0;
    sqe.prp1 = ctrl->dma_buf_phys;
    sqe.cdw10 = NVME_IDENTIFY_CNS_CTRL;

    int rc = nvme_submit_cmd(ctrl, &ctrl->admin_queue, &sqe);
    if (rc != 0) {
        return rc;
    }

    const uint8_t *data = (const uint8_t *)ctrl->dma_buf;

    // Serial (offset 4, 20 bytes)
    memcpy(ctrl->serial, data + 4, 20);
    ctrl->serial[20] = '\0';
    for (int i = 19; i >= 0 && (ctrl->serial[i] == ' ' || ctrl->serial[i] == '\0'); i--) {
        ctrl->serial[i] = '\0';
    }

    // Model (offset 24, 40 bytes)
    memcpy(ctrl->model, data + 24, 40);
    ctrl->model[40] = '\0';
    for (int i = 39; i >= 0 && (ctrl->model[i] == ' ' || ctrl->model[i] == '\0'); i--) {
        ctrl->model[i] = '\0';
    }

    // Firmware (offset 64, 8 bytes)
    memcpy(ctrl->firmware, data + 64, 8);
    ctrl->firmware[8] = '\0';
    for (int i = 7; i >= 0 && (ctrl->firmware[i] == ' ' || ctrl->firmware[i] == '\0'); i--) {
        ctrl->firmware[i] = '\0';
    }

    // Number of namespaces (offset 516, uint32_t)
    ctrl->num_namespaces = *(const uint32_t *)(data + 516);
    if (ctrl->num_namespaces == 0) {
        ctrl->num_namespaces = 1; // Fallback
    }

    kinfo("NVMe: Identified Controller: Model='%s', Serial='%s', FW='%s', Namespaces=%u",
          ctrl->model, ctrl->serial, ctrl->firmware, ctrl->num_namespaces);

    return 0;
}

int nvme_identify_namespace(nvme_controller_t *ctrl, uint32_t nsid) {
    if (!ctrl || !ctrl->dma_buf || nsid == 0) return -1;

    memset(ctrl->dma_buf, 0, 4096);

    nvme_sq_entry_t sqe;
    memset(&sqe, 0, sizeof(sqe));
    sqe.opcode = NVME_ADMIN_IDENTIFY;
    sqe.cid = 2;
    sqe.nsid = nsid;
    sqe.prp1 = ctrl->dma_buf_phys;
    sqe.cdw10 = NVME_IDENTIFY_CNS_NS;

    int rc = nvme_submit_cmd(ctrl, &ctrl->admin_queue, &sqe);
    if (rc != 0) {
        return rc;
    }

    const uint8_t *data = (const uint8_t *)ctrl->dma_buf;

    // Namespace Size (offset 0, uint64_t)
    ctrl->ns_size_blocks = *(const uint64_t *)(data + 0);

    // Formatted LBA Size (offset 26, uint8_t)
    uint8_t flbas = data[26] & 0x0F;

    // LBA Format (offset 128 + flbas * 4)
    // ds is at byte 2 of the 4-byte LBAF entry (bits 23:16)
    uint8_t ds = data[128 + flbas * 4 + 2];
    if (ds == 0) ds = 9; // Fallback to 512 bytes (2^9)
    ctrl->ns_block_size = (1U << ds);

    ctrl->ns_active = 1;

    // Register with block device subsystem
    memset(&ctrl->bdev, 0, sizeof(ctrl->bdev));
    strcpy(ctrl->bdev.name, "nvme0n1");
    strncpy(ctrl->bdev.model, ctrl->model, sizeof(ctrl->bdev.model) - 1);
    strncpy(ctrl->bdev.serial, ctrl->serial, sizeof(ctrl->bdev.serial) - 1);
    ctrl->bdev.type = BLOCK_DEV_TYPE_NVME;
    ctrl->bdev.block_size = ctrl->ns_block_size;
    ctrl->bdev.total_blocks = ctrl->ns_size_blocks;
    ctrl->bdev.ops = &nvme_bdev_ops;
    ctrl->bdev.priv = ctrl;

    block_dev_register(&ctrl->bdev);

    kinfo("NVMe: Namespace %u online: %u blocks @ %u bytes/block (%u MiB)",
          nsid, (uint32_t)ctrl->ns_size_blocks, ctrl->ns_block_size,
          (uint32_t)((ctrl->ns_size_blocks * ctrl->ns_block_size) / (1024 * 1024)));

    return 0;
}

static int nvme_create_io_queues(nvme_controller_t *ctrl) {
    if (nvme_init_queue(ctrl, &ctrl->io_queue, 1, NVME_QUEUE_ENTRIES) != 0) {
        return -1;
    }

    // 1. Create I/O Completion Queue
    nvme_sq_entry_t sqe;
    memset(&sqe, 0, sizeof(sqe));
    sqe.opcode = NVME_ADMIN_CREATE_CQ;
    sqe.cid = 3;
    sqe.prp1 = ctrl->io_queue.cq_phys;
    sqe.cdw10 = ((uint32_t)(NVME_QUEUE_ENTRIES - 1) << 16) | 1; // Size, QID=1
    sqe.cdw11 = 0x0001; // Physically contiguous, interrupts disabled

    int rc = nvme_submit_cmd(ctrl, &ctrl->admin_queue, &sqe);
    if (rc != 0) {
        kwarn("NVMe: Failed to create I/O Completion Queue");
        return rc;
    }

    // 2. Create I/O Submission Queue
    memset(&sqe, 0, sizeof(sqe));
    sqe.opcode = NVME_ADMIN_CREATE_SQ;
    sqe.cid = 4;
    sqe.prp1 = ctrl->io_queue.sq_phys;
    sqe.cdw10 = ((uint32_t)(NVME_QUEUE_ENTRIES - 1) << 16) | 1; // Size, QID=1
    sqe.cdw11 = (1U << 16) | 0x0001; // CQID=1, Physically contiguous

    rc = nvme_submit_cmd(ctrl, &ctrl->admin_queue, &sqe);
    if (rc != 0) {
        kwarn("NVMe: Failed to create I/O Submission Queue");
        return rc;
    }

    kinfo("NVMe: I/O Queue pair established (QID 1, entries=%u)", NVME_QUEUE_ENTRIES);
    return 0;
}

int nvme_read_blocks(nvme_controller_t *ctrl, uint64_t lba, uint32_t count, void *buf) {
    if (!ctrl || !ctrl->ns_active || count == 0 || !buf) return -1;
    if (lba >= ctrl->ns_size_blocks || lba + count > ctrl->ns_size_blocks || lba + count < lba) {
        return -2;
    }

    uint8_t *dest = (uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t cur_lba = lba;
    uint32_t max_blocks = 65536 / ctrl->ns_block_size; // Max blocks in 64 KiB buffer

    while (remaining > 0) {
        uint32_t chunk = (remaining > max_blocks) ? max_blocks : remaining;
        uint32_t transfer_bytes = chunk * ctrl->ns_block_size;

        nvme_sq_entry_t sqe;
        memset(&sqe, 0, sizeof(sqe));
        sqe.opcode = NVME_CMD_READ;
        sqe.nsid = 1;
        sqe.prp1 = ctrl->dma_buf_phys;

        // Spec-compliant PRP construction:
        // <= 4096 bytes (1 page): PRP1 = buffer, PRP2 = 0
        // <= 8192 bytes (2 pages): PRP1 = page 0, PRP2 = page 1
        // > 8192 bytes: PRP1 = page 0, PRP2 = PRP List page pointer
        if (transfer_bytes <= 4096) {
            sqe.prp2 = 0;
        } else if (transfer_bytes <= 8192) {
            sqe.prp2 = ctrl->dma_buf_phys + 4096;
        } else {
            sqe.prp2 = ctrl->prp_list_phys;
        }

        sqe.cdw10 = (uint32_t)(cur_lba & 0xFFFFFFFFU);
        sqe.cdw11 = (uint32_t)(cur_lba >> 32);
        sqe.cdw12 = (chunk - 1); // 0-based block count

        int rc = nvme_submit_cmd(ctrl, &ctrl->io_queue, &sqe);
        if (rc != 0) {
            return rc;
        }

        // Memory barrier: ensure DMA writes to buffer are visible before copying out
        mfence();

        memcpy(dest, ctrl->dma_buf, transfer_bytes);

        dest += transfer_bytes;
        cur_lba += chunk;
        remaining -= chunk;
    }

    return 0;
}

int nvme_write_blocks(nvme_controller_t *ctrl, uint64_t lba, uint32_t count, const void *buf) {
    if (!ctrl || !ctrl->ns_active || count == 0 || !buf) return -1;
    if (lba >= ctrl->ns_size_blocks || lba + count > ctrl->ns_size_blocks || lba + count < lba) {
        return -2;
    }

    const uint8_t *src = (const uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t cur_lba = lba;
    uint32_t max_blocks = 65536 / ctrl->ns_block_size;

    while (remaining > 0) {
        uint32_t chunk = (remaining > max_blocks) ? max_blocks : remaining;
        uint32_t transfer_bytes = chunk * ctrl->ns_block_size;

        memcpy(ctrl->dma_buf, src, transfer_bytes);

        // Memory barrier: ensure data in dma_buf is committed before hardware fetches it
        mfence();

        nvme_sq_entry_t sqe;
        memset(&sqe, 0, sizeof(sqe));
        sqe.opcode = NVME_CMD_WRITE;
        sqe.nsid = 1;
        sqe.prp1 = ctrl->dma_buf_phys;

        if (transfer_bytes <= 4096) {
            sqe.prp2 = 0;
        } else if (transfer_bytes <= 8192) {
            sqe.prp2 = ctrl->dma_buf_phys + 4096;
        } else {
            sqe.prp2 = ctrl->prp_list_phys;
        }

        sqe.cdw10 = (uint32_t)(cur_lba & 0xFFFFFFFFU);
        sqe.cdw11 = (uint32_t)(cur_lba >> 32);
        sqe.cdw12 = (chunk - 1);

        int rc = nvme_submit_cmd(ctrl, &ctrl->io_queue, &sqe);
        if (rc != 0) {
            return rc;
        }

        src += transfer_bytes;
        cur_lba += chunk;
        remaining -= chunk;
    }

    return nvme_flush(ctrl);
}

int nvme_flush(nvme_controller_t *ctrl) {
    if (!ctrl || !ctrl->ns_active) return -1;

    nvme_sq_entry_t sqe;
    memset(&sqe, 0, sizeof(sqe));
    sqe.opcode = NVME_CMD_FLUSH;
    sqe.nsid = 1;

    return nvme_submit_cmd(ctrl, &ctrl->io_queue, &sqe);
}

void nvme_controller_free(nvme_controller_t *ctrl) {
    if (!ctrl) return;
    nvme_free_queue(&ctrl->admin_queue);
    nvme_free_queue(&ctrl->io_queue);
    if (ctrl->dma_buf) {
        dma_free_coherent(ctrl->dma_buf, 65536);
        ctrl->dma_buf = NULL;
    }
    if (ctrl->prp_list) {
        dma_free_coherent(ctrl->prp_list, 4096);
        ctrl->prp_list = NULL;
    }
}

int nvme_init_controller(nvme_controller_t *ctrl, const pci_device_t *pci_dev, volatile uint8_t *bar0) {
    if (!ctrl || !bar0) return -1;

    memset(ctrl, 0, sizeof(*ctrl));
    ctrl->pci_dev = pci_dev;
    ctrl->bar0 = bar0;

    // 1. Read Capabilities
    uint64_t cap = nvme_read64(ctrl, NVME_REG_CAP);
    ctrl->doorbell_stride = NVME_CAP_DSTRD(cap);
    kinfo("NVMe: Capabilities = 0x%lx, DSTRD = %u", cap, ctrl->doorbell_stride);

    // 2. Disable controller if active
    uint32_t cc = nvme_read32(ctrl, NVME_REG_CC);
    if (cc & NVME_CC_EN) {
        nvme_write32(ctrl, NVME_REG_CC, cc & ~NVME_CC_EN);
        int dis_ok = 0;
        for (uint32_t t = 0; t < 100000; t++) {
            if ((nvme_read32(ctrl, NVME_REG_CSTS) & NVME_CSTS_RDY) == 0) {
                dis_ok = 1;
                break;
            }
        }
        if (!dis_ok) {
            kwarn("NVMe: Timeout waiting for controller to disable");
        }
    }

    // 3. Allocate 64 KiB DMA buffer for data transfers
    ctrl->dma_buf = dma_alloc_coherent(65536, &ctrl->dma_buf_phys);
    if (!ctrl->dma_buf) {
        kwarn("NVMe: Failed to allocate DMA transfer buffer");
        return -1;
    }

    // 4. Allocate 4 KiB PRP List Page (holds up to 512 page pointers)
    ctrl->prp_list = (uint64_t *)dma_alloc_coherent(4096, &ctrl->prp_list_phys);
    if (!ctrl->prp_list) {
        kwarn("NVMe: Failed to allocate PRP list page");
        nvme_controller_free(ctrl);
        return -1;
    }
    // Populate PRP list with physical addresses of pages 1 through 15 of dma_buf
    for (uint32_t i = 1; i < 16; i++) {
        ctrl->prp_list[i - 1] = ctrl->dma_buf_phys + (uint64_t)i * 4096;
    }

    // 5. Initialize Admin Queue (QID 0, size 64)
    if (nvme_init_queue(ctrl, &ctrl->admin_queue, 0, NVME_QUEUE_ENTRIES) != 0) {
        nvme_controller_free(ctrl);
        return -1;
    }

    // 5. Configure AQA, ASQ, ACQ
    uint32_t aqa = ((uint32_t)(NVME_QUEUE_ENTRIES - 1) << 16) | (NVME_QUEUE_ENTRIES - 1);
    nvme_write32(ctrl, NVME_REG_AQA, aqa);
    nvme_write64(ctrl, NVME_REG_ASQ, ctrl->admin_queue.sq_phys);
    nvme_write64(ctrl, NVME_REG_ACQ, ctrl->admin_queue.cq_phys);

    // 6. Enable Controller
    cc = NVME_CC_EN | NVME_CC_CSS_NVM | NVME_CC_MPS_4K | NVME_CC_IOSQES_64B | NVME_CC_IOCQES_16B;
    nvme_write32(ctrl, NVME_REG_CC, cc);

    // 7. Wait for CSTS.RDY == 1
    for (uint32_t t = 0; t < 100000; t++) {
        if (nvme_read32(ctrl, NVME_REG_CSTS) & NVME_CSTS_RDY) {
            break;
        }
    }
    if ((nvme_read32(ctrl, NVME_REG_CSTS) & NVME_CSTS_RDY) == 0) {
        kwarn("NVMe: Controller failed to become ready (CSTS=0x%x)", nvme_read32(ctrl, NVME_REG_CSTS));
        nvme_controller_free(ctrl);
        return -2;
    }

    kinfo("NVMe: Controller ready. Identifying...");

    // 8. Identify Controller
    if (nvme_identify_controller(ctrl) != 0) {
        nvme_controller_free(ctrl);
        return -3;
    }

    // 9. Create I/O Queues
    if (nvme_create_io_queues(ctrl) != 0) {
        nvme_controller_free(ctrl);
        return -4;
    }

    // 10. Identify Namespace 1
    if (nvme_identify_namespace(ctrl, 1) != 0) {
        nvme_controller_free(ctrl);
        return -5;
    }

    g_nvme_detected = 1;
    return 0;
}

int nvme_init(void) {
    const pci_device_t *pdev = pci_find_by_class(NVME_PCI_CLASS, NVME_PCI_SUBCLASS, NVME_PCI_PROG_IF);
    if (!pdev) {
        kinfo("NVMe: No NVMe controller detected on PCI bus");
        return -1;
    }

    kinfo("NVMe: Found controller at PCI %02x:%02x.%d [Vendor %04x, Device %04x]",
          pdev->bus, pdev->dev, pdev->func, pdev->vendor_id, pdev->device_id);

    pci_enable_bus_mastering(pdev->bus, pdev->dev, pdev->func);
    pci_enable_io(pdev->bus, pdev->dev, pdev->func);

    uint64_t bar0_phys = pci_get_bar_address(pdev, 0);
    uint64_t bar0_size = pci_get_bar_size(pdev, 0);
    if (bar0_phys == 0) {
        kwarn("NVMe: Invalid BAR 0 (address is 0)");
        return -2;
    }

    kinfo("NVMe: BAR 0 physical address = 0x%lx (size %lu bytes)", bar0_phys, bar0_size);
    volatile uint8_t *bar0 = (volatile uint8_t *)mmio_map(bar0_phys, (size_t)bar0_size);

    return nvme_init_controller(&g_nvme_ctrl, pdev, bar0);
}

nvme_controller_t *nvme_get_controller(void) {
    return g_nvme_detected ? &g_nvme_ctrl : NULL;
}
