#include "usb.h"
#include "pci.h"
#include "string.h"
#include "serial.h"
#include "purple_detect.h"

#ifndef __KERNEL_HOST_TEST__
#include "vmm.h"
#endif

static usb_controller_t controllers[USB_MAX_CONTROLLERS];
static uint32_t controller_count = 0;

static const char *usb_type_to_str(usb_controller_type_t t) {
    switch (t) {
        case USB_TYPE_UHCI: return "UHCI (USB 1.1)";
        case USB_TYPE_OHCI: return "OHCI (USB 1.1)";
        case USB_TYPE_EHCI: return "EHCI (USB 2.0)";
        case USB_TYPE_XHCI: return "xHCI (USB 3.0)";
        default:            return "Unknown USB";
    }
}

int usb_init(void) {
    controller_count = 0;
    memset(controllers, 0, sizeof(controllers));

    uint32_t pci_count = pci_get_device_count();
    for (uint32_t i = 0; i < pci_count; i++) {
        const pci_device_t *pdev = pci_get_device(i);
        if (!pdev) continue;

        if (pdev->class_code == 0x0C && pdev->subclass == 0x03) {
            if (controller_count >= USB_MAX_CONTROLLERS) break;

            usb_controller_t *c = &controllers[controller_count++];
            c->id = controller_count;
            c->bus = pdev->bus;
            c->dev = pdev->dev;
            c->func = pdev->func;
            c->vendor_id = pdev->vendor_id;
            c->device_id = pdev->device_id;
            c->irq = pdev->irq;
            c->active = 1;

            pci_enable_bus_mastering(pdev->bus, pdev->dev, pdev->func);
            pci_enable_io(pdev->bus, pdev->dev, pdev->func);

            if (pdev->prog_if == 0x00) {
                c->type = USB_TYPE_UHCI;
                c->io_base = (uint16_t)pci_get_bar_address(pdev, 4);
                c->max_ports = 2;
                strncpy(c->name, "Intel UHCI Host", sizeof(c->name) - 1);
            } else if (pdev->prog_if == 0x10) {
                c->type = USB_TYPE_OHCI;
                c->mmio_base = pci_get_bar_address(pdev, 0);
                c->max_ports = 4;
                strncpy(c->name, "OHCI Host", sizeof(c->name) - 1);
            } else if (pdev->prog_if == 0x20) {
                c->type = USB_TYPE_EHCI;
                c->mmio_base = pci_get_bar_address(pdev, 0);
                c->max_ports = 6;
                strncpy(c->name, "EHCI USB 2.0 Host", sizeof(c->name) - 1);
            } else if (pdev->prog_if == 0x30) {
                c->type = USB_TYPE_XHCI;
                c->mmio_base = pci_get_bar_address(pdev, 0);
                c->max_ports = 8;
                c->max_slots = 32;
                strncpy(c->name, "xHCI USB 3.0 Host", sizeof(c->name) - 1);
            } else {
                c->type = USB_TYPE_UNKNOWN;
                strncpy(c->name, "Generic USB Host", sizeof(c->name) - 1);
            }

            kinfo("USB: Detected %s [%04x:%04x] at PCI %02x:%02x.%d (Ports: %u, MMIO=0x%lx, IO=0x%x)",
                  usb_type_to_str(c->type), c->vendor_id, c->device_id,
                  c->bus, c->dev, c->func, c->max_ports, c->mmio_base, c->io_base);

            purple_detect_register(PURPLE_CAT_BUS, c->name, pci_vendor_to_str(c->vendor_id),
                                   usb_type_to_str(c->type), "PCI",
                                   c->mmio_base ? c->mmio_base : (uint64_t)c->io_base,
                                   c->irq, PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
        }
    }

    if (controller_count == 0) {
        kinfo("USB: No PCI USB host controllers detected on current system");
    } else {
        kinfo("USB: %u USB Host Controller(s) registered and initialized", controller_count);
    }
    return 0;
}

uint32_t usb_get_controller_count(void) {
    return controller_count;
}

const usb_controller_t *usb_get_controller(uint32_t index) {
    if (index >= controller_count) return NULL;
    return &controllers[index];
}

void usb_dump_controllers(void) {
    kprintf("\n=== USB Host Controller Inventory (%u controller(s)) ===\n", controller_count);
    if (controller_count == 0) {
        kprintf("  No USB host controllers registered.\n\n");
        return;
    }

    kprintf("  ID  TYPE           NAME                   PCI BDF   PORTS  SLOTS  BASE_ADDR\n");
    kprintf("  ----------------------------------------------------------------------------\n");
    for (uint32_t i = 0; i < controller_count; i++) {
        const usb_controller_t *c = &controllers[i];
        kprintf("  %-2u  %-13s  %-21s  %02x:%02x.%-2d  %-5u  %-5u  %s=0x%lx\n",
                c->id, usb_type_to_str(c->type), c->name,
                c->bus, c->dev, c->func, c->max_ports, c->max_slots,
                c->mmio_base ? "MMIO" : "IO",
                c->mmio_base ? c->mmio_base : (uint64_t)c->io_base);
    }
    kprintf("\n");
}
