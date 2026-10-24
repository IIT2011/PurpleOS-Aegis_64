#ifndef USB_H
#define USB_H

#include "types.h"
#include "pci.h"

#define USB_MAX_CONTROLLERS 8

typedef enum {
    USB_TYPE_UNKNOWN = 0,
    USB_TYPE_UHCI    = 1,
    USB_TYPE_OHCI    = 2,
    USB_TYPE_EHCI    = 3,
    USB_TYPE_XHCI    = 4
} usb_controller_type_t;

typedef struct {
    uint32_t id;
    usb_controller_type_t type;
    char name[32];
    uint8_t bus;
    uint8_t dev;
    uint8_t func;
    uint16_t vendor_id;
    uint16_t device_id;
    uint64_t mmio_base;
    uint16_t io_base;
    uint32_t irq;
    uint32_t max_ports;
    uint32_t max_slots; /* xHCI */
    int active;
} usb_controller_t;

int usb_init(void);
uint32_t usb_get_controller_count(void);
const usb_controller_t *usb_get_controller(uint32_t index);
void usb_dump_controllers(void);

#endif /* USB_H */
