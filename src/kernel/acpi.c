#include "acpi.h"
#include "multiboot.h"
#include "string.h"
#include "serial.h"

static const acpi_sdt_header_t *registered_tables[ACPI_MAX_TABLES];
static uint32_t registered_table_count = 0;
static acpi_madt_info_t parsed_madt;
static int madt_is_parsed = 0;

int acpi_validate_checksum(const void *data, size_t length) {
    if (!data || length == 0) return 0;
    const uint8_t *bytes = (const uint8_t *)data;
    uint8_t sum = 0;
    for (size_t i = 0; i < length; i++) {
        sum += bytes[i];
    }
    return (sum == 0) ? 1 : 0;
}

const acpi_rsdp_t *acpi_find_rsdp(void) {
#ifdef __KERNEL_HOST_TEST__
    return NULL;
#else
    /* 1. Search Extended BIOS Data Area (EBDA) */
    uint16_t ebda_seg = 0;
    __asm__ volatile ("movw 0x40E, %0" : "=r"(ebda_seg));
    if (ebda_seg != 0) {
        uintptr_t ebda_base = (uintptr_t)ebda_seg << 4;
        if (ebda_base >= 0x80000 && ebda_base < 0xA0000) {
            for (uintptr_t addr = ebda_base; addr < ebda_base + 1024; addr += 16) {
                const acpi_rsdp_t *candidate = (const acpi_rsdp_t *)addr;
                if (memcmp(candidate->signature, "RSD PTR ", 8) == 0) {
                    if (acpi_validate_checksum(candidate, 20)) {
                        if (candidate->revision >= 2) {
                            if (candidate->length >= 36 && acpi_validate_checksum(candidate, candidate->length)) {
                                return candidate;
                            }
                        } else {
                            return candidate;
                        }
                    }
                }
            }
        }
    }

    /* 2. Search Main BIOS Read-Only Memory (0xE0000..0xFFFFF) */
    for (uintptr_t addr = 0x000E0000; addr <= 0x000FFFF0; addr += 16) {
        const acpi_rsdp_t *candidate = (const acpi_rsdp_t *)addr;
        if (memcmp(candidate->signature, "RSD PTR ", 8) == 0) {
            if (acpi_validate_checksum(candidate, 20)) {
                if (candidate->revision >= 2) {
                    if (candidate->length >= 36 && acpi_validate_checksum(candidate, candidate->length)) {
                        return candidate;
                    }
                } else {
                    return candidate;
                }
            }
        }
    }

    return NULL;
#endif
}

static int register_table(const acpi_sdt_header_t *header) {
    if (!header) return ACPI_ERR_INVALID;

    /* Check if already registered */
    for (uint32_t i = 0; i < registered_table_count; i++) {
        if (registered_tables[i] == header) {
            return ACPI_OK;
        }
    }

    if (registered_table_count >= ACPI_MAX_TABLES) {
        return ACPI_ERR_TABLE_LIMIT;
    }

    registered_tables[registered_table_count++] = header;
    return ACPI_OK;
}

int acpi_init_from_rsdp(const acpi_rsdp_t *rsdp) {
    if (!rsdp) return ACPI_ERR_INVALID;

    /* Validate RSDP signature */
    if (memcmp(rsdp->signature, "RSD PTR ", 8) != 0) {
        return ACPI_ERR_INVALID;
    }

    /* Validate 20-byte checksum */
    if (!acpi_validate_checksum(rsdp, 20)) {
        return ACPI_ERR_CHECKSUM;
    }

    registered_table_count = 0;
    madt_is_parsed = 0;
    memset(&parsed_madt, 0, sizeof(parsed_madt));

    /* Prefer XSDT (64-bit addresses, ACPI 2.0+) if available */
    if (rsdp->revision >= 2 && rsdp->xsdt_address != 0) {
        if (!acpi_validate_checksum(rsdp, rsdp->length)) {
            return ACPI_ERR_CHECKSUM;
        }

        const acpi_sdt_header_t *xsdt = (const acpi_sdt_header_t *)(uintptr_t)rsdp->xsdt_address;
        if (!xsdt || memcmp(xsdt->signature, "XSDT", 4) != 0) {
            return ACPI_ERR_INVALID;
        }

        if (!acpi_validate_checksum(xsdt, xsdt->length)) {
            return ACPI_ERR_CHECKSUM;
        }

        register_table(xsdt);

        uint32_t num_entries = (xsdt->length - sizeof(acpi_sdt_header_t)) / sizeof(uint64_t);
        const uint8_t *entries_bytes = (const uint8_t *)xsdt + sizeof(acpi_sdt_header_t);

        for (uint32_t i = 0; i < num_entries; i++) {
            uint64_t table_addr = 0;
            memcpy(&table_addr, entries_bytes + i * sizeof(uint64_t), sizeof(uint64_t));
            if (table_addr == 0 || table_addr > 0xFFFFFFFFFFFFF000ULL) continue;

            const acpi_sdt_header_t *hdr = (const acpi_sdt_header_t *)(uintptr_t)table_addr;
            if (hdr && hdr->length >= sizeof(acpi_sdt_header_t)) {
                if (acpi_validate_checksum(hdr, hdr->length)) {
                    register_table(hdr);
                }
            }
        }
    } else if (rsdp->rsdt_address != 0) {
        /* ACPI 1.0 RSDT (32-bit addresses) */
        const acpi_sdt_header_t *rsdt = (const acpi_sdt_header_t *)(uintptr_t)rsdp->rsdt_address;
        if (!rsdt || memcmp(rsdt->signature, "RSDT", 4) != 0) {
            return ACPI_ERR_INVALID;
        }

        if (!acpi_validate_checksum(rsdt, rsdt->length)) {
            return ACPI_ERR_CHECKSUM;
        }

        register_table(rsdt);

        uint32_t num_entries = (rsdt->length - sizeof(acpi_sdt_header_t)) / sizeof(uint32_t);
        const uint32_t *entries = (const uint32_t *)((uintptr_t)rsdt + sizeof(acpi_sdt_header_t));

        for (uint32_t i = 0; i < num_entries; i++) {
            uint32_t table_addr = entries[i];
            if (table_addr == 0) continue;

            const acpi_sdt_header_t *hdr = (const acpi_sdt_header_t *)(uintptr_t)table_addr;
            if (hdr && hdr->length >= sizeof(acpi_sdt_header_t)) {
                if (acpi_validate_checksum(hdr, hdr->length)) {
                    register_table(hdr);
                }
            }
        }
    } else {
        return ACPI_ERR_NOT_FOUND;
    }

    /* Check for MADT and parse it */
    const acpi_sdt_header_t *madt_hdr = acpi_find_table("APIC");
    if (madt_hdr) {
        int rc = acpi_parse_madt((const acpi_madt_t *)madt_hdr, &parsed_madt);
        if (rc == ACPI_OK) {
            madt_is_parsed = 1;
            /* Register CPUs discovered in MADT */
            for (uint32_t i = 0; i < parsed_madt.cpu_count; i++) {
                cpu_register_ap(parsed_madt.cpus[i].apic_id, parsed_madt.cpus[i].acpi_id);
            }
        }
    }

    return ACPI_OK;
}

const acpi_rsdp_t *acpi_find_rsdp_with_mbi(const struct multiboot_info *mbi) {
    const acpi_rsdp_t *rsdp = acpi_find_rsdp();
    if (rsdp) return rsdp;

#ifndef __KERNEL_HOST_TEST__
    /* 2. Scan Multiboot memory map for ACPI Reclaim / NVS regions */
    if (mbi && (mbi->flags & MULTIBOOT_INFO_MEM_MAP) && mbi->mmap_addr != 0) {
        uintptr_t mmap_cur = mbi->mmap_addr;
        uintptr_t mmap_end = mbi->mmap_addr + mbi->mmap_length;

        while (mmap_cur < mmap_end) {
            const struct multiboot_mmap_entry *entry = (const struct multiboot_mmap_entry *)mmap_cur;
            if (entry->size == 0) break;

            if (entry->type == MULTIBOOT_MEMORY_ACPI_RECLAIM ||
                entry->type == MULTIBOOT_MEMORY_NVS ||
                entry->type == MULTIBOOT_MEMORY_AVAILABLE) {
                uint64_t start = entry->addr;
                uint64_t len = entry->len;
                if (start < 0xFFFFFFFFULL && len > 0 && len <= 32 * 1024 * 1024) {
                    uint64_t end_scan = start + len;
                    if (end_scan > 0xFFFFFFF0ULL) end_scan = 0xFFFFFFF0ULL;

                    for (uintptr_t addr = (uintptr_t)start; addr <= (uintptr_t)(end_scan - 20); addr += 16) {
                        const acpi_rsdp_t *candidate = (const acpi_rsdp_t *)addr;
                        if (memcmp(candidate->signature, "RSD PTR ", 8) == 0) {
                            if (acpi_validate_checksum(candidate, 20)) {
                                if (candidate->revision >= 2) {
                                    if (candidate->length >= 36 &&
                                        acpi_validate_checksum(candidate, candidate->length)) {
                                        return candidate;
                                    }
                                } else {
                                    return candidate;
                                }
                            }
                        }
                    }
                }
            }
            mmap_cur += entry->size + sizeof(entry->size);
        }
    }
#else
    (void)mbi;
#endif

    return NULL;
}

int acpi_init(const struct multiboot_info *mbi) {
    const acpi_rsdp_t *rsdp = acpi_find_rsdp_with_mbi(mbi);
    if (!rsdp) {
        kwarn("ACPI: RSDP table not found in BIOS or EBDA memory");
        return ACPI_ERR_NOT_FOUND;
    }

    kinfo("ACPI: RSDP located at %p (OEM='%.6s', Rev=%u)",
          (void *)rsdp, rsdp->oem_id, rsdp->revision);

    int rc = acpi_init_from_rsdp(rsdp);
    if (rc != ACPI_OK) {
        kerror("ACPI: Failed to initialize ACPI tables (error=%d)", rc);
        return rc;
    }

    acpi_print_tables();
    return ACPI_OK;
}

const acpi_sdt_header_t *acpi_find_table(const char *signature) {
    if (!signature) return NULL;
    for (uint32_t i = 0; i < registered_table_count; i++) {
        if (memcmp(registered_tables[i]->signature, signature, 4) == 0) {
            return registered_tables[i];
        }
    }
    return NULL;
}

uint32_t acpi_get_table_count(void) {
    return registered_table_count;
}

const acpi_sdt_header_t *acpi_get_table(uint32_t index) {
    if (index >= registered_table_count) return NULL;
    return registered_tables[index];
}

int acpi_parse_madt(const acpi_madt_t *madt, acpi_madt_info_t *out_info) {
    if (!madt || !out_info) return ACPI_ERR_INVALID;

    if (memcmp(madt->header.signature, "APIC", 4) != 0) {
        return ACPI_ERR_INVALID;
    }

    if (madt->header.length < sizeof(acpi_madt_t)) {
        return ACPI_ERR_INVALID;
    }

    if (!acpi_validate_checksum(madt, madt->header.length)) {
        return ACPI_ERR_CHECKSUM;
    }

    memset(out_info, 0, sizeof(acpi_madt_info_t));
    out_info->lapic_phys_addr = madt->lapic_addr;
    out_info->has_dual_pic = (madt->flags & 1) ? 1 : 0;

    const uint8_t *ptr = (const uint8_t *)madt + sizeof(acpi_madt_t);
    const uint8_t *end = (const uint8_t *)madt + madt->header.length;

    while (ptr + 2 <= end) {
        uint8_t type = ptr[0];
        uint8_t len  = ptr[1];

        if (len < 2 || ptr + len > end) {
            // Malformed sub-table entry
            return ACPI_ERR_INVALID;
        }

        switch (type) {
            case 0: { // Processor Local APIC
                if (len >= sizeof(acpi_madt_lapic_t)) {
                    const acpi_madt_lapic_t *lapic = (const acpi_madt_lapic_t *)ptr;
                    // Bit 0 = Enabled, Bit 1 = Online Capable
                    if ((lapic->flags & 1) || (lapic->flags & 2)) {
                        if (out_info->cpu_count < ACPI_MAX_CPUS) {
                            uint32_t idx = out_info->cpu_count++;
                            out_info->cpus[idx].apic_id = lapic->apic_id;
                            out_info->cpus[idx].acpi_id = lapic->acpi_processor_id;
                            out_info->cpus[idx].flags   = lapic->flags;
                        }
                    }
                }
                break;
            }
            case 1: { // I/O APIC
                if (len >= sizeof(acpi_madt_ioapic_t)) {
                    const acpi_madt_ioapic_t *ioapic = (const acpi_madt_ioapic_t *)ptr;
                    if (out_info->ioapic_count < ACPI_MAX_IOAPIC) {
                        uint32_t idx = out_info->ioapic_count++;
                        out_info->ioapics[idx].id        = ioapic->ioapic_id;
                        out_info->ioapics[idx].phys_addr = ioapic->ioapic_addr;
                        out_info->ioapics[idx].gsi_base  = ioapic->gsi_base;
                    }
                }
                break;
            }
            case 2: { // Interrupt Source Override
                if (len >= sizeof(acpi_madt_iso_t)) {
                    const acpi_madt_iso_t *iso = (const acpi_madt_iso_t *)ptr;
                    if (out_info->iso_count < ACPI_MAX_ISO) {
                        uint32_t idx = out_info->iso_count++;
                        out_info->isos[idx].bus        = iso->bus;
                        out_info->isos[idx].source_irq = iso->source_irq;
                        out_info->isos[idx].gsi        = iso->gsi;
                        out_info->isos[idx].flags      = iso->flags;
                    }
                }
                break;
            }
            default:
                // Other sub-tables (NMI, LAPIC Address Override, etc.)
                break;
        }

        ptr += len;
    }

    return ACPI_OK;
}

const acpi_madt_info_t *acpi_get_madt_info(void) {
    return madt_is_parsed ? &parsed_madt : NULL;
}

void acpi_print_tables(void) {
    kinfo("=== ACPI Platform Discovery (%u tables detected) ===", registered_table_count);
    for (uint32_t i = 0; i < registered_table_count; i++) {
        const acpi_sdt_header_t *h = registered_tables[i];
        kinfo("  [%u] Table '%.4s' Rev=%u Length=%u OEM='%.6s' TableID='%.8s'",
              i, h->signature, h->revision, h->length, h->oem_id, h->oem_table_id);
    }
    if (madt_is_parsed) {
        kinfo("ACPI MADT: Local APIC Base=%p, CPUs=%u, IOAPICs=%u, ISOs=%u (Dual-PIC=%u)",
              (void *)parsed_madt.lapic_phys_addr,
              parsed_madt.cpu_count,
              parsed_madt.ioapic_count,
              parsed_madt.iso_count,
              parsed_madt.has_dual_pic);
    }
}
