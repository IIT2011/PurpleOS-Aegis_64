#ifndef PURPLE_DETECT_H
#define PURPLE_DETECT_H

#include "types.h"

#define PURPLE_MAX_DEVICES   64
#define PURPLE_MAX_NAME_LEN  64
#define PURPLE_MAX_STR_LEN   32

/* Device Categories */
typedef enum {
    PURPLE_CAT_UNKNOWN     = 0,
    PURPLE_CAT_STORAGE     = 1,   /* Disks, ATA, AHCI, NVMe, USB mass storage */
    PURPLE_CAT_INPUT       = 2,   /* Keyboard, Mouse, Gamepad */
    PURPLE_CAT_DISPLAY     = 3,   /* VGA, VBE, GOP framebuffer */
    PURPLE_CAT_AUDIO       = 4,   /* AC97, Intel HDA, Sound Blaster */
    PURPLE_CAT_NETWORK     = 5,   /* Ethernet, Wi-Fi */
    PURPLE_CAT_SERIAL      = 6,   /* UART COM ports */
    PURPLE_CAT_TIMER       = 7,   /* PIT, LAPIC Timer, HPET, RTC */
    PURPLE_CAT_INTERRUPT   = 8,   /* PIC, IOAPIC, LAPIC */
    PURPLE_CAT_BRIDGE      = 9,   /* Host Bridge, ISA Bridge, PCI Bridge */
    PURPLE_CAT_BUS         = 10,  /* PCI, USB, SMBus Host */
    PURPLE_CAT_VIRTUAL     = 11   /* Ramdisk, loopback, synthetic */
} purple_dev_category_t;

/* Device States */
typedef enum {
    PURPLE_STATE_DISCOVERED   = 0,  /* Hardware detected but not yet initialized */
    PURPLE_STATE_ENUMERATING  = 1,  /* Probing device capabilities / child nodes */
    PURPLE_STATE_SUPPORTED    = 2,  /* Driver available for device */
    PURPLE_STATE_UNSUPPORTED  = 3,  /* Known device without active driver */
    PURPLE_STATE_TRUSTED      = 4,  /* Verified against AegisDetect baseline */
    PURPLE_STATE_RESTRICTED   = 5,  /* Device restricted by policy/quarantine */
    PURPLE_STATE_ACTIVE       = 6,  /* Online and serving I/O requests */
    PURPLE_STATE_REMOVED      = 7,  /* Physically or logically unplugged */
    PURPLE_STATE_FAILED       = 8   /* I/O timeout, fault, or hardware error */
} purple_dev_state_t;

/* Device Flags */
#define PURPLE_DEV_FLAG_BUILTIN     (1 << 0)  /* Non-removable motherboard device */
#define PURPLE_DEV_FLAG_REMOVABLE   (1 << 1)  /* Hot-pluggable (USB, external, etc.) */
#define PURPLE_DEV_FLAG_CRITICAL    (1 << 2)  /* System fails if missing/failed */
#define PURPLE_DEV_FLAG_DMA_CAPABLE (1 << 3)  /* Supports Direct Memory Access */
#define PURPLE_DEV_FLAG_POWER_ON    (1 << 4)  /* Powered on */

/* Event Types */
#define PURPLE_EVENT_DEVICE_ADDED    1
#define PURPLE_EVENT_DEVICE_REMOVED  2
#define PURPLE_EVENT_STATE_CHANGED   3

typedef struct purple_device {
    uint32_t              id;
    purple_dev_category_t category;
    purple_dev_state_t    state;
    uint32_t              flags;
    char                  name[PURPLE_MAX_NAME_LEN];
    char                  vendor[PURPLE_MAX_STR_LEN];
    char                  model[PURPLE_MAX_STR_LEN];
    char                  bus[PURPLE_MAX_STR_LEN];  /* "ISA", "PCI", "ATA", "MEMORY", "PLATFORM" */
    uint64_t              address;                  /* Port, MMIO, or (bus<<16|dev<<11|func<<8) */
    uint32_t              irq;
    uint64_t              last_seen_tick;
} purple_device_t;

typedef void (*purple_detect_event_fn)(const purple_device_t *dev, uint32_t event);

int purple_detect_init(void);
int purple_detect_register(purple_dev_category_t cat,
                           const char *name,
                           const char *vendor,
                           const char *model,
                           const char *bus,
                           uint64_t addr,
                           uint32_t irq,
                           purple_dev_state_t state,
                           uint32_t flags);
int purple_detect_unregister(uint32_t id);
int purple_detect_update_state(uint32_t id, purple_dev_state_t new_state);
const purple_device_t *purple_detect_get(uint32_t id);
uint32_t purple_detect_count(void);
const purple_device_t *purple_detect_find_by_name(const char *name);
const purple_device_t *purple_detect_find_by_category(purple_dev_category_t cat, uint32_t index);
int purple_detect_set_event_handler(purple_detect_event_fn handler);

/* Device discovery triggers */
int purple_detect_scan_all(void);
void purple_detect_daemon(void *arg);

/* Diagnostics / Status display */
const char *purple_category_to_str(purple_dev_category_t cat);
const char *purple_state_to_str(purple_dev_state_t state);
void purple_detect_print_table(void);

#endif /* PURPLE_DETECT_H */
