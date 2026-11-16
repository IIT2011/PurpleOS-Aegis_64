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
    .long 0, 0, 0, 0, 0
    .long 0
    .long 1280
    .long 800
    .long 32

multiboot_entry:
    cli

    movl %eax, (boot_mb_magic)
    movl %ebx, (boot_mb_info)

    movl $boot_stack_top, %esp

    /* 1. Build early identity page tables covering first 4 GiB using 2 MiB huge pages */
    movl $boot_pdpt, %eax
    orl $0x03, %eax
    movl %eax, (boot_pml4)
    movl $0, (boot_pml4 + 4)

    movl $boot_pd_0, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 0)
    movl $0, (boot_pdpt + 4)

    movl $boot_pd_1, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 8)
    movl $0, (boot_pdpt + 12)

    movl $boot_pd_2, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 16)
    movl $0, (boot_pdpt + 20)

    movl $boot_pd_3, %eax
    orl $0x03, %eax
    movl %eax, (boot_pdpt + 24)
    movl $0, (boot_pdpt + 28)

    movl $0, %ecx
    movl $boot_pd_0, %edi

.fill_pd:
    movl %ecx, %eax
    shll $21, %eax
    orl $0x83, %eax         /* Present | Writable | Huge (2 MiB) */
    movl %eax, (%edi)

    movl %ecx, %eax
    shrl $11, %eax
    movl %eax, 4(%edi)

    addl $8, %edi
    incl %ecx
    cmpl $2048, %ecx
    jne .fill_pd

    /* 2. Enable PAE, OSFXSR, OSXMMEXCPT in CR4 */
    movl %cr0, %eax
    andl $~(1 << 2), %eax   /* Clear CR0.EM */
    orl $(1 << 1), %eax     /* Set CR0.MP */
    movl %eax, %cr0

    movl %cr4, %eax
    orl $(1 << 5) | (1 << 9) | (1 << 10), %eax
    movl %eax, %cr4

    /* 3. Load CR3 with boot_pml4 */
    movl $boot_pml4, %eax
    movl %eax, %cr3

    /* 4. Enable Long Mode (LME) and NXE in EFER MSR */
    movl $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8) | (1 << 11), %eax
    wrmsr

    /* 5. Enable Paging (PG) and Protection (PE) in CR0 */
    movl %cr0, %eax
    orl $(1 << 31) | (1 << 0), %eax
    movl %eax, %cr0

    /* 6. Load early 64-bit GDT */
    lgdt (boot_gdt64_ptr)

    /* 7. Long jump to 64-bit long mode */
    ljmp $0x08, $long_mode_entry

.code64
long_mode_entry:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    movq $boot_stack_top, %rsp

    /* Pass Multiboot arguments to Rust: rdi = mbi_addr, rsi = magic */
    movl (boot_mb_info), %edi
    movl (boot_mb_magic), %esi

    call _start_rust

.halt_loop:
    cli
    hlt
    jmp .halt_loop

.section .rodata
.align 8
boot_gdt64:
    .quad 0x0000000000000000
    .quad 0x00AF9A000000FFFF
    .quad 0x00CF92000000FFFF
boot_gdt64_ptr:
    .word . - boot_gdt64 - 1
    .quad boot_gdt64

.section .bss
.align 4096
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
    .skip 16384
boot_stack_top:

boot_mb_magic:
    .long 0
boot_mb_info:
    .long 0
