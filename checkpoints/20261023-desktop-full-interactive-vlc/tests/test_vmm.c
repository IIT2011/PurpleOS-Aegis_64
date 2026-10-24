#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#include "vmm.h"
#include "pmm.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %d] Running %s... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        printf("PASSED\n"); \
        tests_passed++; \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while (0)

static void setup_test_env(void) {
    /* Map backing physical memory space 16 MiB .. 64 MiB */
    void *backing = mmap((void *)0x1000000, 48 * 1024 * 1024, PROT_READ|PROT_WRITE,
                         MAP_PRIVATE|MAP_ANONYMOUS|MAP_FIXED, -1, 0);
    if (backing == MAP_FAILED) {
        perror("Failed to map host test physical memory backing");
        exit(1);
    }

    void *mem = mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, -1, 0);
    struct multiboot_info *mbi = (struct multiboot_info *)mem;
    struct multiboot_mmap_entry *mmap = (struct multiboot_mmap_entry *)((uint8_t *)mem + sizeof(struct multiboot_info));

    mbi->flags = MULTIBOOT_INFO_MEM_MAP;

    mmap[0].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap[0].addr = 0x0;
    mmap[0].len  = 0x1000000;
    mmap[0].type = MULTIBOOT_MEMORY_RESERVED;

    mmap[1].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap[1].addr = 0x1000000;
    mmap[1].len  = 0x3000000; // 48 MiB
    mmap[1].type = MULTIBOOT_MEMORY_AVAILABLE;

    mbi->mmap_addr = (uint32_t)(uintptr_t)mmap;
    mbi->mmap_length = sizeof(struct multiboot_mmap_entry) * 2;

    pmm_init(mbi, 0x100000, 0x200000);
}

static int test_vmm_canonical_check(void) {
    if (!vmm_is_canonical(0x0000000000000000ULL)) return 0;
    if (!vmm_is_canonical(0x00007FFFFFFFFFFFULL)) return 0;
    if (!vmm_is_canonical(0xFFFF800000000000ULL)) return 0;
    if (!vmm_is_canonical(0xFFFFFFFFFFFFFFFFULL)) return 0;

    // Non-canonical addresses (bits 47..63 differ)
    if (vmm_is_canonical(0x0000800000000000ULL)) return 0;
    if (vmm_is_canonical(0xFFFF7FFFFFFFFFFFULL)) return 0;

    return 1;
}

static int test_vmm_map_and_lookup(void) {
    page_table_t *pml4 = vmm_create_pml4();
    if (!pml4) return 0;

    uint64_t vaddr = 0x40000000; // 1 GiB
    uint64_t paddr = 0x2000000;  // 32 MiB
    uint64_t flags = PAGE_WRITABLE | PAGE_NO_EXECUTE;

    int res = vmm_map_page(pml4, vaddr, paddr, flags);
    if (res != VMM_OK) return 0;

    uint64_t out_p = 0, out_f = 0;
    res = vmm_get_mapping(pml4, vaddr, &out_p, &out_f);
    if (res != VMM_OK) return 0;
    if (out_p != paddr) return 0;
    if (!(out_f & PAGE_WRITABLE)) return 0;
    if (!(out_f & PAGE_NO_EXECUTE)) return 0;

    return 1;
}

static int test_vmm_unmap(void) {
    page_table_t *pml4 = vmm_create_pml4();
    if (!pml4) return 0;

    uint64_t vaddr = 0x50000000;
    uint64_t paddr = 0x2100000;

    vmm_map_page(pml4, vaddr, paddr, PAGE_WRITABLE);
    int res = vmm_unmap_page(pml4, vaddr);
    if (res != VMM_OK) return 0;

    uint64_t out_p = 0, out_f = 0;
    res = vmm_get_mapping(pml4, vaddr, &out_p, &out_f);
    if (res != VMM_ERR_NOT_MAPPED) return 0;

    return 1;
}

static int test_vmm_huge_page_splitting(void) {
    page_table_t *pml4 = vmm_create_pml4();
    if (!pml4) return 0;

    /* Create mock 2 MiB huge page */
    page_table_t *pdpt = (page_table_t *)pmm_alloc_frame();
    page_table_t *pd = (page_table_t *)pmm_alloc_frame();
    memset(pdpt, 0, sizeof(page_table_t));
    memset(pd, 0, sizeof(page_table_t));

    pml4->entries[0] = (uint64_t)(uintptr_t)pdpt | PAGE_PRESENT | PAGE_WRITABLE;
    pdpt->entries[0] = (uint64_t)(uintptr_t)pd | PAGE_PRESENT | PAGE_WRITABLE;

    /* Map 0..2 MiB as huge page */
    pd->entries[0] = 0x00000000 | PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE;

    /* Now trigger split via vmm_split_huge_page */
    int res = vmm_split_huge_page(pml4, 0x1000); // 4 KiB address inside 2 MiB page
    if (res != VMM_OK) return 0;

    /* PD entry should no longer have PAGE_HUGE */
    if (pd->entries[0] & PAGE_HUGE) return 0;

    /* Lookup should succeed for 4 KiB address */
    uint64_t out_p = 0, out_f = 0;
    res = vmm_get_mapping(pml4, 0x1000, &out_p, &out_f);
    if (res != VMM_OK) return 0;
    if (out_p != 0x1000) return 0;

    return 1;
}

static int test_vmm_wx_enforcement(void) {
    page_table_t *pml4 = vmm_create_pml4();
    if (!pml4) return 0;

    uint64_t code_vaddr = 0x100000;
    uint64_t data_vaddr = 0x200000;

    /* Executable code mapping: RX (no write, no NX) */
    vmm_map_page(pml4, code_vaddr, 0x100000, VMM_PROT_READ | VMM_PROT_EXEC);

    /* Data mapping: RW NX (write, NX) */
    vmm_map_page(pml4, data_vaddr, 0x200000, VMM_PROT_READ | VMM_PROT_WRITE | VMM_PROT_NO_EXEC);

    uint64_t p = 0, f = 0;
    vmm_get_mapping(pml4, code_vaddr, &p, &f);
    if (f & PAGE_WRITABLE) return 0;    // Code must NOT be writable
    if (f & PAGE_NO_EXECUTE) return 0; // Code must be executable

    vmm_get_mapping(pml4, data_vaddr, &p, &f);
    if (!(f & PAGE_WRITABLE)) return 0;   // Data must be writable
    if (!(f & PAGE_NO_EXECUTE)) return 0; // Data must NOT be executable (NX)

    return 1;
}

int main(void) {
    printf("========================================\n");
    printf(" Running Aegis64 VMM Unit Tests\n");
    printf("========================================\n");

    setup_test_env();

    RUN_TEST(test_vmm_canonical_check);
    RUN_TEST(test_vmm_map_and_lookup);
    RUN_TEST(test_vmm_unmap);
    RUN_TEST(test_vmm_huge_page_splitting);
    RUN_TEST(test_vmm_wx_enforcement);

    printf("========================================\n");
    printf(" VMM Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("========================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
