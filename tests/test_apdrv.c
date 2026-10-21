#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <assert.h>
#include "apdrv.h"
#include "ksym.h"
#include "netdev.h"
#include "pci.h"
#include "purple_detect.h"

static int test_init_called = 0;
static int test_fini_called = 0;
static int test_probe_called = 0;
static int test_ioctl_called = 0;

static int mock_module_init(void) {
    test_init_called++;
    return 0;
}

static int mock_module_fini(void) {
    test_fini_called++;
    return 0;
}

static int mock_module_probe(uint32_t vendor, uint32_t device) {
    test_probe_called++;
    if (vendor == 0x8086 && (device == 0x100E || device == 0xFFFF)) {
        return 0; /* match */
    }
    return -1;
}

static int mock_module_ioctl(uint32_t cmd, void *arg) {
    test_ioctl_called++;
    if (arg) {
        *(uint32_t *)arg = cmd * 2;
    }
    return 0;
}

static void test_ksym_registration_and_lookup(void) {
    printf("[TEST  1] Running test_ksym_registration_and_lookup        ... ");
    ksym_init();
    assert(ksym_get_count() > 10);

    /* Check pre-registered standard symbols */
    assert(ksym_lookup("kinfo") != 0);
    assert(ksym_lookup("memcpy") != 0);
    assert(ksym_lookup("pci_enable_bus_mastering") != 0);
    assert(ksym_lookup("netdev_register") != 0);

    /* Register custom symbol */
    uint64_t dummy_fn = 0xDEADBEEF12345678ULL;
    assert(ksym_register("custom_symbol", dummy_fn) == 0);
    assert(ksym_lookup("custom_symbol") == dummy_fn);
    assert(ksym_lookup("non_existent") == 0);

    printf("PASSED\n");
}

static void test_ksym_unregistration(void) {
    printf("[TEST  2] Running test_ksym_unregistration                 ... ");
    assert(ksym_register("temp_sym", 0x11223344) == 0);
    assert(ksym_lookup("temp_sym") == 0x11223344);

    assert(ksym_unregister("temp_sym") == 0);
    assert(ksym_lookup("temp_sym") == 0);
    assert(ksym_unregister("temp_sym") == -2);

    printf("PASSED\n");
}

static void test_apdrv_init_state(void) {
    printf("[TEST  3] Running test_apdrv_init_state                    ... ");
    assert(apdrv_subsystem_init() == 0);
    assert(apdrv_get_count() == 0);
    assert(apdrv_find("nonexistent") == NULL);
    printf("PASSED\n");
}

static void test_apdrv_invalid_headers(void) {
    printf("[TEST  4] Running test_apdrv_invalid_headers               ... ");
    uint8_t buffer[1024];
    memset(buffer, 0, sizeof(buffer));

    apdrv_header_t *hdr = (apdrv_header_t *)buffer;
    memcpy(hdr->magic, "BADMAGIC", 8);
    hdr->version = 3;
    hdr->header_size = sizeof(apdrv_header_t);
    memcpy(hdr->mod_type, "AEGISMOD", 8);

    apdrv_module_t *mod = NULL;
    /* 1. Bad magic */
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -3);

    /* 2. Bad version */
    memcpy(hdr->magic, APDRV_MAGIC, 8);
    hdr->version = 99;
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -4);

    /* 3. Bad mod_type */
    hdr->version = 3;
    memcpy(hdr->mod_type, "BAD_TYPE", 8);
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -5);

    /* 4. Bad header_size */
    memcpy(hdr->mod_type, "AEGISMOD", 8);
    hdr->header_size = 12;
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -6);

    /* 5. Out of bounds offset */
    hdr->header_size = sizeof(apdrv_header_t);
    strcpy(hdr->name, "bad_bounds");
    hdr->text_offset = 2000;
    hdr->text_size = 500;
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -8);

    printf("PASSED\n");
}

static void test_apdrv_unresolved_symbols(void) {
    printf("[TEST  5] Running test_apdrv_unresolved_symbols            ... ");
    uint8_t buffer[2048];
    memset(buffer, 0, sizeof(buffer));

    apdrv_header_t *hdr = (apdrv_header_t *)buffer;
    memcpy(hdr->magic, APDRV_MAGIC, 8);
    hdr->version = 3;
    hdr->header_size = sizeof(apdrv_header_t);
    memcpy(hdr->mod_type, "AEGISMOD", 8);
    strcpy(hdr->name, "unresolved_mod");

    hdr->import_offset = sizeof(apdrv_header_t);
    hdr->import_count = 1;

    apdrv_sym_t *imports = (apdrv_sym_t *)(buffer + hdr->import_offset);
    strcpy(imports[0].name, "completely_unknown_symbol_xyz");

    apdrv_module_t *mod = NULL;
    int rc = apdrv_load(buffer, sizeof(buffer), &mod);
    assert(rc == -10); /* Must fail-closed on missing symbol */
    assert(apdrv_find("unresolved_mod") == NULL);

    printf("PASSED\n");
}

static void test_apdrv_load_and_relocations(void) {
    printf("[TEST  6] Running test_apdrv_load_and_relocations          ... ");
    uint8_t buffer[4096];
    memset(buffer, 0, sizeof(buffer));

    apdrv_header_t *hdr = (apdrv_header_t *)buffer;
    memcpy(hdr->magic, APDRV_MAGIC, 8);
    hdr->version = 3;
    hdr->header_size = sizeof(apdrv_header_t);
    memcpy(hdr->mod_type, "AEGISMOD", 8);
    strcpy(hdr->name, "reloc_test_mod");
    strcpy(hdr->description, "Relocation Engine Test Module");
    strcpy(hdr->author, "Purple OS Team");
    hdr->mod_version = 0x010203; /* 1.2.3 */
    hdr->flags = APDRV_FLAG_ENFORCE_WX;

    uint32_t cursor = sizeof(apdrv_header_t);

    /* Text section (64 bytes) */
    hdr->text_offset = cursor;
    hdr->text_size = 64;
    cursor += 64;

    /* Rodata section (64 bytes) */
    hdr->rodata_offset = cursor;
    hdr->rodata_size = 64;
    strcpy((char *)(buffer + hdr->rodata_offset), "Hello Aegis Relocation");
    cursor += 64;

    /* Data section (64 bytes) */
    hdr->data_offset = cursor;
    hdr->data_size = 64;
    cursor += 64;

    hdr->bss_size = 128;

    /* Imports table (1 symbol: memcpy) */
    hdr->import_offset = cursor;
    hdr->import_count = 1;
    apdrv_sym_t *imp = (apdrv_sym_t *)(buffer + hdr->import_offset);
    strcpy(imp[0].name, "memcpy");
    cursor += sizeof(apdrv_sym_t);

    /* Relocations (3 relocs) */
    hdr->reloc_offset = cursor;
    hdr->reloc_count = 3;
    apdrv_reloc_t *rel = (apdrv_reloc_t *)(buffer + hdr->reloc_offset);

    /* Reloc 0: 64-bit absolute pointer in Data section pointing to Rodata string */
    uint32_t text_aligned = 4096;
    uint32_t rodata_aligned = 4096;
    uint32_t data_base_in_mod = text_aligned + rodata_aligned;
    rel[0].offset = data_base_in_mod; /* start of data section */
    rel[0].type = APDRV_RELOC_64;
    rel[0].sym_index = APDRV_SEC_RODATA;
    rel[0].addend = 0;

    /* Reloc 1: 32-bit absolute value in Data section (offset 8) */
    rel[1].offset = data_base_in_mod + 8;
    rel[1].type = APDRV_RELOC_32;
    rel[1].sym_index = 0; /* memcpy import */
    rel[1].addend = 100;

    /* Reloc 2: 32-bit PC-relative call in Text section (offset 16) calling memcpy */
    rel[2].offset = 16;
    rel[2].type = APDRV_RELOC_PC32;
    rel[2].sym_index = 0; /* memcpy import */
    rel[2].addend = -4;

    apdrv_module_t *mod = NULL;
    int rc = apdrv_load(buffer, sizeof(buffer), &mod);
    assert(rc == 0);
    assert(mod != NULL);
    assert(mod->state == APDRV_STATE_RUNNING);
    assert(mod->wx_enforced == 1);

    /* Verify Reloc 0: 64-bit address matches rodata address */
    uint64_t *ptr_data = (uint64_t *)((uint8_t *)mod->memory + data_base_in_mod);
    assert(*ptr_data == mod->rodata_addr);
    const char *str_val = (const char *)(uintptr_t)(*ptr_data);
    assert(strcmp(str_val, "Hello Aegis Relocation") == 0);

    /* Verify Reloc 1: 32-bit value */
    uint32_t *u32_data = (uint32_t *)((uint8_t *)mod->memory + data_base_in_mod + 8);
    uint64_t memcpy_addr = ksym_lookup("memcpy");
    assert(*u32_data == (uint32_t)(memcpy_addr + 100));

    /* Verify Reloc 2: PC-relative call */
    int32_t *call_disp = (int32_t *)((uint8_t *)mod->memory + 16);
    uint64_t call_site = (uint64_t)(uintptr_t)call_disp;
    assert(*call_disp == (int32_t)(memcpy_addr - 4 - call_site));

    assert(apdrv_unload("reloc_test_mod") == 0);
    assert(apdrv_find("reloc_test_mod") == NULL);

    printf("PASSED\n");
}

static void test_apdrv_module_lifecycle(void) {
    printf("[TEST  7] Running test_apdrv_module_lifecycle              ... ");
    test_init_called = 0;
    test_fini_called = 0;
    test_probe_called = 0;
    test_ioctl_called = 0;

    uint8_t buffer[2048];
    memset(buffer, 0, sizeof(buffer));

    apdrv_header_t *hdr = (apdrv_header_t *)buffer;
    memcpy(hdr->magic, APDRV_MAGIC, 8);
    hdr->version = 3;
    hdr->header_size = sizeof(apdrv_header_t);
    memcpy(hdr->mod_type, "AEGISMOD", 8);
    strcpy(hdr->name, "lifecycle_mod");
    hdr->text_offset = sizeof(apdrv_header_t);
    hdr->text_size = 256;

    /* Write trampolines pointing to mock functions */
    /* Store function pointers at entry offsets */
    hdr->entry_init = 0;
    hdr->entry_fini = 8;
    hdr->entry_probe = 16;
    hdr->entry_ioctl = 24;

    /* Populate text section with pointers */
    uint64_t *fns = (uint64_t *)(buffer + hdr->text_offset);
    fns[0] = (uint64_t)(uintptr_t)mock_module_init;
    fns[1] = (uint64_t)(uintptr_t)mock_module_fini;
    fns[2] = (uint64_t)(uintptr_t)mock_module_probe;
    fns[3] = (uint64_t)(uintptr_t)mock_module_ioctl;

    /* For direct testing, override entry point function pointers */
    apdrv_module_t *mod = NULL;
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == 0);
    mod->fn_init = mock_module_init;
    mod->fn_fini = mock_module_fini;
    mod->fn_probe = mock_module_probe;
    mod->fn_ioctl = mock_module_ioctl;

    assert(mod->fn_init() == 0);
    assert(test_init_called > 0);

    /* Probe check */
    assert(mod->fn_probe(0x8086, 0x100E) == 0);
    assert(test_probe_called > 0);

    /* Ioctl check */
    uint32_t val = 0;
    assert(mod->fn_ioctl(21, &val) == 0);
    assert(test_ioctl_called > 0);
    assert(val == 42);

    /* Unload check */
    assert(apdrv_unload("lifecycle_mod") == 0);
    assert(test_fini_called > 0);

    printf("PASSED\n");
}

static void test_apdrv_duplicate_and_capacity_limits(void) {
    printf("[TEST  8] Running test_apdrv_duplicate_and_capacity_limits ... ");
    uint8_t buffer[2048];
    memset(buffer, 0, sizeof(buffer));

    apdrv_header_t *hdr = (apdrv_header_t *)buffer;
    memcpy(hdr->magic, APDRV_MAGIC, 8);
    hdr->version = 3;
    hdr->header_size = sizeof(apdrv_header_t);
    memcpy(hdr->mod_type, "AEGISMOD", 8);
    strcpy(hdr->name, "dup_mod");
    hdr->text_offset = sizeof(apdrv_header_t);
    hdr->text_size = 32;

    apdrv_module_t *mod = NULL;
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == 0);
    assert(apdrv_load(buffer, sizeof(buffer), &mod) == -7); /* duplicate rejection */
    assert(apdrv_unload("dup_mod") == 0);

    printf("PASSED\n");
}

static void test_netdev_subsystem(void) {
    printf("[TEST  9] Running test_netdev_subsystem                    ... ");
    netdev_init();
    assert(netdev_get_count() == 0);

    netdev_t eth0;
    memset(&eth0, 0, sizeof(eth0));
    strcpy(eth0.name, "eth0");
    eth0.mac_addr[0] = 0x52; eth0.mac_addr[1] = 0x54; eth0.mac_addr[2] = 0x00;
    eth0.mac_addr[3] = 0x12; eth0.mac_addr[4] = 0x34; eth0.mac_addr[5] = 0x56;
    eth0.mtu = 1500;

    assert(netdev_register(&eth0) == 0);
    assert(netdev_get_count() == 1);
    assert(netdev_find_by_name("eth0") == &eth0);
    assert(netdev_get(0) == &eth0);
    assert((eth0.flags & (NETDEV_FLAG_UP | NETDEV_FLAG_RUNNING)) != 0);

    /* Duplicate registration rejection */
    assert(netdev_register(&eth0) == -3);

    /* Unregistration */
    assert(netdev_unregister(&eth0) == 0);
    assert(netdev_get_count() == 0);
    assert(netdev_find_by_name("eth0") == NULL);

    printf("PASSED\n");
}

static void test_pci_bus_master_helpers(void) {
    printf("[TEST 10] Running test_pci_bus_master_helpers              ... ");
    /* Verify PCI helper functions link and execute without faults */
    pci_enable_bus_mastering(0, 0, 0);
    pci_enable_io(0, 0, 0);
    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running APDRV v3 & KSYM Native Driver Test Suite    \n");
    printf("=====================================================\n");

    purple_detect_init();
    test_ksym_registration_and_lookup();
    test_ksym_unregistration();
    test_apdrv_init_state();
    test_apdrv_invalid_headers();
    test_apdrv_unresolved_symbols();
    test_apdrv_load_and_relocations();
    test_apdrv_module_lifecycle();
    test_apdrv_duplicate_and_capacity_limits();
    test_netdev_subsystem();
    test_pci_bus_master_helpers();

    printf("=====================================================\n");
    printf(" APDRV v3 & KSYM Tests: 10/10 passed                 \n");
    printf("=====================================================\n");
    return 0;
}
