.set MB_ALIGN,     1 << 0
.set MB_MEMINFO,   1 << 1
.set MB_VIDMODE,   1 << 2
.set MB_FLAGS,     MB_ALIGN | MB_MEMINFO | MB_VIDMODE
.set MB_MAGIC,     0x1BADB002
.set MB_CHECKSUM,  -(MB_MAGIC + MB_FLAGS)

.section .boot, "ax"
.code32
.global _start
.type _start, @function

_start:
    jmp multiboot_entry

    /* Multiboot 1 Header */
    .align 4
    .long MB_MAGIC
    .long MB_FLAGS
    .long MB_CHECKSUM
    .long 0, 0, 0, 0, 0     /* Address fields (unused for ELF) */
    .long 0                 /* Mode type: 0 = linear graphics */
    .long 1280              /* Preferred width */
    .long 800               /* Preferred height */
    .long 32                /* Preferred depth */

multiboot_entry:
    cli

    /* Save Multiboot magic (EAX) and Multiboot info pointer (EBX) */
    movl %eax, (boot_mb_magic)
    movl %ebx, (boot_mb_info)

    /* Set up early 32-bit stack */
    movl $boot_stack_top, %esp

    /* 1. Build early identity page tables covering first 4 GiB using 2 MiB huge pages */
    /* Link boot_pml4[0] -> boot_pdpt */
    movl $boot_pdpt, %eax
    orl $0x03, %eax          /* Present | Writable */
    movl %eax, (boot_pml4)

    /* Link boot_pdpt[0..3] -> boot_pd_0..3 */
    movl $boot_pd_0, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 0)

    movl $boot_pd_1, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 8)

    movl $boot_pd_2, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 16)

    movl $boot_pd_3, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 24)

    /* Fill each boot_pd with 512 entries of 2 MiB pages: 4 * 512 * 2 MiB = 4 GiB */
    movl $0, %ecx            /* Page index: 0..2047 */
    movl $boot_pd_0, %edi

.fill_pd:
    movl %ecx, %eax
    shll $21, %eax          /* eax = ecx * 2 MiB */
    orl $0x83, %eax         /* Present | Writable | Huge (2 MiB) */
    movl %eax, (%edi)

    /* High 32 bits of address */
    movl %ecx, %eax
    shrl $11, %eax          /* ecx >> 11 */
    movl %eax, 4(%edi)

    addl $8, %edi
    incl %ecx
    cmpl $2048, %ecx
    jne .fill_pd

    /* 2. Enable PAE and SSE in CR4, configure CR0 */
    movl %cr0, %eax
    andl $~(1 << 2), %eax   /* Clear CR0.EM */
    orl $(1 << 1), %eax     /* Set CR0.MP */
    movl %eax, %cr0

    movl %cr4, %eax
    orl $(1 << 5) | (1 << 9) | (1 << 10), %eax /* CR4.PAE | CR4.OSFXSR | CR4.OSXMMEXCPT */
    movl %eax, %cr4

    /* 3. Load CR3 with PML4 */
    movl $boot_pml4, %eax
    movl %eax, %cr3

    /* 4. Enable Long Mode (LME) and NXE in EFER MSR */
    movl $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8) | (1 << 11), %eax /* EFER.LME | EFER.NXE */
    wrmsr

    /* 5. Enable Paging and Protection in CR0 */
    movl %cr0, %eax
    orl $(1 << 31) | (1 << 0), %eax /* CR0.PG | CR0.PE */
    movl %eax, %cr0

    /* 6. Load 64-bit GDT */
    lgdt (gdt64_ptr)

    /* 7. Far jump to 64-bit code segment */
    ljmp $0x08, $long_mode_entry

.code64
long_mode_entry:
    /* Reload data segment registers with 64-bit data selector 0x10 */
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    /* Set up 64-bit stack */
    movq $boot_stack_top, %rsp

    /* Pass Multiboot arguments to kernel_main_64(mbi, magic) */
    movl (boot_mb_info), %edi
    movl (boot_mb_magic), %esi

    /* Call kernel C entry */
    call kernel_main_64

.hang64:
    cli
    hlt
    jmp .hang64

/* 64-bit Global Descriptor Table */
.section .rodata
.align 16
gdt64:
    .quad 0x0000000000000000    /* 0x00: Null descriptor */
    .quad 0x00209A0000000000    /* 0x08: 64-bit Kernel Code (Execute/Read, DPL 0, L=1) */
    .quad 0x0000920000000000    /* 0x10: 64-bit Kernel Data (Read/Write, DPL 0) */
gdt64_end:

.global gdt64_ptr
gdt64_ptr:
    .word gdt64_end - gdt64 - 1
    .quad gdt64

.section .bss
.align 4096
.global boot_pml4
boot_pml4:
    .skip 4096
boot_pdpt:
    .skip 4096
boot_pd_0:
    .skip 4096
boot_pd_1:
    .skip 4096
boot_pd_2:
    .skip 4096
boot_pd_3:
    .skip 4096

.align 16
boot_stack_bottom:
    .skip 32768                 /* 32 KiB boot stack */
boot_stack_top:

boot_mb_magic:
    .long 0
boot_mb_info:
    .long 0
