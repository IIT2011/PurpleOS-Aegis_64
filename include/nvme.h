#ifndef NVME_H
#define NVME_H

#include "types.h"
#include "pci.h"
#include "block_dev.h"

#define NVME_PCI_CLASS         0x01
#define NVME_PCI_SUBCLASS      0x08
#define NVME_PCI_PROG_IF       0x02

/* NVMe Controller Register Offsets */
#define NVME_REG_CAP           0x00
#define NVME_REG_VS            0x08
#define NVME_REG_INTMS         0x0C
#define NVME_REG_INTMC         0x10
#define NVME_REG_CC            0x14
#define NVME_REG_CSTS          0x1C
#define NVME_REG_NSSR          0x20
#define NVME_REG_AQA           0x24
#define NVME_REG_ASQ           0x28
#define NVME_REG_ACQ           0x30
#define NVME_REG_DBS           0x1000

/* Controller Capabilities Bits */
#define NVME_CAP_MQES(cap)     ((cap) & 0xFFFF)
#define NVME_CAP_DSTRD(cap)    (((cap) >> 32) & 0x0F)

/* Controller Configuration Bits */
#define NVME_CC_EN             (1U << 0)
#define NVME_CC_CSS_NVM        (0U << 4)
#define NVME_CC_MPS_4K         (0U << 7)
#define NVME_CC_IOSQES_64B     (6U << 16)
#define NVME_CC_IOCQES_16B     (4U << 20)

/* Controller Status Bits */
#define NVME_CSTS_RDY          (1U << 0)
#define NVME_CSTS_CFS          (1U << 1)

/* Admin Opcodes */
#define NVME_ADMIN_DELETE_SQ   0x00
#define NVME_ADMIN_CREATE_SQ   0x01
#define NVME_ADMIN_DELETE_CQ   0x04
#define NVME_ADMIN_CREATE_CQ   0x05
#define NVME_ADMIN_IDENTIFY    0x06
#define NVME_ADMIN_ABORT       0x08
#define NVME_ADMIN_SET_FEAT    0x09
#define NVME_ADMIN_GET_FEAT    0x0A

/* I/O Opcodes */
#define NVME_CMD_FLUSH         0x00
#define NVME_CMD_WRITE         0x01
#define NVME_CMD_READ          0x02

/* Identify CNS */
#define NVME_IDENTIFY_CNS_NS   0x00
#define NVME_IDENTIFY_CNS_CTRL 0x01

#define NVME_QUEUE_ENTRIES     64

/* NVMe Submission Queue Entry (64 bytes) */
typedef struct nvme_sq_entry {
    uint8_t  opcode;
    uint8_t  flags;
    uint16_t cid;
    uint32_t nsid;
    uint64_t reserved0;
    uint64_t mptr;
    uint64_t prp1;
    uint64_t prp2;
    uint32_t cdw10;
    uint32_t cdw11;
    uint32_t cdw12;
    uint32_t cdw13;
    uint32_t cdw14;
    uint32_t cdw15;
} __attribute__((packed)) nvme_sq_entry_t;

/* NVMe Completion Queue Entry (16 bytes) */
typedef struct nvme_cq_entry {
    uint32_t cdw0;
    uint32_t reserved;
    uint16_t sq_head;
    uint16_t sq_id;
    uint16_t cid;
    uint16_t status;
} __attribute__((packed)) nvme_cq_entry_t;

/* NVMe Queue Pair State */
typedef struct nvme_queue {
    uint16_t           qid;
    uint16_t           size;
    uint16_t           sq_tail;
    uint16_t           cq_head;
    uint8_t            cq_phase;
    nvme_sq_entry_t    *sq;
    uint64_t           sq_phys;
    nvme_cq_entry_t    *cq;
    uint64_t           cq_phys;
    volatile uint32_t  *sq_doorbell;
    volatile uint32_t  *cq_doorbell;
} nvme_queue_t;

/* NVMe Controller State */
typedef struct nvme_controller {
    const pci_device_t *pci_dev;
    volatile uint8_t   *bar0;
    uint64_t           bar0_phys;
    uint64_t           bar0_size;
    uint32_t           doorbell_stride;

    nvme_queue_t       admin_queue;
    nvme_queue_t       io_queue;

    char               model[41];
    char               serial[21];
    char               firmware[9];
    uint32_t           num_namespaces;

    // Namespace 1 state
    uint64_t           ns_size_blocks;
    uint32_t           ns_block_size;
    int                ns_active;

    // DMA Scratch Buffer for data transfers (64 KiB = 128 sectors)
    void               *dma_buf;
    uint64_t           dma_buf_phys;

    block_device_t     bdev;
} nvme_controller_t;

/* Public API */
int  nvme_init(void);
int  nvme_init_controller(nvme_controller_t *ctrl, const pci_device_t *pci_dev, volatile uint8_t *bar0);
void nvme_controller_free(nvme_controller_t *ctrl);
int  nvme_identify_controller(nvme_controller_t *ctrl);
int  nvme_identify_namespace(nvme_controller_t *ctrl, uint32_t nsid);
int  nvme_read_blocks(nvme_controller_t *ctrl, uint64_t lba, uint32_t count, void *buf);
int  nvme_write_blocks(nvme_controller_t *ctrl, uint64_t lba, uint32_t count, const void *buf);
int  nvme_flush(nvme_controller_t *ctrl);
nvme_controller_t *nvme_get_controller(void);

#ifdef __KERNEL_HOST_TEST__
void nvme_set_command_hook(void (*hook)(nvme_controller_t *ctrl, nvme_queue_t *q, nvme_sq_entry_t *sqe));
#endif

#endif /* NVME_H */
