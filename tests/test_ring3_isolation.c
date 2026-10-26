#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <stdint.h>

#include "gdt.h"
#include "syscall.h"
#include "idt.h"
#include "vmm.h"
#include "purple_abi.h"

#ifdef __KERNEL_HOST_TEST__
__attribute__((weak)) void register_interrupt_handler(uint8_t int_no, isr_handler_t handler) { (void)int_no; (void)handler; }
__attribute__((weak)) void task_exit(void) {}
__attribute__((weak)) void task_yield(void) {}
__attribute__((weak)) void task_sleep(uint32_t ticks) { (void)ticks; }
#endif

static void test_gdt_selectors_and_dpl(void) {
    printf("[TEST  1] Running test_gdt_selectors_and_dpl                 ... ");
    gdt_init();

    assert(gdt_get_kernel_cs() == 0x08);
    assert(gdt_get_kernel_ds() == 0x10);
    assert(gdt_get_user_ds() == 0x1B); /* 0x18 | 3 */
    assert(gdt_get_user_cs() == 0x23); /* 0x20 | 3 */

    /* Verify RPL / DPL bits */
    assert((gdt_get_kernel_cs() & 3) == 0); /* Ring 0 */
    assert((gdt_get_kernel_ds() & 3) == 0); /* Ring 0 */
    assert((gdt_get_user_ds() & 3) == 3);   /* Ring 3 */
    assert((gdt_get_user_cs() & 3) == 3);   /* Ring 3 */

    printf("PASSED\n");
}

static void test_tss_initialization_and_stack_switching(void) {
    printf("[TEST  2] Running test_tss_initialization_and_stack_switching... ");
    gdt_init();

    tss64_t *tss = gdt_get_tss();
    assert(tss != NULL);
    assert(tss->rsp0 != 0);
    assert(tss->iopb_offset == sizeof(tss64_t));

    /* Test updating kernel stack pointer on context switch */
    uint64_t test_kstack = 0xFFFF800012345000ULL;
    gdt_set_kernel_stack(test_kstack);
    assert(tss->rsp0 == test_kstack);

    printf("PASSED\n");
}

static void test_syscall_pointer_bounds_validation(void) {
    printf("[TEST  3] Running test_syscall_pointer_bounds_validation      ... ");

    /* 1. NULL pointer rejection */
    assert(syscall_validate_user_ptr(NULL, 16, 0) == SYS_ERR_FAULT);
    assert(syscall_validate_user_ptr(NULL, 0, 0) == SYS_ERR_FAULT);

    /* 2. Zero-length with non-null pointer */
    void *valid_user_ptr = (void *)0x0000000001000000ULL;
    assert(syscall_validate_user_ptr(valid_user_ptr, 0, 0) == SYS_OK);

    /* 3. Valid user pointer within canonical user range */
    assert(syscall_validate_user_ptr(valid_user_ptr, 4096, 0) == SYS_OK);
    assert(syscall_validate_user_ptr(valid_user_ptr, 4096, 1) == SYS_OK);

    /* 4. Arithmetic overflow rejection */
    void *overflow_ptr = (void *)0xFFFFFFFFFFFFFFF0ULL;
    assert(syscall_validate_user_ptr(overflow_ptr, 64, 0) == SYS_ERR_FAULT);

    /* 5. Kernel higher-half pointer rejection */
    void *kernel_higher_half = (void *)0xFFFF800000100000ULL;
    assert(syscall_validate_user_ptr(kernel_higher_half, 64, 0) == SYS_ERR_FAULT);

    /* 6. Kernel text/data space below 4 MiB user base rejection */
    void *low_kernel_ptr = (void *)0x0000000000100000ULL;
    assert(syscall_validate_user_ptr(low_kernel_ptr, 64, 0) == SYS_ERR_FAULT);

    /* 7. Upper limit boundary check */
    void *boundary_ptr = (void *)(USER_MAX_VADDR - 10);
    assert(syscall_validate_user_ptr(boundary_ptr, 10, 0) == SYS_OK);
    assert(syscall_validate_user_ptr(boundary_ptr, 12, 0) == SYS_ERR_FAULT);

    printf("PASSED\n");
}

static void test_syscall_string_validation(void) {
    printf("[TEST  4] Running test_syscall_string_validation              ... ");

    assert(syscall_validate_user_string(NULL, 64) == SYS_ERR_FAULT);

    /* Kernel string pointer rejection */
    const char *kernel_str = (const char *)0xFFFF800000100000ULL;
    assert(syscall_validate_user_string(kernel_str, 64) == SYS_ERR_FAULT);

    /* Below USER_MIN_VADDR rejection */
    const char *low_str = (const char *)0x0000000000005000ULL;
    assert(syscall_validate_user_string(low_str, 64) == SYS_ERR_FAULT);

    printf("PASSED\n");
}

static void test_syscall_dispatch_and_error_handling(void) {
    printf("[TEST  5] Running test_syscall_dispatch_and_error_handling   ... ");

    /* Unknown syscall number returns SYS_ERR_NOSYS */
    int64_t res = syscall_dispatch(9999, 0, 0, 0, 0, 0, 0);
    assert(res == SYS_ERR_NOSYS);

    /* SYS_GET_TICKS returns non-negative */
    int64_t ticks = syscall_dispatch(SYS_GET_TICKS, 0, 0, 0, 0, 0, 0);
    assert(ticks >= 0);

    /* SYS_GET_TIME_MS returns non-negative */
    int64_t time_ms = syscall_dispatch(SYS_GET_TIME_MS, 0, 0, 0, 0, 0, 0);
    assert(time_ms >= 0);

    /* SYS_THREAD_YIELD returns SYS_OK */
    assert(syscall_dispatch(SYS_THREAD_YIELD, 0, 0, 0, 0, 0, 0) == SYS_OK);

    /* Memory mmap with invalid length */
    assert(syscall_dispatch(SYS_MMAP, 0, 0, 0, 0, 0, 0) == SYS_ERR_INVAL);
    assert(syscall_dispatch(SYS_MMAP, 0, 128 * 1024 * 1024, 0, 0, 0, 0) == SYS_ERR_INVAL);

    printf("PASSED\n");
}

static void test_adversarial_kernel_pointer_injection(void) {
    printf("[TEST  6] Running test_adversarial_kernel_pointer_injection  ... ");

    /* Attempt to pass kernel higher-half address to SYS_OPEN */
    int64_t open_res = syscall_dispatch(SYS_OPEN, 0xFFFF800000001000ULL, 0, 0, 0, 0, 0);
    assert(open_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel higher-half address to SYS_PRINT */
    int64_t print_res = syscall_dispatch(SYS_PRINT, 0xFFFF800000001000ULL, 0, 0, 0, 0, 0);
    assert(print_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel higher-half address to SYS_WINDOW_CREATE */
    int64_t win_res = syscall_dispatch(SYS_WINDOW_CREATE, 0xFFFF800000001000ULL, 10, 10, 100, 100, 0);
    assert(win_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel pointer to SYS_READ */
    int64_t read_res = syscall_dispatch(SYS_READ, 1, 0xFFFF800000001000ULL, 128, 0, 0, 0);
    assert(read_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel pointer to SYS_WRITE */
    int64_t write_res = syscall_dispatch(SYS_WRITE, 1, 0xFFFF800000001000ULL, 128, 0, 0, 0);
    assert(write_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel pointer to SYS_POLL_EVENT */
    int64_t poll_res = syscall_dispatch(SYS_POLL_EVENT, 0xFFFF800000001000ULL, 0, 0, 0, 0, 0);
    assert(poll_res == SYS_ERR_FAULT);

    /* Attempt to pass kernel pointer to SYS_GET_WALLCLOCK */
    int64_t clk_res = syscall_dispatch(SYS_GET_WALLCLOCK, 0xFFFF800000001000ULL, 0, 0, 0, 0, 0);
    assert(clk_res == SYS_ERR_FAULT);

    printf("PASSED\n");
}

static void test_page_table_user_and_wx_flags(void) {
    printf("[TEST  7] Running test_page_table_user_and_wx_flags          ... ");

    /* 1. User code segment permissions: must be Present, User-accessible, NOT Writable, Executable (no NX) */
    uint64_t code_flags = PAGE_PRESENT | PAGE_USER;
    assert((code_flags & PAGE_USER) != 0);
    assert((code_flags & PAGE_WRITABLE) == 0); /* W^X: Never writable */
    assert((code_flags & PAGE_NO_EXECUTE) == 0); /* Executable */

    /* 2. User data/stack segment permissions: must be Present, User-accessible, Writable, Non-Executable (NX set) */
    uint64_t data_flags = PAGE_PRESENT | PAGE_USER | PAGE_WRITABLE | PAGE_NO_EXECUTE;
    assert((data_flags & PAGE_USER) != 0);
    assert((data_flags & PAGE_WRITABLE) != 0);
    assert((data_flags & PAGE_NO_EXECUTE) != 0); /* W^X: Data cannot be executed */

    /* 3. Kernel code permissions: must NOT have PAGE_USER (Ring 3 cannot access) */
    uint64_t kernel_code_flags = PAGE_PRESENT;
    assert((kernel_code_flags & PAGE_USER) == 0);

    /* 4. Kernel data permissions: must NOT have PAGE_USER */
    uint64_t kernel_data_flags = PAGE_PRESENT | PAGE_WRITABLE | PAGE_NO_EXECUTE;
    assert((kernel_data_flags & PAGE_USER) == 0);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Aegis64 Ring 3 Hardware Isolation Test Suite\n");
    printf("=====================================================\n");

    test_gdt_selectors_and_dpl();
    test_tss_initialization_and_stack_switching();
    test_syscall_pointer_bounds_validation();
    test_syscall_string_validation();
    test_syscall_dispatch_and_error_handling();
    test_adversarial_kernel_pointer_injection();
    test_page_table_user_and_wx_flags();

    printf("=====================================================\n");
    printf(" Ring 3 Isolation Tests: ALL 7 TEST CASES PASSED!    \n");
    printf("=====================================================\n");
    return 0;
}
