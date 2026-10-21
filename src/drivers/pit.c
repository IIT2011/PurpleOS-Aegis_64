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

void pit_delay_10ms(void) {
    pit_delay_us(10000);
}

void pit_delay_us(uint32_t us) {
    if (us == 0) return;
    uint32_t count = (1193182ULL * us) / 1000000ULL;
    if (count == 0) count = 1;
    if (count > 0xFFFF) count = 0xFFFF;

    /* Channel 2, lobyte/hibyte, Mode 0 (interrupt on terminal count) */
    outb(0x61, (inb(0x61) & ~0x02) | 0x01); // Gate high, speaker low
    outb(0x43, 0xB0);
    outb(0x42, (uint8_t)(count & 0xFF));
    outb(0x42, (uint8_t)((count >> 8) & 0xFF));

    /* Reset gate: low then high */
    uint8_t val = inb(0x61) & 0xFE;
    outb(0x61, val);
    outb(0x61, val | 0x01);

    /* Wait until OUT2 (bit 5) goes high */
    while (!(inb(0x61) & 0x20)) {
        pause();
    }
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
