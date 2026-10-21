#include "e1000.h"
#include "dma.h"
#include "serial.h"
#include "string.h"
#include "purple_detect.h"

#ifndef __KERNEL_HOST_TEST__
#include "vmm.h"
#endif

#define REG_CTRL     0x0000
#define REG_STATUS   0x0008
#define REG_EERD     0x0014
#define REG_ICR      0x00C0
#define REG_IMS      0x00D0
#define REG_RCTL     0x0100
#define REG_RDBAL    0x2800
#define REG_RDBAH    0x2804
#define REG_RDLEN    0x2808
#define REG_RDH      0x2810
#define REG_RDT      0x2818
#define REG_TCTL     0x0400
#define REG_TDBAL    0x3800
#define REG_TDBAH    0x3804
#define REG_TDLEN    0x3808
#define REG_TDH      0x3810
#define REG_TDT      0x3818
#define REG_RAL0     0x5400
#define REG_RAH0     0x5404

#define RCTL_EN             (1 << 1)
#define RCTL_SBP            (1 << 2)
#define RCTL_UPE            (1 << 3)
#define RCTL_MPE            (1 << 4)
#define RCTL_BAM            (1 << 15)
#define RCTL_BSIZE_2048     (0 << 16)
#define RCTL_SECRC          (1 << 26)

#define TCTL_EN             (1 << 1)
#define TCTL_PSP            (1 << 3)
#define TCTL_CT_SHIFT       4
#define TCTL_COLD_SHIFT     12

#define TXD_CMD_EOP         (1 << 0)
#define TXD_CMD_IFCS        (1 << 1)
#define TXD_CMD_RS          (1 << 3)
#define TXD_STAT_DD         (1 << 0)

#define RXD_STAT_DD         (1 << 0)
#define RXD_STAT_EOP        (1 << 1)

static e1000_device_t e1000_dev;
static int e1000_present = 0;

static inline void e1000_write32(uint32_t reg, uint32_t val) {
    if (e1000_dev.mmio_base) {
        volatile uint32_t *p = (volatile uint32_t *)(uintptr_t)(e1000_dev.mmio_base + reg);
        *p = val;
    }
}

static inline uint32_t e1000_read32(uint32_t reg) {
    if (e1000_dev.mmio_base) {
        volatile uint32_t *p = (volatile uint32_t *)(uintptr_t)(e1000_dev.mmio_base + reg);
        return *p;
    }
    return 0;
}

static uint16_t e1000_read_eeprom(uint8_t addr) {
    uint32_t tmp = ((uint32_t)addr << 8) | 0x01; // Start read
    e1000_write32(REG_EERD, tmp);

    uint32_t timeout = 10000;
    while (timeout--) {
        uint32_t val = e1000_read32(REG_EERD);
        if (val & 0x10) { // Done bit
            return (uint16_t)((val >> 16) & 0xFFFF);
        }
    }
    return 0;
}

static void e1000_read_mac(void) {
    uint16_t w0 = e1000_read_eeprom(0);
    uint16_t w1 = e1000_read_eeprom(1);
    uint16_t w2 = e1000_read_eeprom(2);

    if (w0 != 0 || w1 != 0 || w2 != 0) {
        e1000_dev.mac[0] = w0 & 0xFF;
        e1000_dev.mac[1] = (w0 >> 8) & 0xFF;
        e1000_dev.mac[2] = w1 & 0xFF;
        e1000_dev.mac[3] = (w1 >> 8) & 0xFF;
        e1000_dev.mac[4] = w2 & 0xFF;
        e1000_dev.mac[5] = (w2 >> 8) & 0xFF;
    } else {
        // Fallback: Read from RAL0 / RAH0 registers
        uint32_t ral = e1000_read32(REG_RAL0);
        uint32_t rah = e1000_read32(REG_RAH0);
        e1000_dev.mac[0] = ral & 0xFF;
        e1000_dev.mac[1] = (ral >> 8) & 0xFF;
        e1000_dev.mac[2] = (ral >> 16) & 0xFF;
        e1000_dev.mac[3] = (ral >> 24) & 0xFF;
        e1000_dev.mac[4] = rah & 0xFF;
        e1000_dev.mac[5] = (rah >> 8) & 0xFF;
    }

    if (e1000_dev.mac[0] == 0 && e1000_dev.mac[1] == 0 && e1000_dev.mac[2] == 0) {
        // Standard QEMU default MAC fallback: 52:54:00:12:34:56
        e1000_dev.mac[0] = 0x52; e1000_dev.mac[1] = 0x54;
        e1000_dev.mac[2] = 0x00; e1000_dev.mac[3] = 0x12;
        e1000_dev.mac[4] = 0x34; e1000_dev.mac[5] = 0x56;
    }
}

int e1000_transmit(netdev_t *dev, const void *buf, uint32_t len) {
    if (!dev || !buf || len == 0 || len > E1000_BUFFER_SIZE) return -1;
    if (!e1000_present || !e1000_dev.tx_descs) return -2;

    uint32_t cur = e1000_dev.tx_cur;
    e1000_tx_desc_t *desc = &e1000_dev.tx_descs[cur];

    memcpy(e1000_dev.tx_buffers[cur], buf, len);
    desc->length = (uint16_t)len;
    desc->cmd = TXD_CMD_EOP | TXD_CMD_IFCS | TXD_CMD_RS;
    desc->status = 0;

    e1000_dev.tx_cur = (cur + 1) % E1000_NUM_TX_DESC;
    e1000_write32(REG_TDT, e1000_dev.tx_cur);

    dev->tx_packets++;
    dev->tx_bytes += len;
    return (int)len;
}

int e1000_receive(netdev_t *dev, void *buf, uint32_t max_len) {
    if (!dev || !buf || max_len == 0) return -1;
    if (!e1000_present || !e1000_dev.rx_descs) return -2;

    uint32_t cur = e1000_dev.rx_cur;
    e1000_rx_desc_t *desc = &e1000_dev.rx_descs[cur];

    if (!(desc->status & RXD_STAT_DD)) {
        return 0; /* No packet waiting */
    }

    uint32_t len = desc->length;
    if (len > max_len) len = max_len;

    memcpy(buf, e1000_dev.rx_buffers[cur], len);

    desc->status = 0;
    uint32_t old_cur = cur;
    e1000_dev.rx_cur = (cur + 1) % E1000_NUM_RX_DESC;
    e1000_write32(REG_RDT, old_cur);

    dev->rx_packets++;
    dev->rx_bytes += len;
    return (int)len;
}

int e1000_init(void) {
    memset(&e1000_dev, 0, sizeof(e1000_dev));
    e1000_present = 0;

    const pci_device_t *pdev = pci_find_device(E1000_VENDOR_INTEL, E1000_DEV_82540EM);
    if (!pdev) pdev = pci_find_device(E1000_VENDOR_INTEL, E1000_DEV_82545EM);
    if (!pdev) pdev = pci_find_device(E1000_VENDOR_INTEL, E1000_DEV_82574L);
    if (!pdev) pdev = pci_find_device(E1000_VENDOR_INTEL, E1000_DEV_82543GC);

    if (!pdev) {
        kinfo("e1000: No Intel Gigabit Ethernet controller detected on PCI bus");
        return -1;
    }

    e1000_dev.pdev = pdev;
    pci_enable_bus_mastering(pdev->bus, pdev->dev, pdev->func);
    pci_enable_io(pdev->bus, pdev->dev, pdev->func);

    e1000_dev.mmio_base = pci_get_bar_address(pdev, 0);
    if (!e1000_dev.mmio_base) {
        kerror("e1000: Failed to read MMIO BAR0 address");
        return -2;
    }

#ifndef __KERNEL_HOST_TEST__
    page_table_t *pml4 = vmm_get_kernel_pml4();
    if (pml4) {
        /* Map 128 KB MMIO space (32 pages) */
        for (uint32_t p = 0; p < 32; p++) {
            uint64_t a = e1000_dev.mmio_base + ((uint64_t)p * 4096ULL);
            vmm_map_page(pml4, a, a, PAGE_PRESENT | PAGE_WRITABLE);
        }
    }
#endif

    e1000_read_mac();

    /* Allocate Coherent DMA Memory for Descriptors */
    uint64_t rx_phys = 0, tx_phys = 0;
    e1000_dev.rx_descs = (e1000_rx_desc_t *)dma_alloc_coherent(E1000_NUM_RX_DESC * sizeof(e1000_rx_desc_t), &rx_phys);
    e1000_dev.tx_descs = (e1000_tx_desc_t *)dma_alloc_coherent(E1000_NUM_TX_DESC * sizeof(e1000_tx_desc_t), &tx_phys);

    if (!e1000_dev.rx_descs || !e1000_dev.tx_descs) {
        kerror("e1000: Failed to allocate coherent DMA memory for descriptors");
        return -3;
    }

    /* Allocate packet buffer pointer arrays */
    uint64_t dummy = 0;
    e1000_dev.rx_buffers = (void **)dma_alloc_coherent(E1000_NUM_RX_DESC * sizeof(void *), &dummy);
    e1000_dev.tx_buffers = (void **)dma_alloc_coherent(E1000_NUM_TX_DESC * sizeof(void *), &dummy);

    for (uint32_t i = 0; i < E1000_NUM_RX_DESC; i++) {
        uint64_t buf_phys = 0;
        e1000_dev.rx_buffers[i] = dma_alloc_coherent(E1000_BUFFER_SIZE, &buf_phys);
        e1000_dev.rx_descs[i].addr = buf_phys;
        e1000_dev.rx_descs[i].status = 0;
    }

    for (uint32_t i = 0; i < E1000_NUM_TX_DESC; i++) {
        uint64_t buf_phys = 0;
        e1000_dev.tx_buffers[i] = dma_alloc_coherent(E1000_BUFFER_SIZE, &buf_phys);
        e1000_dev.tx_descs[i].addr = buf_phys;
        e1000_dev.tx_descs[i].status = TXD_STAT_DD;
        e1000_dev.tx_descs[i].cmd = 0;
    }

    /* Initialize Hardware RX */
    e1000_write32(REG_RDBAL, (uint32_t)rx_phys);
    e1000_write32(REG_RDBAH, (uint32_t)(rx_phys >> 32));
    e1000_write32(REG_RDLEN, E1000_NUM_RX_DESC * sizeof(e1000_rx_desc_t));
    e1000_write32(REG_RDH, 0);
    e1000_write32(REG_RDT, E1000_NUM_RX_DESC - 1);
    e1000_write32(REG_RCTL, RCTL_EN | RCTL_SBP | RCTL_UPE | RCTL_MPE | RCTL_BAM | RCTL_SECRC);

    /* Initialize Hardware TX */
    e1000_write32(REG_TDBAL, (uint32_t)tx_phys);
    e1000_write32(REG_TDBAH, (uint32_t)(tx_phys >> 32));
    e1000_write32(REG_TDLEN, E1000_NUM_TX_DESC * sizeof(e1000_tx_desc_t));
    e1000_write32(REG_TDH, 0);
    e1000_write32(REG_TDT, 0);
    e1000_write32(REG_TCTL, TCTL_EN | TCTL_PSP | (15 << TCTL_CT_SHIFT) | (64 << TCTL_COLD_SHIFT));

    /* Register NetDev */
    strncpy(e1000_dev.netdev.name, "eth0", sizeof(e1000_dev.netdev.name) - 1);
    memcpy(e1000_dev.netdev.mac_addr, e1000_dev.mac, 6);
    e1000_dev.netdev.mtu = 1500;
    e1000_dev.netdev.transmit = e1000_transmit;
    e1000_dev.netdev.receive = e1000_receive;
    e1000_dev.netdev.priv = &e1000_dev;
    netdev_register(&e1000_dev.netdev);

    e1000_present = 1;
    kinfo("e1000: Intel Gigabit Ethernet Adapter initialized at %02x:%02x.%d (MAC %02x:%02x:%02x:%02x:%02x:%02x)",
          pdev->bus, pdev->dev, pdev->func,
          e1000_dev.mac[0], e1000_dev.mac[1], e1000_dev.mac[2],
          e1000_dev.mac[3], e1000_dev.mac[4], e1000_dev.mac[5]);

    purple_detect_register(PURPLE_CAT_NETWORK, "eth0", "Intel Corporation",
                           "82540EM Gigabit Ethernet", "PCI", e1000_dev.mmio_base,
                           pdev->irq, PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);

    return 0;
}

int e1000_is_present(void) {
    return e1000_present;
}

const e1000_device_t *e1000_get_device(void) {
    if (!e1000_present) return NULL;
    return &e1000_dev;
}
