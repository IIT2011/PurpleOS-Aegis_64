#include "apic.h"
#include "cpu.h"
#include "io.h"
#include "idt.h"
#include "serial.h"
#include "pit.h"

static volatile uint32_t *lapic_base = NULL;
static uint64_t lapic_phys_address = 0;
static int apic_enabled = 0;
static uint32_t apic_timer_frequency = 0;

static inline uint32_t lapic_read(uint32_t reg) {
    if (!lapic_base) return 0;
    return lapic_base[reg / 4];
}

static inline void lapic_write(uint32_t reg, uint32_t val) {
    if (!lapic_base) return;
    lapic_base[reg / 4] = val;
}

static void lapic_spurious_handler(interrupt_frame_t *frame) {
    (void)frame;
    // Spurious interrupt does not require an EOI
}

int apic_is_supported(void) {
    const cpu_features_t *f = cpu_get_features();
    if (!(f->flags & CPU_FEAT_APIC)) {
        return 0;
    }
    return 1;
}

int apic_init(uint64_t phys_base) {
    if (!apic_is_supported()) {
        kwarn("APIC: Local APIC not supported by CPU");
        return -1;
    }

    /* 1. Determine Local APIC Base Address from MSR 0x1B */
    uint64_t msr = rdmsr(0x1B);
    if (!(msr & (1ULL << 11))) {
        // Enable APIC Global Enable flag in MSR
        msr |= (1ULL << 11);
        wrmsr(0x1B, msr);
    }

    if (phys_base == 0) {
        phys_base = msr & 0xFFFFFF000ULL;
    }
    lapic_phys_address = phys_base;
    lapic_base = (volatile uint32_t *)(uintptr_t)phys_base;

    /* 2. Configure Local Destination and Format Registers */
    lapic_write(LAPIC_DFR, 0xFFFFFFFF); // Flat model
    lapic_write(LAPIC_LDR, (lapic_read(LAPIC_ID) & 0xFF000000));

    /* 3. Register Spurious Interrupt Handler & Enable APIC Software Flag */
    register_interrupt_handler(LAPIC_SPURIOUS_VECTOR, lapic_spurious_handler);
    lapic_write(LAPIC_SVR, LAPIC_SPURIOUS_VECTOR | LAPIC_SVR_ENABLE);

    /* 4. Accept all interrupts by setting Task Priority Register to 0 */
    lapic_write(LAPIC_TPR, 0);

    /* 5. Clear error status */
    lapic_write(LAPIC_ESR, 0);
    lapic_write(LAPIC_ESR, 0);

    /* 6. Send initial EOI to clear stale pending interrupts */
    lapic_write(LAPIC_EOI, 0);

    /* 7. Mask IRQ 0 (PIT timer) on 8259 PIC so Local APIC timer drives scheduling ticks */
    outb(0x21, inb(0x21) | 0x01);

    apic_enabled = 1;

    uint32_t id = apic_get_id();
    uint32_t ver = apic_get_version();
    kinfo("Local APIC Initialized: Base=%p, APIC ID=%u, Version=0x%x",
          (void *)lapic_phys_address, id, ver);

    return 0;
}

uint32_t apic_get_id(void) {
    return (lapic_read(LAPIC_ID) >> 24) & 0xFF;
}

uint32_t apic_get_version(void) {
    return lapic_read(LAPIC_VERSION) & 0xFF;
}

void apic_eoi(void) {
    if (apic_enabled && lapic_base) {
        lapic_write(LAPIC_EOI, 0);
    }
}

void apic_send_ipi(uint32_t dest_apic_id, uint32_t dword_low) {
    if (!apic_enabled || !lapic_base) return;

    /* Write destination to high ICR */
    lapic_write(LAPIC_ICR_HIGH, (dest_apic_id & 0xFF) << 24);

    /* Write command to low ICR */
    lapic_write(LAPIC_ICR_LOW, dword_low);

    /* Wait for delivery */
    uint32_t timeout = 100000;
    while ((lapic_read(LAPIC_ICR_LOW) & ICR_STATUS_PENDING) && --timeout) {
        pause();
    }
}

void apic_timer_init(uint32_t frequency_hz) {
    if (!apic_enabled || !lapic_base || frequency_hz == 0) return;

    /* Divide by 16 */
    lapic_write(LAPIC_TIMER_DIVIDE, 0x03);

    /* 1. Calibrate using PIT */
    /* Set initial count to max */
    lapic_write(LAPIC_TIMER_INIT_CNT, 0xFFFFFFFF);

    /* Wait 10 ms using hardware PIT Channel 2 */
    pit_delay_10ms();

    /* Read how many ticks elapsed in 10 ms */
    uint32_t elapsed_in_10ms = 0xFFFFFFFF - lapic_read(LAPIC_TIMER_CURR_CNT);

    /* Stop timer */
    lapic_write(LAPIC_LVT_TIMER, LAPIC_TIMER_MASKED);

    /* Calculate ticks per second */
    uint32_t ticks_per_sec = elapsed_in_10ms * 100;
    uint32_t ticks_per_quantum = ticks_per_sec / frequency_hz;
    if (ticks_per_quantum == 0) ticks_per_quantum = 10000;

    apic_timer_frequency = frequency_hz;

    /* 2. Configure Periodic Timer on Vector 32 (matches scheduler IRQ 0) */
    lapic_write(LAPIC_TIMER_DIVIDE, 0x03);
    lapic_write(LAPIC_LVT_TIMER, LAPIC_TIMER_VECTOR | LAPIC_TIMER_PERIODIC);
    lapic_write(LAPIC_TIMER_INIT_CNT, ticks_per_quantum);

    kinfo("Local APIC Timer Initialized: %u Hz (calibrated %u ticks/sec, quantum=%u ticks)",
          frequency_hz, ticks_per_sec, ticks_per_quantum);
}

uint32_t apic_get_timer_freq(void) {
    return apic_timer_frequency;
}

int apic_is_enabled(void) {
    return apic_enabled;
}
