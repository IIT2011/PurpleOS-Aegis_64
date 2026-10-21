#ifndef ACPI_H
#define ACPI_H

#include "types.h"
#include "cpu.h"

#define ACPI_MAX_TABLES 32
#define ACPI_MAX_CPUS   MAX_CPUS
#define ACPI_MAX_IOAPIC 16
#define ACPI_MAX_ISO    32

#define ACPI_OK                 0
#define ACPI_ERR_NOT_FOUND     -1
#define ACPI_ERR_CHECKSUM      -2
#define ACPI_ERR_INVALID       -3
#define ACPI_ERR_OVERFLOW      -4
#define ACPI_ERR_TABLE_LIMIT   -5

/* Generic Address Structure (GAS) */
typedef struct {
    uint8_t  address_space; // 0 = System Memory, 1 = System I/O
    uint8_t  bit_width;
    uint8_t  bit_offset;
    uint8_t  access_size;
    uint64_t address;
} __attribute__((packed)) acpi_gas_t;

/* RSDP Descriptor v1.0 and v2.0 */
typedef struct {
    char     signature[8];  // "RSD PTR "
    uint8_t  checksum;
    char     oem_id[6];
    uint8_t  revision;      // 0 = ACPI 1.0, 2 = ACPI 2.0+
    uint32_t rsdt_address;
    /* ACPI 2.0+ fields */
    uint32_t length;
    uint64_t xsdt_address;
    uint8_t  extended_checksum;
    uint8_t  reserved[3];
} __attribute__((packed)) acpi_rsdp_t;

/* Standard SDT Header */
typedef struct {
    char     signature[4];
    uint32_t length;
    uint8_t  revision;
    uint8_t  checksum;
    char     oem_id[6];
    char     oem_table_id[8];
    uint32_t oem_revision;
    uint32_t creator_id;
    uint32_t creator_revision;
} __attribute__((packed)) acpi_sdt_header_t;

/* MADT Header */
typedef struct {
    acpi_sdt_header_t header;
    uint32_t lapic_addr;
    uint32_t flags; // bit 0: PC-AT Dual-8259 PIC present
} __attribute__((packed)) acpi_madt_t;

/* MADT Sub-Entry: Processor Local APIC (Type 0) */
typedef struct {
    uint8_t  type; // 0
    uint8_t  length; // 8
    uint8_t  acpi_processor_id;
    uint8_t  apic_id;
    uint32_t flags; // bit 0 = enabled, bit 1 = online capable
} __attribute__((packed)) acpi_madt_lapic_t;

/* MADT Sub-Entry: I/O APIC (Type 1) */
typedef struct {
    uint8_t  type; // 1
    uint8_t  length; // 12
    uint8_t  ioapic_id;
    uint8_t  reserved;
    uint32_t ioapic_addr;
    uint32_t gsi_base;
} __attribute__((packed)) acpi_madt_ioapic_t;

/* MADT Sub-Entry: Interrupt Source Override (Type 2) */
typedef struct {
    uint8_t  type; // 2
    uint8_t  length; // 10
    uint8_t  bus; // 0 = ISA
    uint8_t  source_irq;
    uint32_t gsi;
    uint16_t flags;
} __attribute__((packed)) acpi_madt_iso_t;

/* MADT Sub-Entry: Local APIC NMI (Type 4) */
typedef struct {
    uint8_t  type; // 4
    uint8_t  length; // 6
    uint8_t  acpi_processor_id; // 0xFF = all processors
    uint16_t flags;
    uint8_t  lint; // 0 or 1
} __attribute__((packed)) acpi_madt_nmi_t;

/* Parsed MADT Configuration */
typedef struct {
    uint64_t lapic_phys_addr;
    uint8_t  has_dual_pic;

    uint32_t cpu_count;
    struct {
        uint32_t apic_id;
        uint32_t acpi_id;
        uint32_t flags;
    } cpus[ACPI_MAX_CPUS];

    uint32_t ioapic_count;
    struct {
        uint32_t id;
        uint64_t phys_addr;
        uint32_t gsi_base;
    } ioapics[ACPI_MAX_IOAPIC];

    uint32_t iso_count;
    struct {
        uint8_t  bus;
        uint8_t  source_irq;
        uint32_t gsi;
        uint16_t flags;
    } isos[ACPI_MAX_ISO];
} acpi_madt_info_t;

/* Public ACPI Subsystem API */
int acpi_validate_checksum(const void *data, size_t length);
const acpi_rsdp_t *acpi_find_rsdp(void);
int acpi_init_from_rsdp(const acpi_rsdp_t *rsdp);
int acpi_init(void);

const acpi_sdt_header_t *acpi_find_table(const char *signature);
uint32_t acpi_get_table_count(void);
const acpi_sdt_header_t *acpi_get_table(uint32_t index);

int acpi_parse_madt(const acpi_madt_t *madt, acpi_madt_info_t *out_info);
const acpi_madt_info_t *acpi_get_madt_info(void);
void acpi_print_tables(void);

#endif /* ACPI_H */
