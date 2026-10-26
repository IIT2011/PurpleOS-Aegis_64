#ifndef GDT_H
#define GDT_H

#include "types.h"

/* GDT Selectors */
#define GDT_NULL_SEL        0x00
#define GDT_KERNEL_CODE_SEL 0x08
#define GDT_KERNEL_DATA_SEL 0x10
#define GDT_USER_DATA_SEL   0x18
#define GDT_USER_CODE_SEL   0x20
#define GDT_TSS_SEL         0x28

#define GDT_RPL_KERNEL      0
#define GDT_RPL_USER        3

#define GDT_KERNEL_CS       (GDT_KERNEL_CODE_SEL | GDT_RPL_KERNEL) /* 0x08 */
#define GDT_KERNEL_DS       (GDT_KERNEL_DATA_SEL | GDT_RPL_KERNEL) /* 0x10 */
#define GDT_USER_DS         (GDT_USER_DATA_SEL   | GDT_RPL_USER)   /* 0x1B */
#define GDT_USER_CS         (GDT_USER_CODE_SEL   | GDT_RPL_USER)   /* 0x23 */

/* 64-bit Task State Segment (TSS) */
typedef struct tss64 {
    uint32_t reserved0;
    uint64_t rsp0;       /* Kernel stack for Ring 3 transitions */
    uint64_t rsp1;
    uint64_t rsp2;
    uint64_t reserved1;
    uint64_t ist1;       /* Interrupt Stack Table entries */
    uint64_t ist2;
    uint64_t ist3;
    uint64_t ist4;
    uint64_t ist5;
    uint64_t ist6;
    uint64_t ist7;
    uint64_t reserved2;
    uint16_t reserved3;
    uint16_t iopb_offset; /* Offset to I/O Permission Bitmap */
} __attribute__((packed)) tss64_t;

/* GDTR descriptor */
typedef struct gdt_ptr {
    uint16_t limit;
    uint64_t base;
} __attribute__((packed)) gdt_ptr_t;

void gdt_init(void);
void gdt_ap_init(void);
void gdt_set_kernel_stack(uint64_t rsp0);
tss64_t *gdt_get_tss(void);
uint16_t gdt_get_user_cs(void);
uint16_t gdt_get_user_ds(void);
uint16_t gdt_get_kernel_cs(void);
uint16_t gdt_get_kernel_ds(void);

/* Ring 3 transition primitive */
void enter_user_mode(uint64_t user_rip, uint64_t user_rsp);

#endif /* GDT_H */
