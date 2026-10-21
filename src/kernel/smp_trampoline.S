/*
 * smp_trampoline.S - Real-Mode AP Bootstrap Trampoline for Aegis64
 *
 * This code is copied to physical address 0x8000 by the BSP.
 * When the BSP issues a Startup IPI (SIPI) with vector 0x08,
 * the target Application Processor (AP) begins executing at 0x8000
 * in 16-bit real mode.
 *
 * Sequence:
 * 1. 16-bit Real Mode: disable interrupts, load 32-bit GDT, enable PE in CR0.
 * 2. 32-bit Protected Mode: enable PAE/SSE in CR4, load CR3, enable LME in EFER, enable PG in CR0.
 * 3. 64-bit Long Mode: load 64-bit stack, signal ready to BSP, wait for ACK, jump to C entry point.
 */

.section .text
.code16
.global smp_trampoline_start
.global smp_trampoline_end

smp_trampoline_start:
    jmp real_start
    .byte 0x90, 0x90, 0x90, 0x90, 0x90, 0x90

/* Fixed Data Block (Offsets match smp_trampoline_data_t) */
.global smp_trampoline_cr3
smp_trampoline_cr3:     .quad 0          /* offset 0x08: PML4 CR3 */
.global smp_trampoline_rsp
smp_trampoline_rsp:     .quad 0          /* offset 0x10: Target RSP */
.global smp_trampoline_entry
smp_trampoline_entry:   .quad 0          /* offset 0x18: 64-bit C Entry */
.global smp_trampoline_ready
smp_trampoline_ready:   .quad 0          /* offset 0x20: AP sets to 1 */
.global smp_trampoline_ack
smp_trampoline_ack:     .quad 0          /* offset 0x28: BSP sets to 1 */

/* Trampoline Global Descriptor Table */
.align 8
smp_gdt:
    .quad 0x0000000000000000          /* 0x00: null */
    .quad 0x00CF9A000000FFFF          /* 0x08: 32-bit flat code (base=0, limit=4G) */
    .quad 0x00CF92000000FFFF          /* 0x10: 32-bit flat data (base=0, limit=4G) */
    .quad 0x00209A0000000000          /* 0x18: 64-bit flat code (DPL 0, L=1) */
    .quad 0x0000920000000000          /* 0x20: 64-bit flat data (DPL 0) */
smp_gdt_end:

smp_gdtr:
    .word smp_gdt_end - smp_gdt - 1
    .long 0x8000 + (smp_gdt - smp_trampoline_start)
    .word 0

real_start:
    cli
    cld
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    /* Load 32-bit GDT */
    data32 lgdt (0x8000 + (smp_gdtr - smp_trampoline_start))

    /* Enable Protected Mode in CR0 */
    movl %cr0, %eax
    orl $1, %eax
    movl %eax, %cr0

    /* Far jump to 32-bit code selector 0x08 */
    ljmpl $0x08, $(0x8000 + (prot32_start - smp_trampoline_start))

.code32
prot32_start:
    /* Reload data segments with 32-bit data selector 0x10 */
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    /* Enable PAE (bit 5), OSFXSR (bit 9), OSXMMEXCPT (bit 10) in CR4 */
    movl %cr4, %eax
    orl $((1 << 5) | (1 << 9) | (1 << 10)), %eax
    movl %eax, %cr4

    /* Load CR3 with PML4 address from offset 0x08 */
    movl (0x8000 + 0x08), %eax
    movl %eax, %cr3

    /* Enable Long Mode (LME, bit 8) and NXE (bit 11) in EFER MSR (0xC0000080) */
    movl $0xC0000080, %ecx
    rdmsr
    orl $((1 << 8) | (1 << 11)), %eax
    wrmsr

    /* Enable Paging (PG, bit 31) and Protection (PE, bit 0) in CR0 */
    movl %cr0, %eax
    orl $((1 << 31) | (1 << 0)), %eax
    movl %eax, %cr0

    /* Far jump to 64-bit code selector 0x18 */
    ljmp $0x18, $(0x8000 + (long64_start - smp_trampoline_start))

.code64
long64_start:
    /* Reload data segments with 64-bit data selector 0x20 */
    movw $0x20, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    /* Load 64-bit kernel stack for this AP from offset 0x10 */
    movq (0x8000 + 0x10), %rsp

    /* Signal ready to BSP by setting ready flag at offset 0x20 to 1 */
    movq $1, (0x8000 + 0x20)

    /* Wait for BSP acknowledge at offset 0x28 before jumping to entry */
1:
    pause
    movq (0x8000 + 0x28), %rax
    testq %rax, %rax
    jz 1b

    /* Call 64-bit C entry point from offset 0x18 */
    movq (0x8000 + 0x18), %rax
    jmp *%rax

smp_trampoline_end:
