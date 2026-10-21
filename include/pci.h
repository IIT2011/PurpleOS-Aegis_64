#ifndef PCI_H
#define PCI_H

#include "types.h"

#define PCI_CONFIG_ADDRESS 0xCF8
#define PCI_CONFIG_DATA    0xCFC

/* PCI Configuration Registers Offsets */
#define PCI_REG_VENDOR_ID       0x00
#define PCI_REG_DEVICE_ID       0x02
#define PCI_REG_COMMAND         0x04
#define PCI_REG_STATUS          0x06
#define PCI_REG_REVISION_ID     0x08
#define PCI_REG_PROG_IF         0x09
#define PCI_REG_SUBCLASS        0x0A
#define PCI_REG_CLASS           0x0B
#define PCI_REG_CACHE_LINE_SIZE 0x0C
#define PCI_REG_LATENCY_TIMER   0x0D
#define PCI_REG_HEADER_TYPE     0x0E
#define PCI_REG_BIST            0x0F
#define PCI_REG_BAR0            0x10
#define PCI_REG_BAR1            0x14
#define PCI_REG_BAR2            0x18
#define PCI_REG_BAR3            0x1C
#define PCI_REG_BAR4            0x20
#define PCI_REG_BAR5            0x24
#define PCI_REG_INTERRUPT_LINE  0x3C
#define PCI_REG_INTERRUPT_PIN   0x3D

#define PCI_NONE 0xFFFF

typedef struct {
    uint8_t  bus;
    uint8_t  dev;
    uint8_t  func;
    uint16_t vendor_id;
    uint16_t device_id;
    uint8_t  class_code;
    uint8_t  subclass;
    uint8_t  prog_if;
    uint8_t  revision;
    uint8_t  irq;
    uint32_t bar[6];
} pci_device_t;

#define PCI_MAX_DEVICES 32

uint32_t pci_read_config_dword(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset);
uint16_t pci_read_config_word(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset);
uint8_t  pci_read_config_byte(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset);
void     pci_write_config_dword(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset, uint32_t val);
void     pci_write_config_word(uint8_t bus, uint8_t dev, uint8_t func, uint8_t offset, uint16_t val);

int  pci_init(void);
void pci_scan(void);
uint32_t pci_get_device_count(void);
const pci_device_t *pci_get_device(uint32_t index);
const pci_device_t *pci_find_device(uint16_t vendor_id, uint16_t device_id);
void pci_enable_bus_mastering(uint8_t bus, uint8_t dev, uint8_t func);
void pci_enable_io(uint8_t bus, uint8_t dev, uint8_t func);
const char *pci_class_to_str(uint8_t class_code, uint8_t subclass);
const char *pci_vendor_to_str(uint16_t vendor_id);
void pci_print_devices(void);

#endif /* PCI_H */
