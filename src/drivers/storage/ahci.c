#include "ahci.h"
#include "dma.h"
#include "string.h"
#include "serial.h"

static ahci_controller_t g_ahci_ctrl;
static uint32_t g_active_port_count = 0;
static ahci_port_t *g_active_ports[AHCI_MAX_PORTS];

#ifdef __KERNEL_HOST_TEST__
static void (*s_ahci_cmd_hook)(ahci_port_t *port) = NULL;
void ahci_set_command_hook(void (*hook)(ahci_port_t *port)) {
    s_ahci_cmd_hook = hook;
}
#endif

static int ahci_bdev_read(block_device_t *bdev, uint64_t lba, uint32_t count, void *buf) {
    ahci_port_t *port = (ahci_port_t *)bdev->priv;
    return ahci_read_sectors(port, lba, count, buf) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int ahci_bdev_write(block_device_t *bdev, uint64_t lba, uint32_t count, const void *buf) {
    ahci_port_t *port = (ahci_port_t *)bdev->priv;
    return ahci_write_sectors(port, lba, count, buf) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int ahci_bdev_flush(block_device_t *bdev) {
    ahci_port_t *port = (ahci_port_t *)bdev->priv;
    return ahci_flush(port) == 0 ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static const block_device_ops_t ahci_bdev_ops = {
    .read_blocks = ahci_bdev_read,
    .write_blocks = ahci_bdev_write,
    .flush = ahci_bdev_flush,
};

static void ahci_stop_cmd(ahci_hba_port_t *regs) {
    regs->cmd &= ~HBA_CMD_ST;
    regs->cmd &= ~HBA_CMD_FRE;

    for (uint32_t timeout = 0; timeout < 50000; timeout++) {
        if ((regs->cmd & (HBA_CMD_FR | HBA_CMD_CR)) == 0) {
            break;
        }
    }
}

static void ahci_start_cmd(ahci_hba_port_t *regs) {
    for (uint32_t timeout = 0; timeout < 50000; timeout++) {
        if ((regs->cmd & HBA_CMD_CR) == 0) {
            break;
        }
    }
    regs->cmd |= HBA_CMD_FRE;
    regs->cmd |= HBA_CMD_ST;
}

int ahci_port_init(ahci_port_t *port) {
    if (!port || !port->regs) return -1;

    ahci_stop_cmd(port->regs);

    // 1. Allocate Command List (1024 bytes, 1024-byte aligned)
    port->cmd_list = (ahci_cmd_header_t *)dma_alloc_coherent(1024, &port->cmd_list_phys);
    if (!port->cmd_list) {
        kwarn("AHCI: Failed to allocate command list for port %u", port->port_num);
        return -1;
    }

    // 2. Allocate Received FIS (256 bytes, 256-byte aligned)
    port->received_fis = dma_alloc_coherent(256, &port->received_fis_phys);
    if (!port->received_fis) {
        kwarn("AHCI: Failed to allocate received FIS for port %u", port->port_num);
        return -1;
    }

    // 3. Allocate Command Table (128 bytes + PRDT)
    port->cmd_table = (ahci_cmd_table_t *)dma_alloc_coherent(sizeof(ahci_cmd_table_t), &port->cmd_table_phys);
    if (!port->cmd_table) {
        kwarn("AHCI: Failed to allocate command table for port %u", port->port_num);
        return -1;
    }

    // 4. Allocate DMA Scratch Buffer (64 KiB)
    port->dma_buffer = dma_alloc_coherent(65536, &port->dma_buffer_phys);
    if (!port->dma_buffer) {
        kwarn("AHCI: Failed to allocate DMA scratch buffer for port %u", port->port_num);
        return -1;
    }

    // Set register addresses
    port->regs->clb = (uint32_t)port->cmd_list_phys;
    port->regs->clbu = (uint32_t)(port->cmd_list_phys >> 32);

    port->regs->fb = (uint32_t)port->received_fis_phys;
    port->regs->fbu = (uint32_t)(port->received_fis_phys >> 32);

    // Configure Command Header 0 to point to our Command Table
    ahci_cmd_header_t *hdr0 = &port->cmd_list[0];
    hdr0->ctba = (uint32_t)port->cmd_table_phys;
    hdr0->ctbau = (uint32_t)(port->cmd_table_phys >> 32);

    // Clear error status and pending interrupts
    port->regs->serr = port->regs->serr;
    port->regs->is = port->regs->is;

    ahci_start_cmd(port->regs);
    return 0;
}

void ahci_port_free(ahci_port_t *port) {
    if (!port) return;
    if (port->cmd_list) {
        dma_free_coherent(port->cmd_list, 1024);
        port->cmd_list = NULL;
    }
    if (port->received_fis) {
        dma_free_coherent(port->received_fis, 256);
        port->received_fis = NULL;
    }
    if (port->cmd_table) {
        dma_free_coherent(port->cmd_table, sizeof(ahci_cmd_table_t));
        port->cmd_table = NULL;
    }
    if (port->dma_buffer) {
        dma_free_coherent(port->dma_buffer, 65536);
        port->dma_buffer = NULL;
    }
}

static int ahci_issue_command_slot0(ahci_port_t *port) {
    // Wait until port is not busy
    for (uint32_t spin = 0; spin < 500000; spin++) {
        if ((port->regs->tfd & (HBA_TFD_BSY | HBA_TFD_DRQ)) == 0) {
            break;
        }
    }

    // Issue slot 0
    port->regs->ci = (1U << 0);

#ifdef __KERNEL_HOST_TEST__
    if (s_ahci_cmd_hook) {
        s_ahci_cmd_hook(port);
    }
#endif

    // Bounded spin poll for completion
    for (uint32_t spin = 0; spin < 1000000; spin++) {
        if ((port->regs->ci & (1U << 0)) == 0) {
            break; // Command complete!
        }
        if (port->regs->tfd & HBA_TFD_ERR) {
            kwarn("AHCI: Command error on port %u (TFD=0x%x)", port->port_num, port->regs->tfd);
            return -1;
        }
    }

    if (port->regs->ci & (1U << 0)) {
        kwarn("AHCI: Command timeout on port %u (CI=0x%x)", port->port_num, port->regs->ci);
        return -2;
    }

    if (port->regs->tfd & HBA_TFD_ERR) {
        return -1;
    }

    return 0;
}

int ahci_identify(ahci_port_t *port) {
    if (!port || !port->regs || !port->cmd_list || !port->cmd_table) return -1;

    memset(port->cmd_table, 0, sizeof(ahci_cmd_table_t));

    fis_reg_h2d_t *fis = (fis_reg_h2d_t *)port->cmd_table->cfis;
    fis->fis_type = FIS_TYPE_REG_H2D;
    fis->pmport_c = 0x80; // C=1 (Command)
    fis->command = ATA_CMD_IDENTIFY_DEV;

    // PRDT 0: 512 bytes into DMA buffer
    port->cmd_table->prdt[0].dba = (uint32_t)port->dma_buffer_phys;
    port->cmd_table->prdt[0].dbau = (uint32_t)(port->dma_buffer_phys >> 32);
    port->cmd_table->prdt[0].dbc = 511; // 512 bytes (0-based)

    ahci_cmd_header_t *hdr = &port->cmd_list[0];
    hdr->flags = 5; // 5 DWORDS in Command FIS, write=0
    hdr->prdtl = 1;
    hdr->prdbc = 0;

    int rc = ahci_issue_command_slot0(port);
    if (rc != 0) {
        return rc;
    }

    // Parse IDENTIFY data
    const uint16_t *id_data = (const uint16_t *)port->dma_buffer;

    // Serial (words 10..19, 20 chars)
    for (int i = 0; i < 10; i++) {
        uint16_t w = id_data[10 + i];
        port->serial[i * 2]     = (char)((w >> 8) & 0xFF);
        port->serial[i * 2 + 1] = (char)(w & 0xFF);
    }
    port->serial[20] = '\0';
    // Trim trailing spaces
    for (int i = 19; i >= 0 && (port->serial[i] == ' ' || port->serial[i] == '\0'); i--) {
        port->serial[i] = '\0';
    }

    // Model (words 27..46, 40 chars)
    for (int i = 0; i < 20; i++) {
        uint16_t w = id_data[27 + i];
        port->model[i * 2]     = (char)((w >> 8) & 0xFF);
        port->model[i * 2 + 1] = (char)(w & 0xFF);
    }
    port->model[40] = '\0';
    for (int i = 39; i >= 0 && (port->model[i] == ' ' || port->model[i] == '\0'); i--) {
        port->model[i] = '\0';
    }

    // Capacity (LBA48 support in word 83 bit 10)
    if (id_data[83] & (1 << 10)) {
        uint64_t s0 = id_data[100];
        uint64_t s1 = id_data[101];
        uint64_t s2 = id_data[102];
        uint64_t s3 = id_data[103];
        port->total_sectors = s0 | (s1 << 16) | (s2 << 32) | (s3 << 48);
    } else {
        uint32_t s0 = id_data[60];
        uint32_t s1 = id_data[61];
        port->total_sectors = s0 | (s1 << 16);
    }

    port->is_sata = 1;

    // Register with block device layer
    memset(&port->bdev, 0, sizeof(port->bdev));
    port->bdev.name[0] = 's';
    port->bdev.name[1] = 'a';
    port->bdev.name[2] = 't';
    port->bdev.name[3] = 'a';
    if (port->port_num < 10) {
        port->bdev.name[4] = '0' + port->port_num;
        port->bdev.name[5] = '\0';
    } else {
        port->bdev.name[4] = '0' + (port->port_num / 10);
        port->bdev.name[5] = '0' + (port->port_num % 10);
        port->bdev.name[6] = '\0';
    }
    strncpy(port->bdev.model, port->model, sizeof(port->bdev.model) - 1);
    strncpy(port->bdev.serial, port->serial, sizeof(port->bdev.serial) - 1);
    port->bdev.type = BLOCK_DEV_TYPE_AHCI;
    port->bdev.block_size = 512;
    port->bdev.total_blocks = port->total_sectors;
    port->bdev.ops = &ahci_bdev_ops;
    port->bdev.priv = port;

    block_dev_register(&port->bdev);

    kinfo("AHCI: Port %u identified: Model='%s', Serial='%s', Sectors=%u (%u MiB)",
          port->port_num, port->model, port->serial, (uint32_t)port->total_sectors,
          (uint32_t)((port->total_sectors * 512) / (1024 * 1024)));

    return 0;
}

int ahci_read_sectors(ahci_port_t *port, uint64_t lba, uint32_t count, void *buf) {
    if (!port || !port->is_sata || count == 0 || !buf) return -1;
    if (lba >= port->total_sectors || lba + count > port->total_sectors || lba + count < lba) {
        return -2;
    }

    uint8_t *dest = (uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t cur_lba = lba;

    while (remaining > 0) {
        uint32_t chunk = (remaining > 128) ? 128 : remaining; // 128 sectors = 64 KiB

        memset(port->cmd_table, 0, sizeof(ahci_cmd_table_t));
        fis_reg_h2d_t *fis = (fis_reg_h2d_t *)port->cmd_table->cfis;
        fis->fis_type = FIS_TYPE_REG_H2D;
        fis->pmport_c = 0x80;
        fis->command = ATA_CMD_READ_DMA_EXT;
        fis->device = 1 << 6; // LBA mode

        fis->lba0 = (uint8_t)(cur_lba & 0xFF);
        fis->lba1 = (uint8_t)((cur_lba >> 8) & 0xFF);
        fis->lba2 = (uint8_t)((cur_lba >> 16) & 0xFF);
        fis->lba3 = (uint8_t)((cur_lba >> 24) & 0xFF);
        fis->lba4 = (uint8_t)((cur_lba >> 32) & 0xFF);
        fis->lba5 = (uint8_t)((cur_lba >> 40) & 0xFF);

        fis->countl = (uint8_t)(chunk & 0xFF);
        fis->counth = (uint8_t)((chunk >> 8) & 0xFF);

        port->cmd_table->prdt[0].dba = (uint32_t)port->dma_buffer_phys;
        port->cmd_table->prdt[0].dbau = (uint32_t)(port->dma_buffer_phys >> 32);
        port->cmd_table->prdt[0].dbc = (chunk * 512) - 1;

        ahci_cmd_header_t *hdr = &port->cmd_list[0];
        hdr->flags = 5; // 5 DWORDS, write=0
        hdr->prdtl = 1;
        hdr->prdbc = 0;

        int rc = ahci_issue_command_slot0(port);
        if (rc != 0) {
            return rc;
        }

        memcpy(dest, port->dma_buffer, chunk * 512);

        dest += (chunk * 512);
        cur_lba += chunk;
        remaining -= chunk;
    }

    return 0;
}

int ahci_write_sectors(ahci_port_t *port, uint64_t lba, uint32_t count, const void *buf) {
    if (!port || !port->is_sata || count == 0 || !buf) return -1;
    if (lba >= port->total_sectors || lba + count > port->total_sectors || lba + count < lba) {
        return -2;
    }

    const uint8_t *src = (const uint8_t *)buf;
    uint32_t remaining = count;
    uint64_t cur_lba = lba;

    while (remaining > 0) {
        uint32_t chunk = (remaining > 128) ? 128 : remaining;

        memcpy(port->dma_buffer, src, chunk * 512);

        memset(port->cmd_table, 0, sizeof(ahci_cmd_table_t));
        fis_reg_h2d_t *fis = (fis_reg_h2d_t *)port->cmd_table->cfis;
        fis->fis_type = FIS_TYPE_REG_H2D;
        fis->pmport_c = 0x80;
        fis->command = ATA_CMD_WRITE_DMA_EXT;
        fis->device = 1 << 6;

        fis->lba0 = (uint8_t)(cur_lba & 0xFF);
        fis->lba1 = (uint8_t)((cur_lba >> 8) & 0xFF);
        fis->lba2 = (uint8_t)((cur_lba >> 16) & 0xFF);
        fis->lba3 = (uint8_t)((cur_lba >> 24) & 0xFF);
        fis->lba4 = (uint8_t)((cur_lba >> 32) & 0xFF);
        fis->lba5 = (uint8_t)((cur_lba >> 40) & 0xFF);

        fis->countl = (uint8_t)(chunk & 0xFF);
        fis->counth = (uint8_t)((chunk >> 8) & 0xFF);

        port->cmd_table->prdt[0].dba = (uint32_t)port->dma_buffer_phys;
        port->cmd_table->prdt[0].dbau = (uint32_t)(port->dma_buffer_phys >> 32);
        port->cmd_table->prdt[0].dbc = (chunk * 512) - 1;

        ahci_cmd_header_t *hdr = &port->cmd_list[0];
        hdr->flags = 5 | (1 << 6); // 5 DWORDS, bit 6 = Write
        hdr->prdtl = 1;
        hdr->prdbc = 0;

        int rc = ahci_issue_command_slot0(port);
        if (rc != 0) {
            return rc;
        }

        src += (chunk * 512);
        cur_lba += chunk;
        remaining -= chunk;
    }

    return ahci_flush(port);
}

int ahci_flush(ahci_port_t *port) {
    if (!port || !port->is_sata) return -1;

    memset(port->cmd_table, 0, sizeof(ahci_cmd_table_t));
    fis_reg_h2d_t *fis = (fis_reg_h2d_t *)port->cmd_table->cfis;
    fis->fis_type = FIS_TYPE_REG_H2D;
    fis->pmport_c = 0x80;
    fis->command = ATA_CMD_CACHE_FLUSH_EXT;
    fis->device = 1 << 6;

    ahci_cmd_header_t *hdr = &port->cmd_list[0];
    hdr->flags = 5;
    hdr->prdtl = 0;
    hdr->prdbc = 0;

    return ahci_issue_command_slot0(port);
}

int ahci_init_controller(ahci_controller_t *ctrl, const pci_device_t *pci_dev, ahci_hba_mem_t *hba_mem) {
    if (!ctrl || !hba_mem) return -1;

    memset(ctrl, 0, sizeof(*ctrl));
    ctrl->pci_dev = pci_dev;
    ctrl->hba = hba_mem;

    // 1. Enable AHCI Mode (GHC.AE)
    ctrl->hba->ghc |= HBA_GHC_AE;

    // 2. Perform HBA Reset
    ctrl->hba->ghc |= HBA_GHC_HR;
    for (uint32_t t = 0; t < 100000; t++) {
        if ((ctrl->hba->ghc & HBA_GHC_HR) == 0) {
            break;
        }
    }
    // Re-enable AHCI after reset
    ctrl->hba->ghc |= HBA_GHC_AE;

    ctrl->ports_implemented = ctrl->hba->pi;
    kinfo("AHCI: Controller initialized, Ports Implemented mask: 0x%x (cap=0x%x, vs=0x%x)",
          ctrl->ports_implemented, ctrl->hba->cap, ctrl->hba->vs);

    // 3. Scan ports
    uint32_t found = 0;
    for (uint32_t i = 0; i < AHCI_MAX_PORTS; i++) {
        if (ctrl->ports_implemented & (1U << i)) {
            ahci_hba_port_t *regs = &ctrl->hba->ports[i];
            uint32_t ssts = regs->ssts;
            uint8_t det = (uint8_t)(ssts & 0x0F);
            uint8_t ipm = (uint8_t)((ssts >> 8) & 0x0F);

            if (det == HBA_PORT_DET_PRESENT && ipm == HBA_PORT_IPM_ACTIVE) {
                ahci_port_t *port = &ctrl->ports[i];
                port->port_num = (uint8_t)i;
                port->regs = regs;

                if (ahci_port_init(port) == 0) {
                    // Wait briefly for initial D2H FIS signature to latch
                    for (uint32_t t = 0; t < 100000; t++) {
                        if ((regs->tfd & HBA_TFD_BSY) == 0 && regs->sig != 0xFFFFFFFF) {
                            break;
                        }
                    }

                    uint32_t sig = regs->sig;
                    kinfo("AHCI: Port %u link active (SSTS=0x%x, SIG=0x%08x)", i, ssts, sig);

                    if (sig == AHCI_SIG_ATA || sig == 0 || sig == 0xFFFFFFFF) {
                        if (ahci_identify(port) == 0) {
                            if (g_active_port_count < AHCI_MAX_PORTS) {
                                g_active_ports[g_active_port_count++] = port;
                            }
                            found++;
                        }
                    } else if (sig == AHCI_SIG_ATAPI) {
                        kinfo("AHCI: Port %u is ATAPI optical device (skipped)", i);
                    }
                }
            }
        }
    }

    ctrl->port_count = found;
    kinfo("AHCI: %u active SATA drives registered", found);
    return 0;
}

int ahci_init(void) {
    const pci_device_t *pdev = pci_find_by_class(AHCI_PCI_CLASS, AHCI_PCI_SUBCLASS, AHCI_PCI_PROG_IF);
    if (!pdev) {
        kinfo("AHCI: No AHCI SATA controller detected on PCI bus");
        return -1;
    }

    kinfo("AHCI: Found controller at PCI %02x:%02x.%d [Vendor %04x, Device %04x]",
          pdev->bus, pdev->dev, pdev->func, pdev->vendor_id, pdev->device_id);

    // Enable Bus Mastering and MMIO Space
    pci_enable_bus_mastering(pdev->bus, pdev->dev, pdev->func);
    pci_enable_io(pdev->bus, pdev->dev, pdev->func);

    // BAR 5 is ABAR
    uint64_t abar_phys = pci_get_bar_address(pdev, 5);
    uint64_t abar_size = pci_get_bar_size(pdev, 5);
    if (abar_phys == 0) {
        kwarn("AHCI: Invalid ABAR (BAR 5 is 0)");
        return -2;
    }

    kinfo("AHCI: ABAR physical address = 0x%lx (size %lu bytes)", abar_phys, abar_size);
    ahci_hba_mem_t *hba = (ahci_hba_mem_t *)mmio_map(abar_phys, (size_t)abar_size);

    return ahci_init_controller(&g_ahci_ctrl, pdev, hba);
}

uint32_t ahci_get_active_port_count(void) {
    return g_active_port_count;
}

ahci_port_t *ahci_get_port(uint32_t index) {
    if (index >= g_active_port_count) return NULL;
    return g_active_ports[index];
}
