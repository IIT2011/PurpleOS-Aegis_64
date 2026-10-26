#include "syscall.h"
#include "gdt.h"
#include "idt.h"
#include "vmm.h"
#include "pmm.h"
#include "task.h"
#include "pit.h"
#include "string.h"
#include "serial.h"
#include "purple_abi.h"
#include "io.h"

#define MSR_EFER    0xC0000080
#define MSR_STAR    0xC0000081
#define MSR_LSTAR   0xC0000082
#define MSR_SFMASK  0xC0000084
#define EFER_SCE    0x00000001

extern void syscall_entry_asm(void);

int syscall_validate_user_ptr(const void *ptr, size_t len, int write_access) {
    if (!ptr) return SYS_ERR_FAULT;
    if (len == 0) return SYS_OK;

    uint64_t start = (uint64_t)(uintptr_t)ptr;
    uint64_t end = start + len;

    /* Check for arithmetic overflow */
    if (end < start) return SYS_ERR_FAULT;

    /* Enforce strict canonical user space range */
    if (start < USER_MIN_VADDR || end > USER_MAX_VADDR) {
        return SYS_ERR_FAULT;
    }

#ifndef __KERNEL_HOST_TEST__
    page_table_t *pml4 = vmm_get_kernel_pml4();
    if (!pml4) return SYS_ERR_FAULT;

    /* Verify each page in the requested range is user-accessible */
    uint64_t page_start = start & ~0xFFFULL;
    uint64_t page_end = (end + 0xFFFULL) & ~0xFFFULL;

    for (uint64_t va = page_start; va < page_end; va += PAGE_SIZE_4K) {
        uint64_t paddr = 0;
        uint64_t flags = 0;
        if (vmm_get_mapping(pml4, va, &paddr, &flags) != VMM_OK) {
            return SYS_ERR_FAULT;
        }
        if (!(flags & PAGE_PRESENT) || !(flags & PAGE_USER)) {
            return SYS_ERR_FAULT;
        }
        if (write_access && !(flags & PAGE_WRITABLE)) {
            return SYS_ERR_FAULT;
        }
    }
#else
    (void)write_access;
#endif

    return SYS_OK;
}

int syscall_validate_user_string(const char *str, size_t max_len) {
    if (!str) return SYS_ERR_FAULT;
    uint64_t addr = (uint64_t)(uintptr_t)str;
    if (addr < USER_MIN_VADDR || addr >= USER_MAX_VADDR) {
        return SYS_ERR_FAULT;
    }

    size_t len = 0;
    while (len < max_len) {
        uint64_t cur = addr + len;
        if (cur >= USER_MAX_VADDR) return SYS_ERR_FAULT;
        if (str[len] == '\0') return SYS_OK;
        len++;
    }
    return SYS_ERR_INVAL;
}

static void syscall_isr_handler(interrupt_frame_t *frame) {
    frame->rax = (uint64_t)syscall_dispatch(frame->rax, frame->rdi, frame->rsi, frame->rdx, frame->r10, frame->r8, frame->r9);
}

void syscall_init(void) {
    /* 1. Register IDT Gate 128 (0x80) for Ring 3 software interrupt dispatch */
    register_interrupt_handler(128, syscall_isr_handler);

#ifndef __KERNEL_HOST_TEST__
    /* 2. Configure MSRs for fast hardware syscall/sysret instruction */
    uint64_t efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);

    /* STAR: [63:48] User CS base selector = 0x10 (SYSRET loads SS=0x18|3=0x1B, CS=0x20|3=0x23)
     *       [47:32] Kernel CS selector = 0x08 (SYSCALL loads CS=0x08, SS=0x10)
     */
    uint64_t star = (0x0010ULL << 48) | (0x0008ULL << 32);
    wrmsr(MSR_STAR, star);

    /* LSTAR: 64-bit target RIP for syscall instruction */
    wrmsr(MSR_LSTAR, (uint64_t)(uintptr_t)syscall_entry_asm);

    /* SFMASK: Clear IF (0x200) to disable interrupts upon syscall entry */
    wrmsr(MSR_SFMASK, 0x200ULL);
#endif

    kinfo("Syscall ABI Initialized: int 0x80 & SYSCALL/SYSRET active");
}

void syscall_ap_init(void) {
#ifndef __KERNEL_HOST_TEST__
    uint64_t efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);
    uint64_t star = (0x0010ULL << 48) | (0x0008ULL << 32);
    wrmsr(MSR_STAR, star);
    wrmsr(MSR_LSTAR, (uint64_t)(uintptr_t)syscall_entry_asm);
    wrmsr(MSR_SFMASK, 0x200ULL);
#endif
}

int64_t syscall_dispatch(uint64_t sys_no, uint64_t a1, uint64_t a2, uint64_t a3, uint64_t a4, uint64_t a5, uint64_t a6) {
    (void)a5;
    (void)a6;

    switch (sys_no) {
        case SYS_EXIT:
            task_exit();
            return SYS_OK;

        case SYS_THREAD_YIELD:
            task_yield();
            return SYS_OK;

        case SYS_SLEEP_MS:
            task_sleep((uint32_t)(a1 / 10));
            return SYS_OK;

        case SYS_GET_TICKS:
            return (int64_t)pit_get_ticks();

        case SYS_GET_TIME_MS:
            return (int64_t)(pit_get_ticks() * 10);

        case SYS_GET_WALLCLOCK: {
            if (syscall_validate_user_ptr((void *)a1, sizeof(purple_time_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_get_wall_clock((purple_time_t *)a1);
        }

        case SYS_MMAP: {
            /* a1 = hint, a2 = length, a3 = prot, a4 = flags */
            size_t len = (size_t)a2;
            if (len == 0 || len > 64 * 1024 * 1024) return SYS_ERR_INVAL;

            size_t pages = (len + PAGE_SIZE_4K - 1) / PAGE_SIZE_4K;
            uint64_t vaddr = a1 ? (a1 & ~0xFFFULL) : USER_MIN_VADDR + 0x10000000ULL;
            if (vaddr < USER_MIN_VADDR || (vaddr + pages * PAGE_SIZE_4K) > USER_MAX_VADDR) {
                return SYS_ERR_FAULT;
            }

            page_table_t *pml4 = vmm_get_kernel_pml4();
            uint64_t map_flags = PAGE_PRESENT | PAGE_USER;
            if (a3 & 2) map_flags |= PAGE_WRITABLE;

            for (size_t i = 0; i < pages; i++) {
                void *frame = pmm_alloc_frame();
                if (!frame) return SYS_ERR_NOMEM;
                memset(frame, 0, PAGE_SIZE_4K);
                vmm_map_page(pml4, vaddr + i * PAGE_SIZE_4K, (uint64_t)(uintptr_t)frame, map_flags);
            }
            return (int64_t)vaddr;
        }

        case SYS_MUNMAP: {
            uint64_t vaddr = a1 & ~0xFFFULL;
            size_t len = (size_t)a2;
            size_t pages = (len + PAGE_SIZE_4K - 1) / PAGE_SIZE_4K;
            page_table_t *pml4 = vmm_get_kernel_pml4();
            for (size_t i = 0; i < pages; i++) {
                vmm_unmap_page(pml4, vaddr + i * PAGE_SIZE_4K);
            }
            return SYS_OK;
        }

        case SYS_OPEN: {
            if (syscall_validate_user_string((const char *)a1, 256) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            purple_file_t *f = purple_file_open((const char *)a1, (uint32_t)a2);
            return f ? (int64_t)(uintptr_t)f : SYS_ERR_NOENT;
        }

        case SYS_READ: {
            if (!a1) return SYS_ERR_INVAL;
            if (syscall_validate_user_ptr((void *)a2, (size_t)a3, 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_file_read((purple_file_t *)a1, (void *)a2, (size_t)a3);
        }

        case SYS_WRITE: {
            if (!a1) return SYS_ERR_INVAL;
            if (syscall_validate_user_ptr((const void *)a2, (size_t)a3, 0) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_file_write((purple_file_t *)a1, (const void *)a2, (size_t)a3);
        }

        case SYS_SEEK: {
            if (!a1) return SYS_ERR_INVAL;
            return purple_file_seek((purple_file_t *)a1, (int64_t)a2, (int)a3);
        }

        case SYS_CLOSE: {
            if (!a1) return SYS_ERR_INVAL;
            return purple_file_close((purple_file_t *)a1);
        }

        case SYS_STAT: {
            if (syscall_validate_user_string((const char *)a1, 256) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            if (syscall_validate_user_ptr((void *)a2, sizeof(purple_stat_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_file_stat((const char *)a1, (purple_stat_t *)a2);
        }

        case SYS_EXISTS: {
            if (syscall_validate_user_string((const char *)a1, 256) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_file_exists((const char *)a1);
        }

        case SYS_DIR_OPEN: {
            if (syscall_validate_user_string((const char *)a1, 256) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            purple_dir_t *d = purple_dir_open((const char *)a1);
            return d ? (int64_t)(uintptr_t)d : SYS_ERR_NOENT;
        }

        case SYS_DIR_READ: {
            if (!a1) return SYS_ERR_INVAL;
            if (syscall_validate_user_ptr((void *)a2, sizeof(purple_dirent_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_dir_read((purple_dir_t *)a1, (purple_dirent_t *)a2);
        }

        case SYS_DIR_CLOSE: {
            if (!a1) return SYS_ERR_INVAL;
            return purple_dir_close((purple_dir_t *)a1);
        }

        case SYS_WINDOW_CREATE: {
            if (syscall_validate_user_string((const char *)a1, 128) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            purple_window_t *w = purple_window_create((const char *)a1, (uint32_t)a2, (uint32_t)a3, (uint32_t)a4, (uint32_t)a5);
            return w ? (int64_t)(uintptr_t)w : SYS_ERR_NOMEM;
        }

        case SYS_WINDOW_DESTROY: {
            if (!a1) return SYS_ERR_INVAL;
            return purple_window_destroy((purple_window_t *)a1);
        }

        case SYS_WINDOW_SURFACE: {
            if (!a1) return SYS_ERR_INVAL;
            if (syscall_validate_user_ptr((void *)a2, sizeof(purple_surface_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_window_get_surface((purple_window_t *)a1, (purple_surface_t *)a2);
        }

        case SYS_WINDOW_PRESENT: {
            if (!a1) return SYS_ERR_INVAL;
            return purple_window_present((purple_window_t *)a1);
        }

        case SYS_WINDOW_TITLE: {
            if (!a1) return SYS_ERR_INVAL;
            if (syscall_validate_user_string((const char *)a2, 128) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_window_set_title((purple_window_t *)a1, (const char *)a2);
        }

        case SYS_POLL_EVENT: {
            if (syscall_validate_user_ptr((void *)a1, sizeof(purple_event_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_poll_event((purple_event_t *)a1);
        }

        case SYS_WAIT_EVENT: {
            if (syscall_validate_user_ptr((void *)a1, sizeof(purple_event_t), 1) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_wait_event((purple_event_t *)a1, (uint32_t)a2);
        }

        case SYS_AUDIO_AVAILABLE:
            return purple_audio_is_available();

        case SYS_AUDIO_PLAY_PCM: {
            if (syscall_validate_user_ptr((const void *)a1, (size_t)a2, 0) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            return purple_audio_play_pcm((const void *)a1, (size_t)a2, (uint32_t)a3, (uint8_t)a4);
        }

        case SYS_AUDIO_SET_VOL:
            purple_audio_set_volume((uint8_t)a1);
            return SYS_OK;

        case SYS_PRINT: {
            if (syscall_validate_user_string((const char *)a1, 1024) != SYS_OK) {
                return SYS_ERR_FAULT;
            }
            kinfo("[USER APP] %s", (const char *)a1);
            return SYS_OK;
        }

        default:
            return SYS_ERR_NOSYS;
    }
}
