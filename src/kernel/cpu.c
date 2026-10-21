#include "cpu.h"
#include "io.h"
#include "serial.h"
#include "string.h"
#include "apic.h"

static cpu_features_t bsp_features;
static cpu_t cpu_table[MAX_CPUS];
static uint32_t active_cpu_count = 1;

void cpu_init_bsp(void) {
    memset(&bsp_features, 0, sizeof(bsp_features));
    memset(cpu_table, 0, sizeof(cpu_table));

    uint32_t eax, ebx, ecx, edx;

    /* 1. CPUID Leaf 0: Vendor String and Max Basic Leaf */
    cpuid(0, &eax, &ebx, &ecx, &edx);
    bsp_features.max_leaf = eax;

    uint32_t *vendor_ptr = (uint32_t *)bsp_features.vendor;
    vendor_ptr[0] = ebx;
    vendor_ptr[1] = edx;
    vendor_ptr[2] = ecx;
    bsp_features.vendor[12] = '\0';

    /* 2. CPUID Leaf 1: Family, Model, Stepping, Features */
    if (bsp_features.max_leaf >= 1) {
        cpuid(1, &eax, &ebx, &ecx, &edx);

        uint32_t stepping = eax & 0x0F;
        uint32_t base_model = (eax >> 4) & 0x0F;
        uint32_t base_family = (eax >> 8) & 0x0F;
        uint32_t ext_model = (eax >> 16) & 0x0F;
        uint32_t ext_family = (eax >> 20) & 0xFF;

        bsp_features.stepping = stepping;
        if (base_family == 6 || base_family == 15) {
            bsp_features.model = (ext_model << 4) | base_model;
        } else {
            bsp_features.model = base_model;
        }
        if (base_family == 15) {
            bsp_features.family = base_family + ext_family;
        } else {
            bsp_features.family = base_family;
        }

        /* Check EDX feature bits */
        if (edx & (1 << 0))  bsp_features.flags |= CPU_FEAT_FPU;
        if (edx & (1 << 4))  bsp_features.flags |= CPU_FEAT_TSC;
        if (edx & (1 << 5))  bsp_features.flags |= CPU_FEAT_MSR;
        if (edx & (1 << 6))  bsp_features.flags |= CPU_FEAT_PAE;
        if (edx & (1 << 9))  bsp_features.flags |= CPU_FEAT_APIC;
        if (edx & (1 << 25)) bsp_features.flags |= CPU_FEAT_SSE;
        if (edx & (1 << 26)) bsp_features.flags |= CPU_FEAT_SSE2;

        /* Check ECX feature bits */
        if (ecx & (1 << 0))  bsp_features.flags |= CPU_FEAT_SSE3;
        if (ecx & (1 << 9))  bsp_features.flags |= CPU_FEAT_SSSE3;
        if (ecx & (1 << 19)) bsp_features.flags |= CPU_FEAT_SSE4_1;
        if (ecx & (1 << 20)) bsp_features.flags |= CPU_FEAT_SSE4_2;
        if (ecx & (1 << 21)) bsp_features.flags |= CPU_FEAT_x2APIC;
        if (ecx & (1 << 28)) bsp_features.flags |= CPU_FEAT_AVX;
        if (ecx & (1 << 30)) bsp_features.flags |= CPU_FEAT_RDRAND;

        uint32_t init_apic_id = (ebx >> 24) & 0xFF;
        cpu_table[0].apic_id = init_apic_id;
    }

    /* 3. CPUID Leaf 7: Structured Extended Feature Flags */
    if (bsp_features.max_leaf >= 7) {
        cpuid_subleaf(7, 0, &eax, &ebx, &ecx, &edx);
        if (ebx & (1 << 0)) bsp_features.flags |= CPU_FEAT_FSGSBASE;
    }

    /* 4. CPUID Extended Leaves */
    cpuid(0x80000000, &eax, &ebx, &ecx, &edx);
    bsp_features.max_ext_leaf = eax;

    if (bsp_features.max_ext_leaf >= 0x80000001) {
        cpuid(0x80000001, &eax, &ebx, &ecx, &edx);
        if (edx & (1 << 20)) bsp_features.flags |= CPU_FEAT_NX;
    }

    /* Processor Brand String (0x80000002..0x80000004) */
    if (bsp_features.max_ext_leaf >= 0x80000004) {
        uint32_t *brand_ptr = (uint32_t *)bsp_features.brand;
        for (uint32_t l = 0; l < 3; l++) {
            cpuid(0x80000002 + l, &eax, &ebx, &ecx, &edx);
            brand_ptr[l * 4 + 0] = eax;
            brand_ptr[l * 4 + 1] = ebx;
            brand_ptr[l * 4 + 2] = ecx;
            brand_ptr[l * 4 + 3] = edx;
        }
        bsp_features.brand[48] = '\0';

        /* Clean up leading spaces if present */
        char *str = bsp_features.brand;
        while (*str == ' ') str++;
        if (str != bsp_features.brand) {
            memmove(bsp_features.brand, str, strlen(str) + 1);
        }
    } else {
        strcpy(bsp_features.brand, "x86-64 Compatible Processor");
    }

    if (bsp_features.max_ext_leaf >= 0x80000007) {
        cpuid(0x80000007, &eax, &ebx, &ecx, &edx);
        if (edx & (1 << 8)) bsp_features.flags |= CPU_FEAT_INVARIANT_TSC;
    }

    /* Check MSR 0x1B to confirm BSP status */
    uint64_t apic_base_msr = rdmsr(0x1B);
    uint8_t is_bsp = (apic_base_msr & (1ULL << 8)) ? 1 : 0;

    cpu_table[0].id = 0;
    cpu_table[0].acpi_id = 0;
    cpu_table[0].is_bsp = is_bsp;
    cpu_table[0].is_online = 1;

    active_cpu_count = 1;
}

const cpu_features_t *cpu_get_features(void) {
    return &bsp_features;
}

uint32_t cpu_count(void) {
    return active_cpu_count;
}

uint32_t cpu_online_count(void) {
    uint32_t count = 0;
    for (uint32_t i = 0; i < active_cpu_count; i++) {
        if (cpu_table[i].is_online) {
            count++;
        }
    }
    return count;
}

uint32_t cpu_get_current_id(void) {
#ifdef __KERNEL_HOST_TEST__
    return 0;
#else
    if (!apic_is_enabled()) {
        return 0;
    }
    uint32_t cur_apic = apic_get_id();
    for (uint32_t i = 0; i < active_cpu_count; i++) {
        if (cpu_table[i].apic_id == cur_apic) {
            return cpu_table[i].id;
        }
    }
    return 0;
#endif
}

cpu_t *cpu_get(uint32_t id) {
    if (id >= MAX_CPUS) return NULL;
    return &cpu_table[id];
}

void cpu_set_online(uint32_t id, uint8_t online) {
    if (id < MAX_CPUS) {
        cpu_table[id].is_online = online;
    }
}

int cpu_register_ap(uint32_t apic_id, uint32_t acpi_id) {
    if (active_cpu_count >= MAX_CPUS) return -1;

    /* Check if already registered */
    for (uint32_t i = 0; i < active_cpu_count; i++) {
        if (cpu_table[i].apic_id == apic_id) {
            cpu_table[i].acpi_id = acpi_id;
            return (int)i;
        }
    }

    uint32_t idx = active_cpu_count++;
    cpu_table[idx].id = idx;
    cpu_table[idx].apic_id = apic_id;
    cpu_table[idx].acpi_id = acpi_id;
    cpu_table[idx].is_bsp = 0;
    cpu_table[idx].is_online = 0;
    cpu_table[idx].lapic_timer_ticks = 0;
    cpu_table[idx].ipi_count = 0;
    return (int)idx;
}

void cpu_print_info(void) {
    kinfo("CPU: Vendor='%s' Model='%s'", bsp_features.vendor, bsp_features.brand);
    kinfo("CPU: Family=%u Model=%u Stepping=%u (BSP APIC ID=%u)",
          bsp_features.family, bsp_features.model, bsp_features.stepping,
          cpu_table[0].apic_id);
    kinfo("CPU Features: [APIC=%u x2APIC=%u SSE=%u SSE2=%u SSE4.1=%u AVX=%u NX=%u InvTSC=%u]",
          (bsp_features.flags & CPU_FEAT_APIC) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_x2APIC) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_SSE) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_SSE2) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_SSE4_1) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_AVX) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_NX) ? 1 : 0,
          (bsp_features.flags & CPU_FEAT_INVARIANT_TSC) ? 1 : 0);
    kinfo("SMP Cores Discovered: %u, Online: %u", active_cpu_count, cpu_online_count());
    for (uint32_t i = 0; i < active_cpu_count; i++) {
        kinfo("  CPU #%u: APIC ID=%u ACPI ID=%u [%s, %s] Ticks=%u IPIs=%u",
              cpu_table[i].id, cpu_table[i].apic_id, cpu_table[i].acpi_id,
              cpu_table[i].is_bsp ? "BSP" : "AP",
              cpu_table[i].is_online ? "ONLINE" : "OFFLINE",
              (uint32_t)cpu_table[i].lapic_timer_ticks,
              cpu_table[i].ipi_count);
    }
}
