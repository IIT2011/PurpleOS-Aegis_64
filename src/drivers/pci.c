#include "pci.h"
#include "io.h"
#include "serial.h"
#include "string.h"

static pci_device_t pci_devices[PCI_MAX_DEVICES];
static uint32_t pci_device_count = 0;

uint32_t pci_read_config_dword(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset) {
    uint32_t address = (uint32_t)((1U << 31) | ((uint32_t)bus << 16) |
                                  ((uint32_t)dev << 11) | ((uint32_t)func << 8) |
                                  (offset & 0xFC));
    outl(PCI_CONFIG_ADDRESS, address);
    return inl(PCI_CONFIG_DATA);
}

uint16_t pci_read_config_word(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset) {
    uint32_t val = pci_read_config_dword(bus, dev, func, offset);
    return (uint16_t)((val >> ((offset & 2) * 8)) & 0xFFFF);
}

uint8_t pci_read_config_byte(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset) {
    uint32_t val = pci_read_config_dword(bus, dev, func, offset);
    return (uint8_t)((val >> ((offset & 3) * 8)) & 0xFF);
}

void pci_write_config_dword(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset, uint32_t val) {
    uint32_t address = (uint32_t)((1U << 31) | ((uint32_t)bus << 16) |
                                  ((uint32_t)dev << 11) | ((uint32_t)func << 8) |
                                  (offset & 0xFC));
    outl(PCI_CONFIG_ADDRESS, address);
    outl(PCI_CONFIG_DATA, val);
}

void pci_write_config_word(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset, uint16_t val) {
    uint32_t old = pci_read_config_dword(bus, dev, func, offset);
    uint32_t shift = (offset & 2) * 8;
    uint32_t mask = 0xFFFFU << shift;
    uint32_t updated = (old & ~mask) | (((uint32_t)val & 0xFFFFU) << shift);
    pci_write_config_dword(bus, dev, func, offset, updated);
}

const char *pci_vendor_to_str(uint16_t vendor_id) {
    switch (vendor_id) {
        case 0x8086: return "Intel Corp.";
        case 0x1022: return "AMD Inc.";
        case 0x1234: return "Bochs / QEMU";
        case 0x1B36: return "Red Hat / QEMU";
        case 0x10EC: return "Realtek Semiconductor";
        case 0x15AD: return "VMware Inc.";
        case 0x1013: return "Cirrus Logic";
        case 0x1274: return "Ensoniq / Creative";
        default:     return "Standard PCI";
    }
}

const char *pci_class_to_str(uint8_t class_code, uint8_t subclass) {
    switch (class_code) {
        case 0x01:
            switch (subclass) {
                case 0x01: return "IDE Interface";
                case 0x06: return "SATA Controller (AHCI)";
                case 0x08: return "NVMe Controller";
                default:   return "Mass Storage Controller";
            }
        case 0x02:
            switch (subclass) {
                case 0x00: return "Ethernet Controller";
                default:   return "Network Controller";
            }
        case 0x03:
            switch (subclass) {
                case 0x00: return "VGA Compatible Controller";
                default:   return "Display Controller";
            }
        case 0x04:
            switch (subclass) {
                case 0x01: return "Audio Device (AC97)";
                case 0x03: return "High Definition Audio";
                default:   return "Multimedia Device";
            }
        case 0x06:
            switch (subclass) {
                case 0x00: return "Host Bridge";
                case 0x01: return "ISA Bridge";
                case 0x04: return "PCI-to-PCI Bridge";
                default:   return "Bridge Device";
            }
        case 0x0C:
            switch (subclass) {
                case 0x03: return "USB Controller";
                default:   return "Serial Bus Controller";
            }
        default:
            return "Other Device";
    }
}

void pci_scan(void) {
    pci_device_count = 0;
    memset(pci_devices, 0, sizeof(pci_devices));

    for (uint16_t bus = 0; bus < 8 && pci_device_count < PCI_MAX_DEVICES; bus++) {
        for (uint8_t dev = 0; dev < 32 && pci_device_count < PCI_MAX_DEVICES; dev++) {
            uint8_t max_func = 1;
            uint16_t v0 = pci_read_config_word((uint8_t)bus, dev, 0, PCI_REG_VENDOR_ID);
            if (v0 == 0xFFFF || v0 == 0x0000) {
                continue;
            }

            uint8_t header_type = pci_read_config_byte((uint8_t)bus, dev, 0, PCI_REG_HEADER_TYPE);
            if (header_type & 0x80) {
                max_func = 8;
            }

            for (uint8_t func = 0; func < max_func && pci_device_count < PCI_MAX_DEVICES; func++) {
                uint16_t vendor = pci_read_config_word((uint8_t)bus, dev, func, PCI_REG_VENDOR_ID);
                if (vendor == 0xFFFF || vendor == 0x0000) {
                    continue;
                }

                uint16_t device = pci_read_config_word((uint8_t)bus, dev, func, PCI_REG_DEVICE_ID);
                uint8_t class_code = pci_read_config_byte((uint8_t)bus, dev, func, PCI_REG_CLASS);
                uint8_t subclass = pci_read_config_byte((uint8_t)bus, dev, func, PCI_REG_SUBCLASS);
                uint8_t prog_if = pci_read_config_byte((uint8_t)bus, dev, func, PCI_REG_PROG_IF);
                uint8_t revision = pci_read_config_byte((uint8_t)bus, dev, func, PCI_REG_REVISION_ID);
                uint8_t irq = pci_read_config_byte((uint8_t)bus, dev, func, PCI_REG_INTERRUPT_LINE);

                pci_device_t *pdev = &pci_devices[pci_device_count++];
                pdev->bus = (uint8_t)bus;
                pdev->dev = dev;
                pdev->func = func;
                pdev->vendor_id = vendor;
                pdev->device_id = device;
                pdev->class_code = class_code;
                pdev->subclass = subclass;
                pdev->prog_if = prog_if;
                pdev->revision = revision;
                pdev->irq = irq;

                for (int b = 0; b < 6; b++) {
                    pdev->bar[b] = pci_read_config_dword((uint8_t)bus, dev, func, PCI_REG_BAR0 + (b * 4));
                }

                kinfo("PCI: Discovered %02x:%02x.%d [%04x:%04x] %s - %s (IRQ %u)",
                      bus, dev, func, vendor, device,
                      pci_vendor_to_str(vendor),
                      pci_class_to_str(class_code, subclass),
                      irq);
            }
        }
    }

    kinfo("PCI: Scan complete, %u devices registered", pci_device_count);
}

int pci_init(void) {
    pci_scan();
    return 0;
}

uint32_t pci_get_device_count(void) {
    return pci_device_count;
}

const pci_device_t *pci_get_device(uint32_t index) {
    if (index >= pci_device_count) return NULL;
    return &pci_devices[index];
}

const pci_device_t *pci_find_device(uint16_t vendor_id, uint16_t device_id) {
    for (uint32_t i = 0; i < pci_device_count; i++) {
        if (pci_devices[i].vendor_id == vendor_id &&
            (device_id == 0xFFFF || pci_devices[i].device_id == device_id)) {
            return &pci_devices[i];
        }
    }
    return NULL;
}

const pci_device_t *pci_find_by_class(uint8_t class_code, uint8_t subclass, uint8_t prog_if) {
    for (uint32_t i = 0; i < pci_device_count; i++) {
        if (pci_devices[i].class_code == class_code &&
            pci_devices[i].subclass == subclass &&
            (prog_if == 0xFF || pci_devices[i].prog_if == prog_if)) {
            return &pci_devices[i];
        }
    }
    return NULL;
}

uint64_t pci_get_bar_address(const pci_device_t *dev, uint8_t bar_index) {
    if (!dev || bar_index >= 6) return 0;
    uint32_t bar_val = dev->bar[bar_index];
    if (bar_val & 1) {
        // I/O space
        return (uint64_t)(bar_val & 0xFFFFFFFCU);
    } else {
        // Memory space
        uint8_t type = (bar_val >> 1) & 0x03;
        if (type == 0x02 && bar_index < 5) {
            // 64-bit memory space
            uint64_t high = (uint64_t)dev->bar[bar_index + 1];
            return (high << 32) | (bar_val & 0xFFFFFFF0ULL);
        } else {
            // 32-bit memory space
            return (uint64_t)(bar_val & 0xFFFFFFF0ULL);
        }
    }
}

uint64_t pci_get_bar_size(const pci_device_t *dev, uint8_t bar_index) {
    if (!dev || bar_index >= 6) return 0;
    uint8_t offset = PCI_REG_BAR0 + (bar_index * 4);
    uint32_t orig = pci_read_config_dword(dev->bus, dev->dev, dev->func, offset);
    pci_write_config_dword(dev->bus, dev->dev, dev->func, offset, 0xFFFFFFFF);
    uint32_t mask = pci_read_config_dword(dev->bus, dev->dev, dev->func, offset);
    pci_write_config_dword(dev->bus, dev->dev, dev->func, offset, orig);

    if (orig & 1) {
        mask &= 0xFFFFFFFC;
        return (uint64_t)((~mask) + 1);
    } else {
        mask &= 0xFFFFFFF0;
        return (uint64_t)((~mask) + 1);
    }
}

void pci_enable_bus_mastering(uint8_t bus, uint8_t dev, uint8_t func) {
    uint16_t cmd = pci_read_config_word(bus, dev, func, PCI_REG_COMMAND);
    cmd |= 0x0004; /* Bit 2: Bus Master */
    pci_write_config_word(bus, dev, func, PCI_REG_COMMAND, cmd);
}

void pci_enable_io(uint8_t bus, uint8_t dev, uint8_t func) {
    uint16_t cmd = pci_read_config_word(bus, dev, func, PCI_REG_COMMAND);
    cmd |= 0x0001 | 0x0002; /* Bit 0: I/O Space, Bit 1: Memory Space */
    pci_write_config_word(bus, dev, func, PCI_REG_COMMAND, cmd);
}

void pci_print_devices(void) {
    kprintf("\n=== PCI Bus Device Inventory (%u devices detected) ===\n", pci_device_count);
    kprintf("  BDF      VENDOR:DEVICE  CLASS:SUBCLASS  IRQ  DESCRIPTION\n");
    kprintf("  -------------------------------------------------------------------------\n");
    for (uint32_t i = 0; i < pci_device_count; i++) {
        const pci_device_t *d = &pci_devices[i];
        kprintf("  %02x:%02x.%d  %04x:%04x       %02x:%02x          %-3u  %s (%s)\n",
                d->bus, d->dev, d->func,
                d->vendor_id, d->device_id,
                d->class_code, d->subclass,
                d->irq,
                pci_class_to_str(d->class_code, d->subclass),
                pci_vendor_to_str(d->vendor_id));
    }
    kprintf("  -------------------------------------------------------------------------\n\n");
}
