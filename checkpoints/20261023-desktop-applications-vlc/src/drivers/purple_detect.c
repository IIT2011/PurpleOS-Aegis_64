#include "purple_detect.h"
#include "spinlock.h"
#include "string.h"
#include "serial.h"
#include "pci.h"

#ifndef __KERNEL_HOST_TEST__
#include "pit.h"
#include "apic.h"
#include "ata_pio.h"
#include "kernel_storage.h"
#include "task.h"
#endif

static purple_device_t devices[PURPLE_MAX_DEVICES];
static uint32_t device_count = 0;
static uint32_t next_device_id = 1;
static spinlock_t detect_lock;
static purple_detect_event_fn event_handler = NULL;

const char *purple_category_to_str(purple_dev_category_t cat) {
    switch (cat) {
        case PURPLE_CAT_STORAGE:   return "STORAGE";
        case PURPLE_CAT_INPUT:     return "INPUT";
        case PURPLE_CAT_DISPLAY:   return "DISPLAY";
        case PURPLE_CAT_AUDIO:     return "AUDIO";
        case PURPLE_CAT_NETWORK:   return "NETWORK";
        case PURPLE_CAT_SERIAL:    return "SERIAL";
        case PURPLE_CAT_TIMER:     return "TIMER";
        case PURPLE_CAT_INTERRUPT: return "INTERRUPT";
        case PURPLE_CAT_BRIDGE:    return "BRIDGE";
        case PURPLE_CAT_BUS:       return "BUS";
        case PURPLE_CAT_VIRTUAL:   return "VIRTUAL";
        default:                   return "UNKNOWN";
    }
}

const char *purple_state_to_str(purple_dev_state_t state) {
    switch (state) {
        case PURPLE_STATE_DISCOVERED:  return "DISCOVERED";
        case PURPLE_STATE_ENUMERATING: return "ENUMERATING";
        case PURPLE_STATE_SUPPORTED:   return "SUPPORTED";
        case PURPLE_STATE_UNSUPPORTED: return "UNSUPPORTED";
        case PURPLE_STATE_TRUSTED:     return "TRUSTED";
        case PURPLE_STATE_RESTRICTED:  return "RESTRICTED";
        case PURPLE_STATE_ACTIVE:      return "ACTIVE";
        case PURPLE_STATE_REMOVED:     return "REMOVED";
        case PURPLE_STATE_FAILED:      return "FAILED";
        default:                       return "UNKNOWN";
    }
}

int purple_detect_init(void) {
    spinlock_init(&detect_lock);
    device_count = 0;
    next_device_id = 1;
    event_handler = NULL;
    memset(devices, 0, sizeof(devices));
    kinfo("PurpleDetect: Hardware and Device Lifecycle Subsystem Initialized");
    return 0;
}

int purple_detect_set_event_handler(purple_detect_event_fn handler) {
    uint64_t flags = 0;
    spinlock_acquire_irqsave(&detect_lock, &flags);
    event_handler = handler;
    spinlock_release_irqrestore(&detect_lock, flags);
    return 0;
}

int purple_detect_register(purple_dev_category_t cat,
                           const char *name,
                           const char *vendor,
                           const char *model,
                           const char *bus,
                           uint64_t addr,
                           uint32_t irq,
                           purple_dev_state_t state,
                           uint32_t flags) {
    uint64_t irq_flags = 0;
    spinlock_acquire_irqsave(&detect_lock, &irq_flags);

    if (device_count >= PURPLE_MAX_DEVICES) {
        spinlock_release_irqrestore(&detect_lock, irq_flags);
        kwarn("PurpleDetect: Device table full (%u devices)", PURPLE_MAX_DEVICES);
        return -1;
    }

    uint32_t slot = device_count;
    purple_device_t *dev = &devices[slot];
    memset(dev, 0, sizeof(purple_device_t));

    dev->id = next_device_id++;
    dev->category = cat;
    dev->state = state;
    dev->flags = flags;
    dev->address = addr;
    dev->irq = irq;

    if (name) {
        strncpy(dev->name, name, PURPLE_MAX_NAME_LEN - 1);
        dev->name[PURPLE_MAX_NAME_LEN - 1] = '\0';
    }
    if (vendor) {
        strncpy(dev->vendor, vendor, PURPLE_MAX_STR_LEN - 1);
        dev->vendor[PURPLE_MAX_STR_LEN - 1] = '\0';
    }
    if (model) {
        strncpy(dev->model, model, PURPLE_MAX_STR_LEN - 1);
        dev->model[PURPLE_MAX_STR_LEN - 1] = '\0';
    }
    if (bus) {
        strncpy(dev->bus, bus, PURPLE_MAX_STR_LEN - 1);
        dev->bus[PURPLE_MAX_STR_LEN - 1] = '\0';
    }

#ifndef __KERNEL_HOST_TEST__
    dev->last_seen_tick = pit_get_ticks();
#else
    dev->last_seen_tick = 0;
#endif

    device_count++;
    uint32_t assigned_id = dev->id;

    purple_detect_event_fn cb = event_handler;
    spinlock_release_irqrestore(&detect_lock, irq_flags);

    if (cb) {
        cb(dev, PURPLE_EVENT_DEVICE_ADDED);
    }

    kinfo("PurpleDetect: Registered [%s] '%s' (%s %s) on %s at 0x%x (State=%s)",
          purple_category_to_str(cat), dev->name, dev->vendor, dev->model,
          dev->bus, (uint32_t)dev->address, purple_state_to_str(state));

    return (int)assigned_id;
}

int purple_detect_unregister(uint32_t id) {
    uint64_t irq_flags = 0;
    spinlock_acquire_irqsave(&detect_lock, &irq_flags);

    for (uint32_t i = 0; i < device_count; i++) {
        if (devices[i].id == id) {
            devices[i].state = PURPLE_STATE_REMOVED;
            purple_detect_event_fn cb = event_handler;
            purple_device_t copy = devices[i];
            spinlock_release_irqrestore(&detect_lock, irq_flags);

            if (cb) {
                cb(&copy, PURPLE_EVENT_DEVICE_REMOVED);
            }
            kinfo("PurpleDetect: Device ID %u ('%s') removed", id, copy.name);
            return 0;
        }
    }

    spinlock_release_irqrestore(&detect_lock, irq_flags);
    return -1;
}

int purple_detect_update_state(uint32_t id, purple_dev_state_t new_state) {
    uint64_t irq_flags = 0;
    spinlock_acquire_irqsave(&detect_lock, &irq_flags);

    for (uint32_t i = 0; i < device_count; i++) {
        if (devices[i].id == id) {
            devices[i].state = new_state;
#ifndef __KERNEL_HOST_TEST__
            devices[i].last_seen_tick = pit_get_ticks();
#endif
            purple_detect_event_fn cb = event_handler;
            purple_device_t copy = devices[i];
            spinlock_release_irqrestore(&detect_lock, irq_flags);

            if (cb) {
                cb(&copy, PURPLE_EVENT_STATE_CHANGED);
            }
            return 0;
        }
    }

    spinlock_release_irqrestore(&detect_lock, irq_flags);
    return -1;
}

const purple_device_t *purple_detect_get(uint32_t id) {
    for (uint32_t i = 0; i < device_count; i++) {
        if (devices[i].id == id) {
            return &devices[i];
        }
    }
    return NULL;
}

uint32_t purple_detect_count(void) {
    return device_count;
}

const purple_device_t *purple_detect_find_by_name(const char *name) {
    if (!name) return NULL;
    for (uint32_t i = 0; i < device_count; i++) {
        if (strcmp(devices[i].name, name) == 0) {
            return &devices[i];
        }
    }
    return NULL;
}

const purple_device_t *purple_detect_find_by_category(purple_dev_category_t cat, uint32_t index) {
    uint32_t matches = 0;
    for (uint32_t i = 0; i < device_count; i++) {
        if (devices[i].category == cat) {
            if (matches == index) {
                return &devices[i];
            }
            matches++;
        }
    }
    return NULL;
}

int purple_detect_scan_all(void) {
#ifndef __KERNEL_HOST_TEST__
    kinfo("PurpleDetect: Beginning system platform and bus discovery...");

    /* Register Motherboard Core Peripherals */
    purple_detect_register(PURPLE_CAT_SERIAL, "COM1", "Standard", "16550A UART", "ISA", 0x3F8, 4,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);

    purple_detect_register(PURPLE_CAT_TIMER, "PIT", "Intel", "8254 Timer", "ISA", 0x40, 0,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN | PURPLE_DEV_FLAG_CRITICAL);

    purple_detect_register(PURPLE_CAT_INPUT, "PS2KBD", "IBM", "PS/2 Keyboard", "ISA", 0x60, 1,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);

    if (apic_is_enabled()) {
        purple_detect_register(PURPLE_CAT_INTERRUPT, "LAPIC", "Intel/AMD", "Local APIC", "MEMORY", 0xFEE00000, 0,
                               PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN | PURPLE_DEV_FLAG_CRITICAL);
    }

    purple_detect_register(PURPLE_CAT_STORAGE, "ATA_PRI", "Standard", "IDE Primary Controller", "ISA", 0x1F0, 14,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);

    purple_detect_register(PURPLE_CAT_STORAGE, "ATA_SEC", "Standard", "IDE Secondary Controller", "ISA", 0x170, 15,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);

    /* Register Discovered ATA Drives */
    for (int d = 0; d < MAX_ATA_DEVICES; d++) {
        if (g_kernel_storage.devices[d].is_present) {
            char dname[32];
            dname[0] = 'a'; dname[1] = 't'; dname[2] = 'a'; dname[3] = '0' + (char)d; dname[4] = '\0';
            purple_detect_register(PURPLE_CAT_STORAGE, dname, "ATA/IDE", g_kernel_storage.devices[d].model, "ATA",
                                   g_kernel_storage.devices[d].total_sectors, 14, PURPLE_STATE_ACTIVE,
                                   PURPLE_DEV_FLAG_REMOVABLE);
        }
    }

    /* Scan PCI Bus and Register Discovered PCI Devices */
    pci_scan();
    uint32_t pci_count = pci_get_device_count();
    for (uint32_t p = 0; p < pci_count; p++) {
        const pci_device_t *pdev = pci_get_device(p);
        if (!pdev) continue;

        purple_dev_category_t pcat = PURPLE_CAT_UNKNOWN;
        switch (pdev->class_code) {
            case 0x01: pcat = PURPLE_CAT_STORAGE; break;
            case 0x02: pcat = PURPLE_CAT_NETWORK; break;
            case 0x03: pcat = PURPLE_CAT_DISPLAY; break;
            case 0x04: pcat = PURPLE_CAT_AUDIO;   break;
            case 0x06: pcat = PURPLE_CAT_BRIDGE;  break;
            case 0x0C: pcat = PURPLE_CAT_BUS;    break;
            default:   pcat = PURPLE_CAT_UNKNOWN; break;
        }

        char pname[32];
        pname[0] = 'p'; pname[1] = 'c'; pname[2] = 'i'; pname[3] = '_';
        pname[4] = '0' + (char)pdev->bus; pname[5] = '_';
        pname[6] = (pdev->dev < 10) ? ('0' + (char)pdev->dev) : ('a' + (char)(pdev->dev - 10));
        pname[7] = '\0';

        uint64_t bdf_addr = ((uint64_t)pdev->bus << 16) | ((uint64_t)pdev->dev << 11) | ((uint64_t)pdev->func << 8);

        purple_dev_state_t dstate = PURPLE_STATE_ACTIVE;
        if (pcat == PURPLE_CAT_STORAGE || pcat == PURPLE_CAT_BRIDGE || pcat == PURPLE_CAT_DISPLAY) {
            dstate = PURPLE_STATE_ACTIVE;
        } else {
            dstate = PURPLE_STATE_SUPPORTED;
        }

        purple_detect_register(pcat, pname, pci_vendor_to_str(pdev->vendor_id),
                               pci_class_to_str(pdev->class_code, pdev->subclass),
                               "PCI", bdf_addr, pdev->irq, dstate,
                               PURPLE_DEV_FLAG_BUILTIN);
    }
#endif
    return (int)device_count;
}

void purple_detect_daemon(void *arg) {
    (void)arg;
#ifndef __KERNEL_HOST_TEST__
    task_t *cur = task_get_current();
    kinfo("PurpleDetect: Background device monitoring daemon online (PID %u)", cur ? cur->id : 0);
    while (1) {
        task_sleep(200); // Check every 2 seconds
        // Update presence and heartbeats
        uint64_t now = pit_get_ticks();
        for (uint32_t i = 0; i < device_count; i++) {
            if (devices[i].state == PURPLE_STATE_ACTIVE) {
                devices[i].last_seen_tick = now;
            }
        }
    }
#endif
}

void purple_detect_print_table(void) {
    kprintf("\n=== PurpleDetect Hardware & Removable Device Registry ===\n");
    kprintf("  Discovered Devices: %u\n\n", device_count);
    kprintf("  ID  CATEGORY    STATE       BUS      ADDRESS     IRQ  NAME         VENDOR / MODEL\n");
    kprintf("  ----------------------------------------------------------------------------------------\n");
    for (uint32_t i = 0; i < device_count; i++) {
        const purple_device_t *d = &devices[i];
        kprintf("  %-2u  %-10s  %-10s  %-7s  0x%-8x  %-3u  %-11s  %s - %s\n",
                d->id,
                purple_category_to_str(d->category),
                purple_state_to_str(d->state),
                d->bus,
                (uint32_t)d->address,
                d->irq,
                d->name,
                d->vendor,
                d->model);
    }
    kprintf("  ----------------------------------------------------------------------------------------\n\n");
}
