#ifndef APDRV_H
#define APDRV_H

#include "types.h"
#include "ksym.h"

#define APDRV_MAGIC           "APDRV100"
#define APDRV_MAGIC_LEN       8
#define APDRV_VERSION         3
#define APDRV_TYPE_AEGISMOD   "AEGISMOD"

#define APDRV_MAX_NAME        32
#define APDRV_MAX_DESC        64
#define APDRV_MAX_AUTHOR      32
#define APDRV_MAX_MODULES     16
#define APDRV_MAX_RELOCS      256
#define APDRV_MAX_SYMBOLS     64

/* Relocation Types */
#define APDRV_RELOC_NONE      0
#define APDRV_RELOC_64        1   /* R_X86_64_64: *(uint64_t*)target = sym + addend */
#define APDRV_RELOC_PC32      2   /* R_X86_64_PC32: *(int32_t*)target = sym + addend - target */
#define APDRV_RELOC_PLT32     4   /* R_X86_64_PLT32: *(int32_t*)target = sym + addend - target */
#define APDRV_RELOC_32        10  /* R_X86_64_32: *(uint32_t*)target = (uint32_t)(sym + addend) */

/* Relocation Section Base Indices */
#define APDRV_SEC_TEXT        0xFFF0
#define APDRV_SEC_RODATA      0xFFF1
#define APDRV_SEC_DATA        0xFFF2
#define APDRV_SEC_BSS         0xFFF3
#define APDRV_SEC_MODULE      0xFFF4

/* Flags */
#define APDRV_FLAG_ENFORCE_WX 0x0001
#define APDRV_FLAG_AUTO_PROBE 0x0002

typedef struct {
    uint32_t offset;     /* Offset from start of module memory image */
    uint32_t type;       /* APDRV_RELOC_* */
    uint32_t sym_index;  /* Index into import table OR APDRV_SEC_* */
    int64_t  addend;     /* Addend */
} __attribute__((packed)) apdrv_reloc_t;

typedef struct {
    char     name[APDRV_MAX_NAME];
    uint64_t address;    /* Resolved address or offset */
} __attribute__((packed)) apdrv_sym_t;

/* On-disk APDRV v3 Header */
typedef struct {
    uint8_t  magic[8];          /* "APDRV100" */
    uint32_t version;           /* 3 */
    uint32_t header_size;       /* sizeof(apdrv_header_t) */
    uint8_t  mod_type[8];       /* "AEGISMOD" */
    char     name[APDRV_MAX_NAME];
    char     description[APDRV_MAX_DESC];
    char     author[APDRV_MAX_AUTHOR];
    uint32_t mod_version;       /* e.g. 0x010000 = 1.0.0 */

    /* Section Offsets in file and Sizes in bytes */
    uint32_t text_offset;
    uint32_t text_size;
    uint32_t rodata_offset;
    uint32_t rodata_size;
    uint32_t data_offset;
    uint32_t data_size;
    uint32_t bss_size;

    /* Relocation Table */
    uint32_t reloc_offset;
    uint32_t reloc_count;

    /* Symbol Tables */
    uint32_t import_offset;
    uint32_t import_count;
    uint32_t export_offset;
    uint32_t export_count;

    /* Entry Point Offsets relative to module base */
    uint32_t entry_init;        /* int (*init)(void) */
    uint32_t entry_fini;        /* int (*fini)(void) */
    uint32_t entry_probe;       /* int (*probe)(uint32_t vendor, uint32_t device) */
    uint32_t entry_ioctl;       /* int (*ioctl)(uint32_t cmd, void *arg) */

    /* Hardware PCI Matcher (0xFFFF = wildcard) */
    uint16_t target_vendor;
    uint16_t target_device;
    uint32_t flags;             /* APDRV_FLAG_* */
    uint32_t checksum;          /* Simple CRC32 / Fletcher checksum of payload */
} __attribute__((packed)) apdrv_header_t;

/* Module State */
typedef enum {
    APDRV_STATE_UNLOADED = 0,
    APDRV_STATE_LOADED   = 1,
    APDRV_STATE_RUNNING  = 2,
    APDRV_STATE_ERROR    = 3
} apdrv_state_t;

/* Loaded Module Runtime Instance */
typedef struct apdrv_module {
    apdrv_header_t hdr;
    apdrv_state_t  state;
    void          *memory;         /* Base pointer of allocated module memory */
    uint32_t       total_size;     /* Total allocated pages / bytes */
    uint64_t       text_addr;
    uint64_t       rodata_addr;
    uint64_t       data_addr;
    uint64_t       bss_addr;
    int          (*fn_init)(void);
    int          (*fn_fini)(void);
    int          (*fn_probe)(uint32_t vendor, uint32_t device);
    int          (*fn_ioctl)(uint32_t cmd, void *arg);
    uint32_t       bound_devices;  /* Count of hardware devices bound */
    int            wx_enforced;    /* 1 if W^X permissions were set */
} apdrv_module_t;

/* Subsystem API */
int  apdrv_subsystem_init(void);
int  apdrv_load(const void *image, uint32_t image_size, apdrv_module_t **out_mod);
int  apdrv_unload(const char *name);
apdrv_module_t *apdrv_find(const char *name);
uint32_t apdrv_get_count(void);
const apdrv_module_t *apdrv_get(uint32_t index);
int  apdrv_probe_device(uint16_t vendor_id, uint16_t device_id);
int  apdrv_load_from_storage(const char *filepath);
void apdrv_dump_modules(void);

#endif /* APDRV_H */
