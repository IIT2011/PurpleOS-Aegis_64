#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <sys/mman.h>

#include "apexec.h"
#include "vmm.h"
#include "pmm.h"
#include "multiboot.h"

static void setup_test_env(void) {
    void *backing = mmap((void *)0x1000000, 48 * 1024 * 1024, PROT_READ|PROT_WRITE,
                         MAP_PRIVATE|MAP_ANONYMOUS|MAP_FIXED, -1, 0);
    if (backing == MAP_FAILED) {
        perror("Failed to map host test physical memory backing");
        exit(1);
    }

    void *mem = mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, -1, 0);
    struct multiboot_info *mbi = (struct multiboot_info *)mem;
    struct multiboot_mmap_entry *mmap_entries = (struct multiboot_mmap_entry *)((uint8_t *)mem + sizeof(struct multiboot_info));

    mbi->flags = MULTIBOOT_INFO_MEM_MAP;
    mmap_entries[0].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[0].addr = 0x0;
    mmap_entries[0].len  = 0x1000000;
    mmap_entries[0].type = MULTIBOOT_MEMORY_RESERVED;

    mmap_entries[1].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[1].addr = 0x1000000;
    mmap_entries[1].len  = 0x3000000;
    mmap_entries[1].type = MULTIBOOT_MEMORY_AVAILABLE;

    mbi->mmap_addr = (uint32_t)(uintptr_t)mmap_entries;
    mbi->mmap_length = sizeof(struct multiboot_mmap_entry) * 2;

    pmm_init(mbi, 0x100000, 0x200000);
}

static void test_apex_validation_valid_binary(void) {
    printf("[TEST  1] Running test_apex_validation_valid_binary          ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    assert(apex_validate(buf, sz) == APEX_OK);

    apex_header_t hdr;
    assert(apex_get_header(buf, sz, &hdr) == APEX_OK);
    assert(hdr.magic == APEX_MAGIC);
    assert(hdr.version == APEX_VERSION_CURRENT);
    assert(hdr.arch == APEX_ARCH_X86_64);
    assert(hdr.abi_version == APEX_ABI_PURPLE_V1);
    assert(hdr.entry_point == 0x0000000000401000ULL);
    assert(hdr.num_segments == 2);

    apex_segment_t seg0, seg1;
    assert(apex_get_segment(buf, sz, 0, &seg0) == APEX_OK);
    assert(seg0.type == APEX_SEG_CODE);
    assert(seg0.flags == (APEX_FLAG_READ | APEX_FLAG_EXEC));
    assert(seg0.vaddr == 0x0000000000401000ULL);

    assert(apex_get_segment(buf, sz, 1, &seg1) == APEX_OK);
    assert(seg1.type == APEX_SEG_RODATA);
    assert(seg1.flags == APEX_FLAG_READ);
    assert(seg1.vaddr == 0x0000000000402000ULL);

    free(buf);
    printf("PASSED\n");
}

static void test_apex_malformed_rejection(void) {
    printf("[TEST  2] Running test_apex_malformed_rejection              ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    /* 1. NULL pointer */
    assert(apex_validate(NULL, sz) == APEX_ERR_NULL_PTR);

    /* 2. Truncated file */
    assert(apex_validate(buf, 32) == APEX_ERR_TOO_SMALL);

    /* 3. Bad magic */
    uint8_t bad_magic[256];
    memcpy(bad_magic, buf, sz);
    bad_magic[0] = 'X';
    assert(apex_validate(bad_magic, sz) == APEX_ERR_BAD_MAGIC);

    /* 4. Bad version */
    uint8_t bad_ver[256];
    memcpy(bad_ver, buf, sz);
    bad_ver[4] = 99;
    assert(apex_validate(bad_ver, sz) == APEX_ERR_BAD_VERSION);

    /* 5. Bad architecture */
    uint8_t bad_arch[256];
    memcpy(bad_arch, buf, sz);
    bad_arch[6] = 0x32; bad_arch[7] = 0x86;
    assert(apex_validate(bad_arch, sz) == APEX_ERR_BAD_ARCH);

    free(buf);
    printf("PASSED\n");
}

static void test_apex_security_wx_enforcement(void) {
    printf("[TEST  3] Running test_apex_security_wx_enforcement          ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    /* Corrupt segment 0 flags to have both WRITE and EXEC */
    apex_segment_t *s0 = (apex_segment_t *)(buf + sizeof(apex_header_t));
    s0->flags = APEX_FLAG_READ | APEX_FLAG_WRITE | APEX_FLAG_EXEC;

    assert(apex_validate(buf, sz) == APEX_ERR_SECURITY_WX);

    free(buf);
    printf("PASSED\n");
}

static void test_apex_kernel_space_vaddr_rejection(void) {
    printf("[TEST  4] Running test_apex_kernel_space_vaddr_rejection     ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    /* 1. Higher-half address attempt */
    apex_segment_t *s0 = (apex_segment_t *)(buf + sizeof(apex_header_t));
    s0->vaddr = 0xFFFF800000401000ULL;
    assert(apex_validate(buf, sz) == APEX_ERR_INVALID_VADDR);

    /* 2. Below USER_MIN_VADDR attempt */
    s0->vaddr = 0x0000000000005000ULL;
    assert(apex_validate(buf, sz) == APEX_ERR_INVALID_VADDR);

    free(buf);
    printf("PASSED\n");
}

static void test_apex_invalid_entry_point_rejection(void) {
    printf("[TEST  5] Running test_apex_invalid_entry_point_rejection    ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    apex_header_t *hdr = (apex_header_t *)buf;

    /* Entry point points to non-executable RODATA segment */
    hdr->entry_point = 0x0000000000402000ULL;
    assert(apex_validate(buf, sz) == APEX_ERR_INVALID_ENTRY);

    /* Entry point points to unmapped address */
    hdr->entry_point = 0x0000000000500000ULL;
    assert(apex_validate(buf, sz) == APEX_ERR_INVALID_ENTRY);

    free(buf);
    printf("PASSED\n");
}

static void test_apex_loader_execution(void) {
    printf("[TEST  6] Running test_apex_loader_execution                 ... ");

    FILE *f = fopen("build/apps/sample.apexec", "rb");
    assert(f != NULL);
    fseek(f, 0, SEEK_END);
    size_t sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    uint8_t *buf = malloc(sz);
    assert(buf != NULL);
    assert(fread(buf, 1, sz, f) == sz);
    fclose(f);

    page_table_t pml4;
    memset(&pml4, 0, sizeof(pml4));

    apex_binary_info_t info;
    int rc = apex_load(&pml4, buf, sz, &info);
    assert(rc == APEX_OK);
    assert(info.entry_point == 0x0000000000401000ULL);
    assert(info.stack_top == 0x00007FFFF0000000ULL);
    assert(info.stack_size == 65536);
    assert(info.min_vaddr == 0x0000000000401000ULL);
    assert(info.max_vaddr == 0x0000000000403000ULL);

    free(buf);
    printf("PASSED\n");
}

int main(void) {
    setup_test_env();
    printf("=====================================================\n");
    printf(" Running Purple OS Native .apexec Loader Test Suite  \n");
    printf("=====================================================\n");

    test_apex_validation_valid_binary();
    test_apex_malformed_rejection();
    test_apex_security_wx_enforcement();
    test_apex_kernel_space_vaddr_rejection();
    test_apex_invalid_entry_point_rejection();
    test_apex_loader_execution();

    printf("=====================================================\n");
    printf(" .apexec Loader Tests: ALL 6 TEST CASES PASSED!      \n");
    printf("=====================================================\n");
    return 0;
}
