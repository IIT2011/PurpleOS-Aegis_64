#ifndef SMP_H
#define SMP_H

#include "types.h"

#define SMP_TRAMPOLINE_PHYS_ADDR 0x8000
#define SMP_SIPI_VECTOR          (SMP_TRAMPOLINE_PHYS_ADDR >> 12)

typedef struct {
    uint8_t  jmp_code[8];
    uint64_t cr3;
    uint64_t rsp;
    uint64_t entry;
    volatile uint64_t ready;
    volatile uint64_t bsp_ack;
} __attribute__((packed)) smp_trampoline_data_t;

#ifdef __KERNEL_HOST_TEST__
static inline void smp_init(void) {}
static inline void smp_broadcast_resched(void) {}
static inline void smp_broadcast_halt(void) {}
static inline void smp_send_resched(uint32_t cpu_id) { (void)cpu_id; }
static inline uint32_t smp_get_online_count(void) { return 1; }
#else
void smp_init(void);
void smp_ap_entry(void);
void smp_send_resched(uint32_t cpu_id);
void smp_broadcast_resched(void);
void smp_broadcast_halt(void);
uint32_t smp_get_online_count(void);
#endif

#endif /* SMP_H */
