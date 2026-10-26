#include "gdt.h"
#include "string.h"
#include "serial.h"

/* GDT Structure: 7 quadwords = 56 bytes */
typedef struct {
    uint64_t entries[7];
} __attribute__((aligned(16))) gdt_table_t;

static gdt_table_t gdt;
static gdt_ptr_t   gdt_ptr;
static tss64_t     tss;

static uint8_t default_kernel_stack[16384] __attribute__((aligned(16)));

void gdt_init(void) {
    memset(&gdt, 0, sizeof(gdt));
    memset(&tss, 0, sizeof(tss));

    /* 0x00: Null Descriptor */
    gdt.entries[0] = 0x0000000000000000ULL;

    /* 0x08: 64-bit Kernel Code Segment (DPL 0, Exec/Read, Long Mode L=1, Limit=0xFFFFF, G=1) */
    gdt.entries[1] = 0x00AF9A000000FFFFULL;

    /* 0x10: 64-bit Kernel Data Segment (DPL 0, Read/Write, 32-bit D/B=1, Limit=0xFFFFF, G=1) */
    gdt.entries[2] = 0x00CF92000000FFFFULL;

    /* 0x18: 64-bit User Data Segment (DPL 3, Read/Write, 32-bit D/B=1, Limit=0xFFFFF, G=1) */
    gdt.entries[3] = 0x00CFF2000000FFFFULL;

    /* 0x20: 64-bit User Code Segment (DPL 3, Exec/Read, Long Mode L=1, Limit=0xFFFFF, G=1) */
    gdt.entries[4] = 0x00AFFA000000FFFFULL;

    /* 0x28: 64-bit Task State Segment (TSS) Descriptor (16 bytes = 2 entries) */
    uint64_t tss_base = (uint64_t)(uintptr_t)&tss;
    uint64_t tss_limit = sizeof(tss64_t) - 1;

    tss.rsp0 = (uint64_t)(uintptr_t)(default_kernel_stack + sizeof(default_kernel_stack));
    tss.iopb_offset = (uint16_t)sizeof(tss64_t);

    uint64_t tss_low = (tss_limit & 0xFFFF)
                     | ((tss_base & 0xFFFF) << 16)
                     | (((tss_base >> 16) & 0xFF) << 32)
                     | (0x89ULL << 40) /* Present=1, DPL=0, Type=0x9 (Available 64-bit TSS) */
                     | (((tss_limit >> 16) & 0x0F) << 48)
                     | (((tss_base >> 24) & 0xFF) << 56);

    uint64_t tss_high = (tss_base >> 32) & 0xFFFFFFFFULL;

    gdt.entries[5] = tss_low;
    gdt.entries[6] = tss_high;

    gdt_ptr.limit = sizeof(gdt) - 1;
    gdt_ptr.base  = (uint64_t)(uintptr_t)&gdt;

#ifndef __KERNEL_HOST_TEST__
    /* Load GDTR */
    __asm__ volatile("lgdt (%0)" : : "r"(&gdt_ptr) : "memory");

    /* Reload CS via 64-bit far return */
    __asm__ volatile(
        "pushq $0x08\n"
        "leaq 1f(%%rip), %%rax\n"
        "pushq %%rax\n"
        "lretq\n"
        "1:\n"
        : : : "rax", "memory"
    );

    /* Reload data segment registers */
    __asm__ volatile(
        "movw $0x10, %%ax\n"
        "movw %%ax, %%ds\n"
        "movw %%ax, %%es\n"
        "movw %%ax, %%ss\n"
        "movw %%ax, %%fs\n"
        "movw %%ax, %%gs\n"
        : : : "rax", "memory"
    );

    /* Load TSS into TR register */
    __asm__ volatile("ltr %%ax" : : "a"((uint16_t)GDT_TSS_SEL));
#endif

    kinfo("GDT & TSS Initialized: User Code (0x23), User Data (0x1B), TSS (0x28)");
}

void gdt_ap_init(void) {
#ifndef __KERNEL_HOST_TEST__
    __asm__ volatile("lgdt (%0)" : : "r"(&gdt_ptr) : "memory");

    __asm__ volatile(
        "pushq $0x08\n"
        "leaq 1f(%%rip), %%rax\n"
        "pushq %%rax\n"
        "lretq\n"
        "1:\n"
        : : : "rax", "memory"
    );

    __asm__ volatile(
        "movw $0x10, %%ax\n"
        "movw %%ax, %%ds\n"
        "movw %%ax, %%es\n"
        "movw %%ax, %%ss\n"
        "movw %%ax, %%fs\n"
        "movw %%ax, %%gs\n"
        : : : "rax", "memory"
    );
#endif
}

void gdt_set_kernel_stack(uint64_t rsp0) {
    tss.rsp0 = rsp0;
}

tss64_t *gdt_get_tss(void) {
    return &tss;
}

uint16_t gdt_get_user_cs(void) {
    return GDT_USER_CS;
}

uint16_t gdt_get_user_ds(void) {
    return GDT_USER_DS;
}

uint16_t gdt_get_kernel_cs(void) {
    return GDT_KERNEL_CS;
}

uint16_t gdt_get_kernel_ds(void) {
    return GDT_KERNEL_DS;
}

#ifndef __KERNEL_HOST_TEST__
__attribute__((naked)) void enter_user_mode(uint64_t user_rip, uint64_t user_rsp) {
    (void)user_rip;
    (void)user_rsp;
    __asm__ volatile (
        "cli\n"
        "movw $0x1B, %ax\n"
        "movw %ax, %ds\n"
        "movw %ax, %es\n"
        "movw %ax, %fs\n"
        "movw %ax, %gs\n"
        "pushq $0x1B\n"        /* SS: User Data (0x18 | 3) */
        "pushq %rsi\n"         /* RSP: User Stack */
        "pushq $0x202\n"       /* RFLAGS: IF=1, bit 1=1 */
        "pushq $0x23\n"        /* CS: User Code (0x20 | 3) */
        "pushq %rdi\n"         /* RIP: User Entry Point */
        "iretq\n"
    );
}
#else
void enter_user_mode(uint64_t user_rip, uint64_t user_rsp) {
    (void)user_rip;
    (void)user_rsp;
}
#endif
