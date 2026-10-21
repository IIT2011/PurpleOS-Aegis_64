#include "idt.h"
#include "pic.h"
#include "serial.h"
#include "string.h"
#include "io.h"

static idt_entry_t idt_entries[256];
static idt_ptr_t   idt_ptr;
static isr_handler_t isr_handlers[256];

extern void idt_load(idt_ptr_t *ptr);

/* Declare assembly ISR symbols */
#define DECLARE_ISR(i) extern void isr##i(void)

DECLARE_ISR(0);  DECLARE_ISR(1);  DECLARE_ISR(2);  DECLARE_ISR(3);
DECLARE_ISR(4);  DECLARE_ISR(5);  DECLARE_ISR(6);  DECLARE_ISR(7);
DECLARE_ISR(8);  DECLARE_ISR(9);  DECLARE_ISR(10); DECLARE_ISR(11);
DECLARE_ISR(12); DECLARE_ISR(13); DECLARE_ISR(14); DECLARE_ISR(15);
DECLARE_ISR(16); DECLARE_ISR(17); DECLARE_ISR(18); DECLARE_ISR(19);
DECLARE_ISR(20); DECLARE_ISR(21); DECLARE_ISR(22); DECLARE_ISR(23);
DECLARE_ISR(24); DECLARE_ISR(25); DECLARE_ISR(26); DECLARE_ISR(27);
DECLARE_ISR(28); DECLARE_ISR(29); DECLARE_ISR(30); DECLARE_ISR(31);

DECLARE_ISR(32); DECLARE_ISR(33); DECLARE_ISR(34); DECLARE_ISR(35);
DECLARE_ISR(36); DECLARE_ISR(37); DECLARE_ISR(38); DECLARE_ISR(39);
DECLARE_ISR(40); DECLARE_ISR(41); DECLARE_ISR(42); DECLARE_ISR(43);
DECLARE_ISR(44); DECLARE_ISR(45); DECLARE_ISR(46); DECLARE_ISR(47);

void idt_set_gate(uint8_t num, uint64_t base, uint16_t sel, uint8_t flags) {
    idt_entries[num].offset_low  = (uint16_t)(base & 0xFFFF);
    idt_entries[num].selector    = sel;
    idt_entries[num].ist         = 0;
    idt_entries[num].type_attr   = flags;
    idt_entries[num].offset_mid  = (uint16_t)((base >> 16) & 0xFFFF);
    idt_entries[num].offset_high = (uint32_t)((base >> 32) & 0xFFFFFFFF);
    idt_entries[num].zero        = 0;
}

void register_interrupt_handler(uint8_t int_no, isr_handler_t handler) {
    isr_handlers[int_no] = handler;
}

static const char *exception_messages[32] = {
    "Division By Zero", "Debug", "Non Maskable Interrupt", "Breakpoint",
    "Into Detected Overflow", "Out of Bounds", "Invalid Opcode", "No Coprocessor",
    "Double Fault", "Coprocessor Segment Overrun", "Bad TSS", "Segment Not Present",
    "Stack Fault", "General Protection Fault", "Page Fault", "Unknown Interrupt",
    "Coprocessor Fault", "Alignment Check", "Machine Check", "SIMD Floating-Point",
    "Virtualization", "Control Protection", "Reserved", "Reserved",
    "Reserved", "Reserved", "Reserved", "Reserved",
    "Reserved", "Reserved", "Security", "Reserved"
};

void interrupt_dispatch(interrupt_frame_t *frame) {
    if (frame->int_no < 32) {
        kerror("CPU EXCEPTION %u (%s) at RIP=%p (CS=%p, ERR=%p)",
               (uint32_t)frame->int_no,
               exception_messages[frame->int_no],
               (void *)frame->rip, (void *)frame->cs, (void *)frame->err_code);

        if (frame->int_no == 14) {
            uint64_t cr2 = read_cr2();
            kerror("PAGE FAULT DETAILS: CR2 (Linear Address) = %p", (void *)cr2);
            kerror("Flags: [Present=%u, Write=%u, User=%u, ReservedBit=%u, InstructionFetch=%u]",
                   (uint32_t)(frame->err_code & 1),
                   (uint32_t)((frame->err_code >> 1) & 1),
                   (uint32_t)((frame->err_code >> 2) & 1),
                   (uint32_t)((frame->err_code >> 3) & 1),
                   (uint32_t)((frame->err_code >> 4) & 1));
        }

        kerror("Halting system on unhandled CPU exception.");
        while (1) {
            cli();
            hlt();
        }
    }

    if (isr_handlers[frame->int_no]) {
        isr_handlers[frame->int_no](frame);
    }

    if (frame->int_no >= 32 && frame->int_no < 48) {
        pic_send_eoi((uint8_t)(frame->int_no - 32));
    }
}

void idt_init(void) {
    memset(idt_entries, 0, sizeof(idt_entries));
    memset(isr_handlers, 0, sizeof(isr_handlers));

    idt_ptr.limit = sizeof(idt_entries) - 1;
    idt_ptr.base  = (uint64_t)(uintptr_t)&idt_entries;

    /* 0x8E = Present, Ring 0, 64-bit Interrupt Gate */
    uint16_t cs = 0x08; // Kernel code segment

#define SET_ISR(i) idt_set_gate(i, (uint64_t)isr##i, cs, 0x8E)

    SET_ISR(0);  SET_ISR(1);  SET_ISR(2);  SET_ISR(3);
    SET_ISR(4);  SET_ISR(5);  SET_ISR(6);  SET_ISR(7);
    SET_ISR(8);  SET_ISR(9);  SET_ISR(10); SET_ISR(11);
    SET_ISR(12); SET_ISR(13); SET_ISR(14); SET_ISR(15);
    SET_ISR(16); SET_ISR(17); SET_ISR(18); SET_ISR(19);
    SET_ISR(20); SET_ISR(21); SET_ISR(22); SET_ISR(23);
    SET_ISR(24); SET_ISR(25); SET_ISR(26); SET_ISR(27);
    SET_ISR(28); SET_ISR(29); SET_ISR(30); SET_ISR(31);

    SET_ISR(32); SET_ISR(33); SET_ISR(34); SET_ISR(35);
    SET_ISR(36); SET_ISR(37); SET_ISR(38); SET_ISR(39);
    SET_ISR(40); SET_ISR(41); SET_ISR(42); SET_ISR(43);
    SET_ISR(44); SET_ISR(45); SET_ISR(46); SET_ISR(47);

    pic_init();
    idt_load(&idt_ptr);
    kinfo("IDT Initialized: 256 gates loaded, PIC remapped to IRQ 32..47");
}
