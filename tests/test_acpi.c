#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "acpi.h"

/* Helper to compute and set 8-bit ACPI checksum */
static void set_acpi_checksum(void *data, size_t length, size_t checksum_offset) {
    uint8_t *bytes = (uint8_t *)data;
    bytes[checksum_offset] = 0;
    uint8_t sum = 0;
    for (size_t i = 0; i < length; i++) {
        sum += bytes[i];
    }
    bytes[checksum_offset] = (uint8_t)(0x100 - sum);
}

static void test_acpi_checksum(void) {
    printf("[TEST  1] Running test_acpi_checksum                          ... ");
    uint8_t buffer[32];
    for (size_t i = 0; i < sizeof(buffer); i++) buffer[i] = (uint8_t)(i * 3 + 1);

    set_acpi_checksum(buffer, sizeof(buffer), 9);
    assert(acpi_validate_checksum(buffer, sizeof(buffer)) == 1);

    /* Invalidate checksum */
    buffer[9] ^= 0x55;
    assert(acpi_validate_checksum(buffer, sizeof(buffer)) == 0);
    printf("PASSED\n");
}

static void test_acpi_rsdp_validation(void) {
    printf("[TEST  2] Running test_acpi_rsdp_validation                    ... ");
    acpi_rsdp_t rsdp;
    memset(&rsdp, 0, sizeof(rsdp));
    memcpy(rsdp.signature, "RSD PTR ", 8);
    memcpy(rsdp.oem_id, "PURPLE", 6);
    rsdp.revision = 0;
    rsdp.rsdt_address = 0; // No tables

    set_acpi_checksum(&rsdp, 20, offsetof(acpi_rsdp_t, checksum));
    assert(acpi_init_from_rsdp(&rsdp) == ACPI_ERR_NOT_FOUND); // Valid RSDP but 0 RSDT addr

    /* Corrupt signature */
    rsdp.signature[0] = 'X';
    assert(acpi_init_from_rsdp(&rsdp) == ACPI_ERR_INVALID);
    printf("PASSED\n");
}

static void test_acpi_rsdt_parsing_valid(void) {
    printf("[TEST  3] Running test_acpi_rsdt_parsing_valid                 ... ");
    /* Create synthetic FACP table */
    acpi_sdt_header_t facp;
    memset(&facp, 0, sizeof(facp));
    memcpy(facp.signature, "FACP", 4);
    facp.length = sizeof(facp);
    facp.revision = 1;
    memcpy(facp.oem_id, "PURPLE", 6);
    memcpy(facp.oem_table_id, "PURPLE01", 8);
    set_acpi_checksum(&facp, sizeof(facp), offsetof(acpi_sdt_header_t, checksum));

    /* Create synthetic RSDT with 1 entry */
    uint32_t rsdt_len = sizeof(acpi_sdt_header_t) + sizeof(uint32_t);
    uint8_t *rsdt_buf = calloc(1, rsdt_len);
    acpi_sdt_header_t *rsdt = (acpi_sdt_header_t *)rsdt_buf;
    memcpy(rsdt->signature, "RSDT", 4);
    rsdt->length = rsdt_len;
    rsdt->revision = 1;
    memcpy(rsdt->oem_id, "PURPLE", 6);

    uint32_t *entries = (uint32_t *)(rsdt_buf + sizeof(acpi_sdt_header_t));
    entries[0] = (uint32_t)(uintptr_t)&facp;
    set_acpi_checksum(rsdt_buf, rsdt_len, offsetof(acpi_sdt_header_t, checksum));

    /* Create RSDP pointing to RSDT */
    acpi_rsdp_t rsdp;
    memset(&rsdp, 0, sizeof(rsdp));
    memcpy(rsdp.signature, "RSD PTR ", 8);
    memcpy(rsdp.oem_id, "PURPLE", 6);
    rsdp.revision = 0;
    rsdp.rsdt_address = (uint32_t)(uintptr_t)rsdt;
    set_acpi_checksum(&rsdp, 20, offsetof(acpi_rsdp_t, checksum));

    int rc = acpi_init_from_rsdp(&rsdp);
    assert(rc == ACPI_OK);
    assert(acpi_get_table_count() == 2); // RSDT + FACP

    const acpi_sdt_header_t *found_facp = acpi_find_table("FACP");
    assert(found_facp != NULL);
    assert(memcmp(found_facp->oem_table_id, "PURPLE01", 8) == 0);

    assert(acpi_find_table("NONX") == NULL);
    free(rsdt_buf);
    printf("PASSED\n");
}

static void test_acpi_xsdt_parsing_64bit(void) {
    printf("[TEST  4] Running test_acpi_xsdt_parsing_64bit                 ... ");
    /* Create synthetic HPET table */
    acpi_sdt_header_t hpet;
    memset(&hpet, 0, sizeof(hpet));
    memcpy(hpet.signature, "HPET", 4);
    hpet.length = sizeof(hpet);
    hpet.revision = 1;
    memcpy(hpet.oem_id, "PURPLE", 6);
    set_acpi_checksum(&hpet, sizeof(hpet), offsetof(acpi_sdt_header_t, checksum));

    /* Create synthetic XSDT with 1 64-bit entry */
    uint32_t xsdt_len = sizeof(acpi_sdt_header_t) + sizeof(uint64_t);
    uint8_t *xsdt_buf = calloc(1, xsdt_len);
    acpi_sdt_header_t *xsdt = (acpi_sdt_header_t *)xsdt_buf;
    memcpy(xsdt->signature, "XSDT", 4);
    xsdt->length = xsdt_len;
    xsdt->revision = 1;
    memcpy(xsdt->oem_id, "PURPLE", 6);

    uint64_t *entries = (uint64_t *)(xsdt_buf + sizeof(acpi_sdt_header_t));
    entries[0] = (uint64_t)(uintptr_t)&hpet;
    set_acpi_checksum(xsdt_buf, xsdt_len, offsetof(acpi_sdt_header_t, checksum));

    /* Create RSDP 2.0 */
    acpi_rsdp_t rsdp;
    memset(&rsdp, 0, sizeof(rsdp));
    memcpy(rsdp.signature, "RSD PTR ", 8);
    memcpy(rsdp.oem_id, "PURPLE", 6);
    rsdp.revision = 2;
    rsdp.length = sizeof(rsdp);
    rsdp.xsdt_address = (uint64_t)(uintptr_t)xsdt;
    set_acpi_checksum(&rsdp, 20, offsetof(acpi_rsdp_t, checksum));
    set_acpi_checksum(&rsdp, sizeof(rsdp), offsetof(acpi_rsdp_t, extended_checksum));

    int rc = acpi_init_from_rsdp(&rsdp);
    assert(rc == ACPI_OK);
    assert(acpi_find_table("HPET") != NULL);
    free(xsdt_buf);
    printf("PASSED\n");
}

static void test_acpi_madt_parsing(void) {
    printf("[TEST  5] Running test_acpi_madt_parsing                       ... ");
    /* Buffer for MADT + 4 LAPICs + 1 IOAPIC + 1 ISO */
    uint32_t total_len = sizeof(acpi_madt_t) +
                         4 * sizeof(acpi_madt_lapic_t) +
                         1 * sizeof(acpi_madt_ioapic_t) +
                         1 * sizeof(acpi_madt_iso_t);

    uint8_t *buf = calloc(1, total_len);
    acpi_madt_t *madt = (acpi_madt_t *)buf;
    memcpy(madt->header.signature, "APIC", 4);
    madt->header.length = total_len;
    madt->header.revision = 1;
    memcpy(madt->header.oem_id, "PURPLE", 6);
    madt->lapic_addr = 0xFEE00000;
    madt->flags = 1; // PC-AT dual PIC

    uint8_t *cur = buf + sizeof(acpi_madt_t);

    /* 4 Local APICs (0, 1 enabled; 2, 3 disabled) */
    for (int i = 0; i < 4; i++) {
        acpi_madt_lapic_t *l = (acpi_madt_lapic_t *)cur;
        l->type = 0;
        l->length = sizeof(acpi_madt_lapic_t);
        l->acpi_processor_id = (uint8_t)i;
        l->apic_id = (uint8_t)i;
        l->flags = (i < 2) ? 1 : 0; // 0 & 1 enabled
        cur += sizeof(acpi_madt_lapic_t);
    }

    /* 1 IOAPIC */
    acpi_madt_ioapic_t *io = (acpi_madt_ioapic_t *)cur;
    io->type = 1;
    io->length = sizeof(acpi_madt_ioapic_t);
    io->ioapic_id = 1;
    io->ioapic_addr = 0xFEC00000;
    io->gsi_base = 0;
    cur += sizeof(acpi_madt_ioapic_t);

    /* 1 ISO: IRQ 0 -> GSI 2 */
    acpi_madt_iso_t *iso = (acpi_madt_iso_t *)cur;
    iso->type = 2;
    iso->length = sizeof(acpi_madt_iso_t);
    iso->bus = 0;
    iso->source_irq = 0;
    iso->gsi = 2;
    iso->flags = 0;

    set_acpi_checksum(buf, total_len, offsetof(acpi_sdt_header_t, checksum));

    acpi_madt_info_t info;
    int rc = acpi_parse_madt(madt, &info);
    assert(rc == ACPI_OK);
    assert(info.lapic_phys_addr == 0xFEE00000);
    assert(info.has_dual_pic == 1);
    assert(info.cpu_count == 2); // 2 enabled CPUs
    assert(info.cpus[0].apic_id == 0);
    assert(info.cpus[1].apic_id == 1);
    assert(info.ioapic_count == 1);
    assert(info.ioapics[0].phys_addr == 0xFEC00000);
    assert(info.iso_count == 1);
    assert(info.isos[0].source_irq == 0 && info.isos[0].gsi == 2);

    free(buf);
    printf("PASSED\n");
}

static void test_acpi_madt_malformed_entries(void) {
    printf("[TEST  6] Running test_acpi_madt_malformed_entries             ... ");
    uint32_t total_len = sizeof(acpi_madt_t) + 16;
    uint8_t *buf = calloc(1, total_len);
    acpi_madt_t *madt = (acpi_madt_t *)buf;
    memcpy(madt->header.signature, "APIC", 4);
    madt->header.length = total_len;
    madt->lapic_addr = 0xFEE00000;

    /* Bad sub-entry length = 0 (infinite loop trap) */
    buf[sizeof(acpi_madt_t)] = 0; // type 0
    buf[sizeof(acpi_madt_t) + 1] = 0; // length 0
    set_acpi_checksum(buf, total_len, offsetof(acpi_sdt_header_t, checksum));

    acpi_madt_info_t info;
    int rc = acpi_parse_madt(madt, &info);
    assert(rc == ACPI_ERR_INVALID);

    /* Bad sub-entry length exceeding table bounds */
    buf[sizeof(acpi_madt_t) + 1] = 100; // exceeds total_len
    set_acpi_checksum(buf, total_len, offsetof(acpi_sdt_header_t, checksum));
    rc = acpi_parse_madt(madt, &info);
    assert(rc == ACPI_ERR_INVALID);

    free(buf);
    printf("PASSED\n");
}

static void test_acpi_madt_truncated(void) {
    printf("[TEST  7] Running test_acpi_madt_truncated                     ... ");
    acpi_sdt_header_t hdr;
    memcpy(hdr.signature, "APIC", 4);
    hdr.length = sizeof(hdr); // Smaller than sizeof(acpi_madt_t)
    set_acpi_checksum(&hdr, sizeof(hdr), offsetof(acpi_sdt_header_t, checksum));

    acpi_madt_info_t info;
    int rc = acpi_parse_madt((const acpi_madt_t *)&hdr, &info);
    assert(rc == ACPI_ERR_INVALID);
    printf("PASSED\n");
}

static void test_acpi_duplicate_table_dedup(void) {
    printf("[TEST  8] Running test_acpi_duplicate_table_dedup              ... ");
    acpi_sdt_header_t tbl;
    memcpy(tbl.signature, "TEST", 4);
    tbl.length = sizeof(tbl);
    set_acpi_checksum(&tbl, sizeof(tbl), offsetof(acpi_sdt_header_t, checksum));

    /* Create RSDT referencing tbl twice */
    uint32_t rsdt_len = sizeof(acpi_sdt_header_t) + 2 * sizeof(uint32_t);
    uint8_t *rsdt_buf = calloc(1, rsdt_len);
    acpi_sdt_header_t *rsdt = (acpi_sdt_header_t *)rsdt_buf;
    memcpy(rsdt->signature, "RSDT", 4);
    rsdt->length = rsdt_len;

    uint32_t *entries = (uint32_t *)(rsdt_buf + sizeof(acpi_sdt_header_t));
    entries[0] = (uint32_t)(uintptr_t)&tbl;
    entries[1] = (uint32_t)(uintptr_t)&tbl;
    set_acpi_checksum(rsdt_buf, rsdt_len, offsetof(acpi_sdt_header_t, checksum));

    acpi_rsdp_t rsdp;
    memset(&rsdp, 0, sizeof(rsdp));
    memcpy(rsdp.signature, "RSD PTR ", 8);
    rsdp.rsdt_address = (uint32_t)(uintptr_t)rsdt;
    set_acpi_checksum(&rsdp, 20, offsetof(acpi_rsdp_t, checksum));

    int rc = acpi_init_from_rsdp(&rsdp);
    assert(rc == ACPI_OK);
    /* Should register RSDT + 1 instance of TEST (deduplicated) */
    assert(acpi_get_table_count() == 2);
    free(rsdt_buf);
    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running ACPI Platform & MADT Unit Test Suite        \n");
    printf("=====================================================\n");

    test_acpi_checksum();
    test_acpi_rsdp_validation();
    test_acpi_rsdt_parsing_valid();
    test_acpi_xsdt_parsing_64bit();
    test_acpi_madt_parsing();
    test_acpi_madt_malformed_entries();
    test_acpi_madt_truncated();
    test_acpi_duplicate_table_dedup();

    printf("=====================================================\n");
    printf(" ACPI Tests: 8/8 passed                              \n");
    printf("=====================================================\n");
    return 0;
}
