ENTRY(_start)

SECTIONS {
    . = 0x101000;
    _kernel_start = .;

    .boot : ALIGN(4) {
        *(.boot)
    }

    .text : ALIGN(4096) {
        *(.text*)
    }

    .rodata : ALIGN(4096) {
        *(.rodata*)
    }

    .data : ALIGN(4096) {
        *(.data*)
    }

    .bss : ALIGN(4096) {
        *(COMMON)
        *(.bss*)
    }

    . = ALIGN(4096);
    _kernel_end = .;
}
