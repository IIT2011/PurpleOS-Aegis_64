ENTRY(_start)

PHDRS {
    code PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = 0x0000008000000000;
    .text : ALIGN(4096) { *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) } :rodata
    .data : ALIGN(4096) { *(.data .data.* .got .got.*) } :data
    .bss (NOLOAD) : ALIGN(16) { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame .eh_frame.* .note .note.* .comment) }
}
