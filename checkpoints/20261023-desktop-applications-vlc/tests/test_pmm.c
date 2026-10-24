#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <string.h>
#include <sys/mman.h>

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

static struct multiboot_info *create_mock_mbi(uint64_t ram_size) {
    void *mem = mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, -1, 0);
    struct multiboot_info *mbi = (struct multiboot_info *)mem;
    struct multiboot_mmap_entry *mmap_entries = (struct multiboot_mmap_entry *)((uint8_t *)mem + sizeof(struct multiboot_info));

    mbi->flags = MULTIBOOT_INFO_MEM_MAP;

    mmap_entries[0].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[0].addr = 0x0;
    mmap_entries[0].len  = 0x1000000; // First 16 MiB
    mmap_entries[0].type = MULTIBOOT_MEMORY_RESERVED;

    mmap_entries[1].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[1].addr = 0x1000000; // 16 MiB onwards
    mmap_entries[1].len  = ram_size - 0x1000000;
    mmap_entries[1].type = MULTIBOOT_MEMORY_AVAILABLE;

    mbi->mmap_addr = (uint32_t)(uintptr_t)mmap_entries;
    mbi->mmap_length = sizeof(struct multiboot_mmap_entry) * 2;

    return mbi;
}

static int test_pmm_initial_state(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000); // 64 MiB RAM
    pmm_init(mbi, 0x100000, 0x200000);

    size_t free_f = pmm_get_free_frames();
    if (free_f == 0) return 0;
    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_alloc_and_free(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    size_t initial_free = pmm_get_free_frames();
    void *f = pmm_alloc_frame();
    if (!f) return 0;
    if (pmm_get_free_frames() != initial_free - 1) return 0;

    int res = pmm_free_frame(f);
    if (res != PMM_OK) return 0;
    if (pmm_get_free_frames() != initial_free) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_double_free_protection(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    void *f = pmm_alloc_frame();
    if (!f) return 0;

    int res1 = pmm_free_frame(f);
    if (res1 != PMM_OK) return 0;

    int res2 = pmm_free_frame(f);
    if (res2 != PMM_ERR_ALREADY_FREE) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_reserved_region_protection(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    /* Attempting to free address in reserved 0..16 MiB */
    int res = pmm_free_frame((void *)0x80000);
    if (res != PMM_ERR_RESERVED) return 0;

    res = pmm_free_frame((void *)0x0);
    if (res != PMM_ERR_RESERVED) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_unaligned_free_protection(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    int res = pmm_free_frame((void *)0x1000001);
    if (res != PMM_ERR_INVALID_PARAM) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_out_of_bounds_protection(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    int res = pmm_free_frame((void *)0x100000000ULL);
    if (res != PMM_ERR_OUT_OF_BOUNDS) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_contiguous_alloc(void) {
    struct multiboot_info *mbi = create_mock_mbi(0x4000000);
    pmm_init(mbi, 0x100000, 0x200000);

    size_t count = 4;
    void *f = pmm_alloc_frames(count);
    if (!f) return 0;

    int res = pmm_free_frames(f, count);
    if (res != PMM_OK) return 0;

    munmap(mbi, 8192);
    return 1;
}

static int test_pmm_exhaustion_safety(void) {
    /* Initialize with a small pool of 18 MiB = 16 MiB reserved + 2 MiB usable (512 frames) */
    struct multiboot_info *mbi = create_mock_mbi(0x1200000);
    pmm_init(mbi, 0x100000, 0x200000);

    size_t free_count = pmm_get_free_frames();
    if (free_count == 0) return 0;

    void **ptrs = (void **)malloc(sizeof(void *) * free_count);
    if (!ptrs) return 0;

    for (size_t i = 0; i < free_count; i++) {
        ptrs[i] = pmm_alloc_frame();
        if (!ptrs[i]) {
            for (size_t j = 0; j < i; j++) pmm_free_frame(ptrs[j]);
            free(ptrs);
            munmap(mbi, 8192);
            return 0;
        }
    }

    /* Next allocation MUST safely return NULL */
    void *exhausted = pmm_alloc_frame();
    if (exhausted != NULL) {
        free(ptrs);
        munmap(mbi, 8192);
        return 0;
    }

    /* Restore all frames */
    for (size_t i = 0; i < free_count; i++) {
        pmm_free_frame(ptrs[i]);
    }
    free(ptrs);
    munmap(mbi, 8192);

    return 1;
}

static int test_pmm_overflow_arithmetic(void) {
    void *mem = mmap(NULL, 4096, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, -1, 0);
    struct multiboot_info *mbi = (struct multiboot_info *)mem;
    struct multiboot_mmap_entry *mmap_entries = (struct multiboot_mmap_entry *)((uint8_t *)mem + sizeof(struct multiboot_info));

    mbi->flags = MULTIBOOT_INFO_MEM_MAP;

    mmap_entries[0].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[0].addr = 0xFFFFFFFF00000000ULL;
    mmap_entries[0].len  = 0x200000000ULL;
    mmap_entries[0].type = MULTIBOOT_MEMORY_AVAILABLE;

    mbi->mmap_addr = (uint32_t)(uintptr_t)mmap_entries;
    mbi->mmap_length = sizeof(struct multiboot_mmap_entry);

    /* Should handle overflow safely */
    pmm_init(mbi, 0x100000, 0x200000);
    munmap(mem, 4096);
    return 1;
}

int main(void) {
    printf("========================================\n");
    printf(" Running Aegis64 PMM Unit Tests (9 Cases)\n");
    printf("========================================\n");

    RUN_TEST(test_pmm_initial_state);
    RUN_TEST(test_pmm_alloc_and_free);
    RUN_TEST(test_pmm_double_free_protection);
    RUN_TEST(test_pmm_reserved_region_protection);
    RUN_TEST(test_pmm_unaligned_free_protection);
    RUN_TEST(test_pmm_out_of_bounds_protection);
    RUN_TEST(test_pmm_contiguous_alloc);
    RUN_TEST(test_pmm_exhaustion_safety);
    RUN_TEST(test_pmm_overflow_arithmetic);

    printf("========================================\n");
    printf(" PMM Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("========================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
