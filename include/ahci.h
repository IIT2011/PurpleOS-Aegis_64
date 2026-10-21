#ifndef AHCI_H
#define AHCI_H

#include "types.h"
#include "pci.h"
#include "block_dev.h"

#define AHCI_PCI_CLASS     0x01
#define AHCI_PCI_SUBCLASS  0x06
#define AHCI_PCI_PROG_IF   0x01

#define AHCI_SIG_ATA       0x00000101
#define AHCI_SIG_ATAPI     0xEB140101
#define AHCI_SIG_SEMB      0xC33C0101
#define AHCI_SIG_PM        0x96690101

#define HBA_PORT_DET_PRESENT 0x03
#define HBA_PORT_IPM_ACTIVE  0x01

#define HBA_CMD_ST         (1U << 0)
#define HBA_CMD_FRE        (1U << 4)
#define HBA_CMD_FR         (1U << 14)
#define HBA_CMD_CR         (1U << 15)

#define HBA_TFD_ERR        (1U << 0)
#define HBA_TFD_DRQ        (1U << 3)
#define HBA_TFD_BSY        (1U << 7)

#define HBA_GHC_HR         (1U << 0)
#define HBA_GHC_IE         (1U << 1)
#define HBA_GHC_AE         (1U << 31)

#define FIS_TYPE_REG_H2D   0x27

#define ATA_CMD_READ_DMA_EXT   0x25
#define ATA_CMD_WRITE_DMA_EXT  0x35
#define ATA_CMD_CACHE_FLUSH_EXT 0xEA
#define ATA_CMD_IDENTIFY_DEV   0xEC

#define AHCI_MAX_PRDT      16
#define AHCI_MAX_PORTS     32

/* Port MMIO Registers (128 bytes per port) */
typedef volatile struct ahci_hba_port {
    uint32_t clb;
    uint32_t clbu;
    uint32_t fb;
    uint32_t fbu;
    uint32_t is;
    uint32_t ie;
    uint32_t cmd;
    uint32_t reserved0;
    uint32_t tfd;
    uint32_t sig;
    uint32_t ssts;
    uint32_t sctl;
    uint32_t serr;
    uint32_t sact;
    uint32_t ci;
    uint32_t sntf;
    uint32_t fbs;
    uint32_t devslp;
    uint32_t reserved1[10];
    uint32_t vendor[4];
} __attribute__((packed)) ahci_hba_port_t;

/* Generic Host Control MMIO Registers (ABAR) */
typedef volatile struct ahci_hba_mem {
    uint32_t cap;
    uint32_t ghc;
    uint32_t is;
    uint32_t pi;
    uint32_t vs;
    uint32_t ccc_ctl;
    uint32_t ccc_pts;
    uint32_t em_loc;
    uint32_t em_ctl;
    uint32_t cap2;
    uint32_t bohc;
    uint8_t  reserved[0xA0 - 0x2C];
    uint8_t  vendor[0x100 - 0xA0];
    ahci_hba_port_t ports[AHCI_MAX_PORTS];
} __attribute__((packed)) ahci_hba_mem_t;

/* Command Header (32 bytes) */
typedef struct ahci_cmd_header {
    uint16_t flags;
    uint16_t prdtl;
    uint32_t prdbc;
    uint32_t ctba;
    uint32_t ctbau;
    uint32_t reserved[4];
} __attribute__((packed)) ahci_cmd_header_t;

/* PRDT Entry (16 bytes) */
typedef struct ahci_prdt_entry {
    uint32_t dba;
    uint32_t dbau;
    uint32_t reserved;
    uint32_t dbc;
} __attribute__((packed)) ahci_prdt_entry_t;

/* Command Table (128 bytes + PRDT) */
typedef struct ahci_cmd_table {
    uint8_t  cfis[64];
    uint8_t  acmd[16];
    uint8_t  reserved[48];
    ahci_prdt_entry_t prdt[AHCI_MAX_PRDT];
} __attribute__((packed, aligned(128))) ahci_cmd_table_t;

/* Host to Device Register FIS (20 bytes) */
typedef struct fis_reg_h2d {
    uint8_t  fis_type;      /* 0x27 */
    uint8_t  pmport_c;      /* Bit 7: 1=Command, bits 3:0: pmport */
    uint8_t  command;
    uint8_t  featurel;

    uint8_t  lba0;
    uint8_t  lba1;
    uint8_t  lba2;
    uint8_t  device;

    uint8_t  lba3;
    uint8_t  lba4;
    uint8_t  lba5;
    uint8_t  featureh;

    uint8_t  countl;
    uint8_t  counth;
    uint8_t  icc;
    uint8_t  control;

    uint8_t  reserved[4];
} __attribute__((packed)) fis_reg_h2d_t;

/* AHCI Port Driver State */
typedef struct ahci_port {
    ahci_hba_port_t   *regs;
    uint8_t           port_num;
    uint8_t           is_sata;
    uint64_t          total_sectors;
    char              model[41];
    char              serial[21];

    ahci_cmd_header_t *cmd_list;
    uint64_t          cmd_list_phys;

    void              *received_fis;
    uint64_t          received_fis_phys;

    ahci_cmd_table_t  *cmd_table;
    uint64_t          cmd_table_phys;

    void              *dma_buffer;
    uint64_t          dma_buffer_phys;

    block_device_t    bdev;
} ahci_port_t;

/* AHCI Controller State */
typedef struct ahci_controller {
    const pci_device_t *pci_dev;
    ahci_hba_mem_t     *hba;
    uint64_t           abar_phys;
    uint32_t           ports_implemented;
    uint32_t           port_count;
    ahci_port_t        ports[AHCI_MAX_PORTS];
} ahci_controller_t;

/* Public API */
int  ahci_init(void);
int  ahci_init_controller(ahci_controller_t *ctrl, const pci_device_t *pci_dev, ahci_hba_mem_t *hba_mem);
int  ahci_port_init(ahci_port_t *port);
int  ahci_identify(ahci_port_t *port);
int  ahci_read_sectors(ahci_port_t *port, uint64_t lba, uint32_t count, void *buf);
int  ahci_write_sectors(ahci_port_t *port, uint64_t lba, uint32_t count, const void *buf);
int  ahci_flush(ahci_port_t *port);
uint32_t ahci_get_active_port_count(void);
ahci_port_t *ahci_get_port(uint32_t index);

#ifdef __KERNEL_HOST_TEST__
void ahci_set_command_hook(void (*hook)(ahci_port_t *port));
#endif

#endif /* AHCI_H */
