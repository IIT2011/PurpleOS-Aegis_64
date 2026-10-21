#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "ahci.h"
#include "dma.h"

static ahci_hba_mem_t s_mock_hba;
static uint8_t s_mock_drive_data[256 * 512]; // 256 sectors mock drive

static void mock_hba_reset(void) {
    memset((void *)&s_mock_hba, 0, sizeof(s_mock_hba));
    memset(s_mock_drive_data, 0, sizeof(s_mock_drive_data));
    s_mock_hba.cap = 0x0000001F; // 32 ports
    s_mock_hba.vs = 0x00010301;  // AHCI 1.3.1
    s_mock_hba.pi = 0x00000001;  // Port 0 implemented
    s_mock_hba.ports[0].ssts = (HBA_PORT_IPM_ACTIVE << 8) | HBA_PORT_DET_PRESENT;
    s_mock_hba.ports[0].sig = AHCI_SIG_ATA;
}

// Emulate hardware processing when command is issued
static void emulate_ahci_hardware(ahci_port_t *port) {
    if ((port->regs->ci & 1) == 0) return;

    ahci_cmd_header_t *hdr = &port->cmd_list[0];
    ahci_cmd_table_t *tbl = port->cmd_table;
    fis_reg_h2d_t *fis = (fis_reg_h2d_t *)tbl->cfis;

    if (fis->command == ATA_CMD_IDENTIFY_DEV) {
        // Generate identify sector in dma buffer
        uint16_t *id_words = (uint16_t *)port->dma_buffer;
        memset(id_words, 0, 512);

        // Serial "SATATEST123"
        const char *serial = "SATATEST123         ";
        for (int i = 0; i < 10; i++) {
            id_words[10 + i] = ((uint16_t)serial[i*2] << 8) | (uint8_t)serial[i*2 + 1];
        }

        // Model "QEMU HARDDISK SATA"
        const char *model = "QEMU HARDDISK SATA                      ";
        for (int i = 0; i < 20; i++) {
            id_words[27 + i] = ((uint16_t)model[i*2] << 8) | (uint8_t)model[i*2 + 1];
        }

        // Support LBA48 (word 83 bit 10)
        id_words[83] = (1 << 10);
        // Total sectors = 256
        id_words[100] = 256;
        id_words[101] = 0;
        id_words[102] = 0;
        id_words[103] = 0;

        hdr->prdbc = 512;
    } else if (fis->command == ATA_CMD_READ_DMA_EXT) {
        uint64_t lba = (uint64_t)fis->lba0 | ((uint64_t)fis->lba1 << 8) | ((uint64_t)fis->lba2 << 16) |
                       ((uint64_t)fis->lba3 << 24) | ((uint64_t)fis->lba4 << 32) | ((uint64_t)fis->lba5 << 40);
        uint32_t count = (uint32_t)fis->countl | ((uint32_t)fis->counth << 8);
        assert(lba + count <= 256);
        memcpy(port->dma_buffer, s_mock_drive_data + lba * 512, count * 512);
        hdr->prdbc = count * 512;
    } else if (fis->command == ATA_CMD_WRITE_DMA_EXT) {
        uint64_t lba = (uint64_t)fis->lba0 | ((uint64_t)fis->lba1 << 8) | ((uint64_t)fis->lba2 << 16) |
                       ((uint64_t)fis->lba3 << 24) | ((uint64_t)fis->lba4 << 32) | ((uint64_t)fis->lba5 << 40);
        uint32_t count = (uint32_t)fis->countl | ((uint32_t)fis->counth << 8);
        assert(lba + count <= 256);
        memcpy(s_mock_drive_data + lba * 512, port->dma_buffer, count * 512);
        hdr->prdbc = count * 512;
    }

    // Hardware completes command by clearing bit 0 of CI
    port->regs->ci &= ~1U;
}

static void test_ahci_port_init_and_identify(void) {
    printf("[TEST  1] Running test_ahci_port_init_and_identify         ... ");
    mock_hba_reset();

    ahci_controller_t ctrl;
    memset(&ctrl, 0, sizeof(ctrl));
    ctrl.hba = &s_mock_hba;

    ahci_port_t port;
    memset(&port, 0, sizeof(port));
    port.port_num = 0;
    port.regs = &s_mock_hba.ports[0];

    int rc = ahci_port_init(&port);
    assert(rc == 0);
    assert(port.cmd_list != NULL);
    assert(port.cmd_table != NULL);
    assert(port.received_fis != NULL);
    assert(port.dma_buffer != NULL);
    assert(port.regs->clb != 0);
    assert(port.regs->fb != 0);
    assert(port.regs->cmd & HBA_CMD_ST);
    assert(port.regs->cmd & HBA_CMD_FRE);

    // Register test hardware emulator hook
    ahci_set_command_hook(emulate_ahci_hardware);

    rc = ahci_identify(&port);
    assert(rc == 0);
    assert(port.is_sata == 1);
    assert(port.total_sectors == 256);
    assert(strstr(port.model, "QEMU HARDDISK") != NULL);
    assert(strstr(port.serial, "SATATEST") != NULL);
    assert(port.bdev.total_blocks == 256);

    printf("PASSED\n");
}

static void test_ahci_read_write_flush(void) {
    printf("[TEST  2] Running test_ahci_read_write_flush               ... ");
    mock_hba_reset();
    ahci_set_command_hook(emulate_ahci_hardware);

    ahci_port_t port;
    memset(&port, 0, sizeof(port));
    port.port_num = 0;
    port.regs = &s_mock_hba.ports[0];
    ahci_port_init(&port);
    ahci_identify(&port);

    // Test write 2 sectors to LBA 10
    uint8_t wbuf[1024];
    for (int i = 0; i < 1024; i++) wbuf[i] = (uint8_t)(i ^ 0x3C);

    int wrc = ahci_write_sectors(&port, 10, 2, wbuf);
    assert(wrc == 0);

    // Verify written data in mock storage
    assert(memcmp(s_mock_drive_data + 10 * 512, wbuf, 1024) == 0);

    // Test read 2 sectors back from LBA 10
    uint8_t rbuf[1024];
    memset(rbuf, 0, sizeof(rbuf));
    int rrc = ahci_read_sectors(&port, 10, 2, rbuf);
    assert(rrc == 0);
    assert(memcmp(rbuf, wbuf, 1024) == 0);

    // Test flush
    assert(ahci_flush(&port) == 0);

    printf("PASSED\n");
}
    fis->command = ATA_CMD_READ_DMA_EXT;
    fis->device = 1 << 6;
    fis->lba0 = 10;
    fis->countl = 2;
    port.regs->ci = 1;
    emulate_ahci_hardware(&port);
    assert((port.regs->ci & 1) == 0);
    assert(memcmp(port.dma_buffer, wbuf, 1024) == 0);

    printf("PASSED\n");
}

static void test_ahci_bounds_checking(void) {
    printf("[TEST  3] Running test_ahci_bounds_checking                ... ");
    mock_hba_reset();

    ahci_port_t port;
    memset(&port, 0, sizeof(port));
    port.port_num = 0;
    port.regs = &s_mock_hba.ports[0];
    port.is_sata = 1;
    port.total_sectors = 256;

    uint8_t buf[512];
    // Out of bounds: LBA >= total_sectors
    assert(ahci_read_sectors(&port, 256, 1, buf) == -2);
    assert(ahci_write_sectors(&port, 256, 1, buf) == -2);

    // Out of bounds: LBA + count > total_sectors
    assert(ahci_read_sectors(&port, 250, 10, buf) == -2);
    assert(ahci_write_sectors(&port, 250, 10, buf) == -2);

    // Invalid parameters
    assert(ahci_read_sectors(NULL, 0, 1, buf) == -1);
    assert(ahci_read_sectors(&port, 0, 0, buf) == -1);
    assert(ahci_read_sectors(&port, 0, 1, NULL) == -1);

    printf("PASSED\n");
}

int main(void) {
    printf("=== Aegis64 AHCI SATA Host Controller Test Suite ===\n");
    test_ahci_port_init_and_identify();
    test_ahci_read_write_flush();
    test_ahci_bounds_checking();
    printf("=== All AHCI Host Tests Passed Successfully! ===\n");
    return 0;
}
