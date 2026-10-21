#include "pit.h"
#include "io.h"
#include "idt.h"
#include "task.h"
#include "serial.h"

static volatile uint64_t timer_ticks = 0;

static void pit_handler(interrupt_frame_t *frame) {
    (void)frame;
    timer_ticks++;
    task_timer_tick();
}

uint64_t pit_get_ticks(void) {
    return timer_ticks;
}

void pit_init(uint32_t frequency) {
    if (frequency == 0) frequency = PIT_TARGET_HZ;

    uint16_t divisor = (uint16_t)(PIT_BASE_FREQ / frequency);

    // Channel 0, lobyte/hibyte, Mode 3 (Square wave), binary mode
    outb(0x43, 0x36);
    outb(0x40, (uint8_t)(divisor & 0xFF));
    outb(0x40, (uint8_t)((divisor >> 8) & 0xFF));

    register_interrupt_handler(32, pit_handler);
    kinfo("PIT Initialized: %u Hz (divisor %u, tick period %u ms)",
          frequency, divisor, 1000 / frequency);
}
