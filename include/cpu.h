#ifndef CPU_H
#define CPU_H

#include "types.h"

#define MAX_CPUS 64

/* CPU feature flags */
#define CPU_FEAT_FPU            (1ULL << 0)
#define CPU_FEAT_APIC           (1ULL << 1)
#define CPU_FEAT_MSR            (1ULL << 2)
#define CPU_FEAT_PAE            (1ULL << 3)
#define CPU_FEAT_TSC            (1ULL << 4)
#define CPU_FEAT_NX             (1ULL << 5)
#define CPU_FEAT_SSE            (1ULL << 6)
#define CPU_FEAT_SSE2           (1ULL << 7)
#define CPU_FEAT_SSE3           (1ULL << 8)
#define CPU_FEAT_SSSE3          (1ULL << 9)
#define CPU_FEAT_SSE4_1         (1ULL << 10)
#define CPU_FEAT_SSE4_2         (1ULL << 11)
#define CPU_FEAT_AVX            (1ULL << 12)
#define CPU_FEAT_x2APIC         (1ULL << 13)
#define CPU_FEAT_RDRAND         (1ULL << 14)
#define CPU_FEAT_FSGSBASE       (1ULL << 15)
#define CPU_FEAT_INVARIANT_TSC  (1ULL << 16)

typedef struct {
    char vendor[13];
    char brand[49];
    uint32_t family;
    uint32_t model;
    uint32_t stepping;
    uint32_t max_leaf;
    uint32_t max_ext_leaf;
    uint64_t flags;
} cpu_features_t;

typedef struct cpu {
    uint32_t id;
    uint32_t apic_id;
    uint32_t acpi_id;
    uint8_t  is_bsp;
    volatile uint8_t is_online;
    uint64_t kernel_stack;
    void    *current_task;
    volatile uint64_t lapic_timer_ticks;
    volatile uint32_t ipi_count;
} cpu_t;

void cpu_init_bsp(void);
const cpu_features_t *cpu_get_features(void);
uint32_t cpu_count(void);
uint32_t cpu_online_count(void);
uint32_t cpu_get_current_id(void);
cpu_t *cpu_get(uint32_t id);
int cpu_register_ap(uint32_t apic_id, uint32_t acpi_id);
void cpu_set_online(uint32_t id, uint8_t online);
void cpu_print_info(void);

#endif /* CPU_H */
