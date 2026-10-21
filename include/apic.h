#ifndef APIC_H
#define APIC_H

#include "types.h"

/* Local APIC Register Offsets */
#define LAPIC_ID                0x0020
#define LAPIC_VERSION           0x0030
#define LAPIC_TPR               0x0080
#define LAPIC_APR               0x0090
#define LAPIC_PPR               0x00A0
#define LAPIC_EOI               0x00B0
#define LAPIC_RRD               0x00C0
#define LAPIC_LDR               0x00D0
#define LAPIC_DFR               0x00E0
#define LAPIC_SVR               0x00F0
#define LAPIC_ISR               0x0100
#define LAPIC_TMR               0x0180
#define LAPIC_IRR               0x0200
#define LAPIC_ESR               0x0280
#define LAPIC_ICR_LOW           0x0300
#define LAPIC_ICR_HIGH          0x0310
#define LAPIC_LVT_TIMER         0x0320
#define LAPIC_LVT_THERMAL       0x0330
#define LAPIC_LVT_PERF          0x0340
#define LAPIC_LVT_LINT0         0x0350
#define LAPIC_LVT_LINT1         0x0360
#define LAPIC_LVT_ERROR         0x0370
#define LAPIC_TIMER_INIT_CNT    0x0380
#define LAPIC_TIMER_CURR_CNT    0x0390
#define LAPIC_TIMER_DIVIDE      0x03E0

/* Interrupt Vectors */
#define LAPIC_TIMER_VECTOR      0x20  /* Vector 32 (maps to scheduler IRQ 0) */
#define LAPIC_IPI_RESCHED       0xFC  /* Inter-processor reschedule */
#define LAPIC_IPI_HALT          0xFD  /* Inter-processor halt */
#define LAPIC_ERROR_VECTOR      0xFE  /* Vector 254 */
#define LAPIC_SPURIOUS_VECTOR   0xFF  /* Vector 255 */

/* SVR Control Bits */
#define LAPIC_SVR_ENABLE        (1 << 8)

/* LVT Timer Modes */
#define LAPIC_TIMER_ONESHOT     (0 << 17)
#define LAPIC_TIMER_PERIODIC    (1 << 17)
#define LAPIC_TIMER_TSC_DEADLINE (2 << 17)
#define LAPIC_TIMER_MASKED      (1 << 16)

/* ICR Delivery Modes */
#define ICR_DELIV_FIXED         (0x0 << 8)
#define ICR_DELIV_LOWEST        (0x1 << 8)
#define ICR_DELIV_SMI           (0x2 << 8)
#define ICR_DELIV_NMI           (0x4 << 8)
#define ICR_DELIV_INIT          (0x5 << 8)
#define ICR_DELIV_STARTUP       (0x6 << 8)

#define ICR_DEST_PHYSICAL       (0x0 << 11)
#define ICR_DEST_LOGICAL        (0x1 << 11)

#define ICR_STATUS_IDLE         (0x0 << 12)
#define ICR_STATUS_PENDING      (0x1 << 12)

#define ICR_LEVEL_DEASSERT      (0x0 << 14)
#define ICR_LEVEL_ASSERT        (0x1 << 14)

#define ICR_TRIGGER_EDGE        (0x0 << 15)
#define ICR_TRIGGER_LEVEL       (0x1 << 15)

#define ICR_DEST_NORMAL         (0x0 << 18)
#define ICR_DEST_SELF           (0x1 << 18)
#define ICR_DEST_ALL_INC_SELF   (0x2 << 18)
#define ICR_DEST_ALL_EXC_SELF   (0x3 << 18)

/* API */
int apic_is_supported(void);
int apic_init(uint64_t phys_base);
void apic_init_ap(void);
uint32_t apic_get_id(void);
uint32_t apic_get_version(void);
void apic_eoi(void);
void apic_send_ipi(uint32_t dest_apic_id, uint32_t dword_low);
void apic_timer_init(uint32_t frequency_hz);
uint32_t apic_get_timer_freq(void);
int apic_is_enabled(void);

#endif /* APIC_H */
