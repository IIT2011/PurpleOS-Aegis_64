#include "ksym.h"
#include "string.h"
#include "serial.h"
#include "pci.h"
#include "netdev.h"
#include "purple_detect.h"
#include "pmm.h"
#include "io.h"
#include "task.h"
#include "desktop.h"

static ksym_entry_t ksym_table[KSYM_MAX_SYMBOLS];
static uint32_t ksym_count = 0;

static uint8_t ksym_inb_wrap(uint16_t port) { return inb(port); }
static void ksym_outb_wrap(uint16_t port, uint8_t val) { outb(port, val); }
static uint16_t ksym_inw_wrap(uint16_t port) { return inw(port); }
static void ksym_outw_wrap(uint16_t port, uint16_t val) { outw(port, val); }
static uint32_t ksym_inl_wrap(uint16_t port) { return inl(port); }
static void ksym_outl_wrap(uint16_t port, uint32_t val) { outl(port, val); }

int ksym_register(const char *name, uint64_t address) {
    if (!name || address == 0) return -1;
    if (ksym_count >= KSYM_MAX_SYMBOLS) return -2;

    for (uint32_t i = 0; i < ksym_count; i++) {
        if (strcmp(ksym_table[i].name, name) == 0) {
            ksym_table[i].address = address; /* Update existing */
            return 0;
        }
    }

    strncpy(ksym_table[ksym_count].name, name, KSYM_NAME_MAX - 1);
    ksym_table[ksym_count].name[KSYM_NAME_MAX - 1] = '\0';
    ksym_table[ksym_count].address = address;
    ksym_count++;
    return 0;
}

int ksym_unregister(const char *name) {
    if (!name) return -1;
    int idx = -1;
    for (uint32_t i = 0; i < ksym_count; i++) {
        if (strcmp(ksym_table[i].name, name) == 0) {
            idx = (int)i;
            break;
        }
    }
    if (idx < 0) return -2;

    for (uint32_t i = idx; i + 1 < ksym_count; i++) {
        ksym_table[i] = ksym_table[i + 1];
    }
    ksym_count--;
    return 0;
}

uint64_t ksym_lookup(const char *name) {
    if (!name) return 0;
    for (uint32_t i = 0; i < ksym_count; i++) {
        if (strcmp(ksym_table[i].name, name) == 0) {
            return ksym_table[i].address;
        }
    }
    return 0;
}

uint32_t ksym_get_count(void) {
    return ksym_count;
}

const ksym_entry_t *ksym_get_entry(uint32_t index) {
    if (index >= ksym_count) return NULL;
    return &ksym_table[index];
}

void ksym_dump(const char *filter) {
    kprintf("\n=== Aegis64 Kernel Symbol Table (%u symbols) ===\n", ksym_count);
    kprintf("  ADDRESS             SYMBOL NAME\n");
    kprintf("  --------------------------------------------------\n");
    uint32_t matched = 0;
    for (uint32_t i = 0; i < ksym_count; i++) {
        if (!filter || strlen(filter) == 0 || strstr(ksym_table[i].name, filter)) {
            kprintf("  0x%016llx  %s\n", ksym_table[i].address, ksym_table[i].name);
            matched++;
        }
    }
    kprintf("  --------------------------------------------------\n");
    kprintf("  Total matched: %u\n\n", matched);
}

void ksym_init(void) {
    ksym_count = 0;
    memset(ksym_table, 0, sizeof(ksym_table));

    /* 1. Core Logging (Specified in APDRV v3 spec) */
    ksym_register("kinfo", (uint64_t)(uintptr_t)kinfo);
    ksym_register("kerror", (uint64_t)(uintptr_t)kerror);
    ksym_register("kprintf", (uint64_t)(uintptr_t)kprintf);
    ksym_register("kwarn", (uint64_t)(uintptr_t)kwarn);

    /* 2. Memory / String Primitives */
    ksym_register("memcpy", (uint64_t)(uintptr_t)memcpy);
    ksym_register("memset", (uint64_t)(uintptr_t)memset);
    ksym_register("memcmp", (uint64_t)(uintptr_t)memcmp);
    ksym_register("strlen", (uint64_t)(uintptr_t)strlen);
    ksym_register("strcpy", (uint64_t)(uintptr_t)strcpy);
    ksym_register("strcmp", (uint64_t)(uintptr_t)strcmp);
    ksym_register("strncpy", (uint64_t)(uintptr_t)strncpy);

    /* 3. PCI Configuration & Bus Mastering */
    ksym_register("pci_enable_bus_mastering", (uint64_t)(uintptr_t)pci_enable_bus_mastering);
    ksym_register("pci_enable_io", (uint64_t)(uintptr_t)pci_enable_io);
    ksym_register("pci_read_config_dword", (uint64_t)(uintptr_t)pci_read_config_dword);
    ksym_register("pci_write_config_dword", (uint64_t)(uintptr_t)pci_write_config_dword);
    ksym_register("pci_find_device", (uint64_t)(uintptr_t)pci_find_device);
    ksym_register("pci_get_device", (uint64_t)(uintptr_t)pci_get_device);
    ksym_register("pci_get_device_count", (uint64_t)(uintptr_t)pci_get_device_count);

    /* 4. Network Device Interface */
    ksym_register("netdev_register", (uint64_t)(uintptr_t)netdev_register);
    ksym_register("netdev_unregister", (uint64_t)(uintptr_t)netdev_unregister);
    ksym_register("netdev_get", (uint64_t)(uintptr_t)netdev_get);
    ksym_register("netdev_find_by_name", (uint64_t)(uintptr_t)netdev_find_by_name);

    /* 5. Device Discovery & Lifecycle (PurpleDetect) */
    ksym_register("purple_detect_register", (uint64_t)(uintptr_t)purple_detect_register);
    ksym_register("purple_detect_find_by_name", (uint64_t)(uintptr_t)purple_detect_find_by_name);
    ksym_register("purple_detect_update_state", (uint64_t)(uintptr_t)purple_detect_update_state);

    /* 6. Physical & Virtual Memory Allocation */
    ksym_register("pmm_alloc_frame", (uint64_t)(uintptr_t)pmm_alloc_frame);
    ksym_register("pmm_free_frame", (uint64_t)(uintptr_t)pmm_free_frame);

    /* 7. Task Management */
    ksym_register("task_create", (uint64_t)(uintptr_t)task_create);
    ksym_register("task_sleep", (uint64_t)(uintptr_t)task_sleep);
    ksym_register("task_yield", (uint64_t)(uintptr_t)task_yield);

    /* 8. Desktop GUI & Window Subsystem */
    ksym_register("desktop_create_window", (uint64_t)(uintptr_t)desktop_create_window);
    ksym_register("desktop_render", (uint64_t)(uintptr_t)desktop_render);

    /* 9. I/O Port Access */
    ksym_register("inb", (uint64_t)(uintptr_t)ksym_inb_wrap);
    ksym_register("outb", (uint64_t)(uintptr_t)ksym_outb_wrap);
    ksym_register("inw", (uint64_t)(uintptr_t)ksym_inw_wrap);
    ksym_register("outw", (uint64_t)(uintptr_t)ksym_outw_wrap);
    ksym_register("inl", (uint64_t)(uintptr_t)ksym_inl_wrap);
    ksym_register("outl", (uint64_t)(uintptr_t)ksym_outl_wrap);

    kinfo("KSYM: Kernel Symbol Table initialized with %u exported symbols", ksym_count);
}
