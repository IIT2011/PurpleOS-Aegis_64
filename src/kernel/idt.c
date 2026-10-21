#include "idt.h"
#include "pic.h"
#include "serial.h"
#include "string.h"
#include "io.h"

static idt_entry_t idt_entries[256];
static idt_ptr_t   idt_ptr;
static isr_handler_t isr_handlers[256];

extern void idt_load(idt_ptr_t *ptr);
extern uintptr_t isr_stub_table[256];

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
        kerror("=================================================");
        kerror("!!! KERNEL PANIC: CPU EXCEPTION %u (%s) !!!",
               (uint32_t)frame->int_no,
               exception_messages[frame->int_no]);
        kerror("=================================================");
        kerror("RIP: %p  CS: %p  RFLAGS: %p",
               (void *)frame->rip, (void *)frame->cs, (void *)frame->rflags);
        kerror("RSP: %p  SS: %p  ERR_CODE: %p",
               (void *)frame->rsp, (void *)frame->ss, (void *)frame->err_code);
        kerror("RAX: %p  RBX: %p  RCX: %p  RDX: %p",
               (void *)frame->rax, (void *)frame->rbx, (void *)frame->rcx, (void *)frame->rdx);
        kerror("RSI: %p  RDI: %p  RBP: %p",
               (void *)frame->rsi, (void *)frame->rdi, (void *)frame->rbp);
        kerror("R8 : %p  R9 : %p  R10: %p  R11: %p",
               (void *)frame->r8, (void *)frame->r9, (void *)frame->r10, (void *)frame->r11);
        kerror("R12: %p  R13: %p  R14: %p  R15: %p",
               (void *)frame->r12, (void *)frame->r13, (void *)frame->r14, (void *)frame->r15);

        if (frame->int_no == 14) {
            uint64_t cr2 = read_cr2();
            kerror("CR2 (Faulting Address): %p", (void *)cr2);
            kerror("Page Fault Reason: [%s, %s, %s, %s, %s]",
                   (frame->err_code & 1) ? "Protection Violation" : "Non-Present Page",
                   (frame->err_code & 2) ? "Write Access" : "Read Access",
                   (frame->err_code & 4) ? "User Mode" : "Kernel Mode",
                   (frame->err_code & 8) ? "Reserved Bit Overwrite" : "Valid Bits",
                   (frame->err_code & 16) ? "Instruction Fetch" : "Data Access");
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

    /* Send EOI to legacy 8259 PIC if vector is in PIC range */
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

    for (int i = 0; i < 256; i++) {
        idt_set_gate((uint8_t)i, (uint64_t)isr_stub_table[i], cs, 0x8E);
    }

    idt_load(&idt_ptr);
    kinfo("IDT Initialized: 256 gates loaded");
}
