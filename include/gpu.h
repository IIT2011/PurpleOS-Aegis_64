#ifndef GPU_H
#define GPU_H

#include "types.h"
#include "pci.h"

/* Supported GPU Vendors */
#define GPU_VENDOR_NONE         0x0000
#define GPU_VENDOR_INTEL        0x8086
#define GPU_VENDOR_AMD          0x1002
#define GPU_VENDOR_QEMU_VGA     0x1234

/* Hardware Architectures */
typedef enum {
    GPU_ARCH_UNKNOWN = 0,
    GPU_ARCH_SW_FALLBACK,
    /* Intel Generations */
    GPU_ARCH_INTEL_GEN4,        /* GMA 3000 / X3000 / G35 */
    GPU_ARCH_INTEL_GEN5,        /* Ironlake */
    GPU_ARCH_INTEL_GEN6,        /* Sandy Bridge */
    GPU_ARCH_INTEL_GEN7,        /* Ivy Bridge / Haswell */
    GPU_ARCH_INTEL_GEN8,        /* Broadwell */
    GPU_ARCH_INTEL_GEN9,        /* Skylake / Kaby Lake / Coffee Lake */
    GPU_ARCH_INTEL_GEN11,       /* Ice Lake */
    GPU_ARCH_INTEL_GEN12,       /* Tiger Lake / Iris Xe / Rocket / Alder Lake */
    /* AMD Families */
    GPU_ARCH_AMD_GCN1_SI,       /* Southern Islands (HD 7000 / 8000) */
    GPU_ARCH_AMD_GCN2_CI,       /* Sea Islands (R7/R9 200) */
    GPU_ARCH_AMD_GCN3_VI,       /* Volcanic Islands (Tonga / Fiji) */
    GPU_ARCH_AMD_GCN4_POLARIS,  /* Polaris (RX 400 / 500) */
    GPU_ARCH_AMD_GCN5_VEGA,     /* Vega (Vega 56/64 / Radeon VII) */
    GPU_ARCH_AMD_RDNA1_NAVI10,  /* Navi 10/14 (RX 5000) */
    GPU_ARCH_AMD_RDNA2_NAVI20   /* Navi 21/22/23 (RX 6000) */
} gpu_arch_t;

/* Capability Flags */
#define GPU_CAP_2D_FILL         0x00000001
#define GPU_CAP_2D_BLIT         0x00000002
#define GPU_CAP_DMA             0x00000004
#define GPU_CAP_FENCE           0x00000008
#define GPU_CAP_ALPHA_BLEND     0x00000010
#define GPU_CAP_HARDWARE_RING   0x00000020

/* Intel Blitter Ring Registers (BCS) */
#define INTEL_BCS_RING_TAIL     0x022030
#define INTEL_BCS_RING_HEAD     0x022034
#define INTEL_BCS_RING_START    0x022038
#define INTEL_BCS_RING_CTL      0x02203C

/* Intel Blitter Opcodes */
#define INTEL_CMD_NOOP          0x00000000
#define INTEL_CMD_FLUSH         (0x04 << 23)
#define INTEL_CMD_COLOR_BLT     ((0x2 << 29) | (0x50 << 22))
#define INTEL_CMD_SRC_COPY_BLT  ((0x2 << 29) | (0x53 << 22))
#define INTEL_CMD_STORE_DATA    ((0x3 << 29) | (0x20 << 23))

/* AMD SDMA Registers & Opcodes */
#define AMD_SDMA_OP_COPY        0x01
#define AMD_SDMA_OP_CONST_FILL  0x08
#define AMD_SDMA_OP_FENCE       0x05
#define AMD_SDMA_OP_POLL_REGMEM 0x03

#define GPU_RING_SIZE_BYTES     (16 * 1024) /* 16 KiB Ring Buffer */
#define GPU_RING_NUM_DWORDS     (GPU_RING_SIZE_BYTES / 4)

struct gpu_device;

/* GPU Driver Operations */
typedef struct {
    int  (*init)(struct gpu_device *dev);
    void (*reset)(struct gpu_device *dev);
    int  (*fill_rect)(struct gpu_device *dev, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color);
    int  (*copy_rect)(struct gpu_device *dev, uint32_t src_x, uint32_t src_y,
                      uint32_t dst_x, uint32_t dst_y, uint32_t w, uint32_t h, uint32_t pitch);
    uint32_t (*emit_fence)(struct gpu_device *dev);
    int  (*wait_fence)(struct gpu_device *dev, uint32_t fence_seq, uint32_t timeout_ms);
    void (*wait_idle)(struct gpu_device *dev);
    void (*get_status)(struct gpu_device *dev, char *buf, size_t max_len);
} gpu_ops_t;

/* Unified GPU Device Structure */
typedef struct gpu_device {
    char            name[64];
    uint16_t        vendor_id;
    uint16_t        device_id;
    gpu_arch_t      arch;
    const char     *arch_name;
    uint32_t        capabilities;
    int             is_hardware_accelerated;

    /* MMIO and Framebuffer apertures */
    uint64_t        mmio_phys;
    void           *mmio_base;
    uint32_t        mmio_size;
    uint64_t        vram_phys;
    void           *vram_base;
    uint32_t        vram_size;

    /* Command Streamer / Ring Buffer */
    uint32_t       *ring_buffer;
    uint64_t        ring_phys;
    uint32_t        ring_head;
    uint32_t        ring_tail;
    uint32_t        ring_size;

    /* Synchronization and Status Page */
    volatile uint32_t *status_page;
    uint64_t        status_page_phys;
    uint32_t        last_submitted_fence;
    uint32_t        last_completed_fence;

    /* Benchmark and Performance Counters */
    uint64_t        ops_fill_count;
    uint64_t        ops_copy_count;
    uint64_t        pixels_rendered;

    const gpu_ops_t *ops;
    const pci_device_t *pci_dev;
} gpu_device_t;

/* Public API */
int  gpu_subsystem_init(void);
gpu_device_t *gpu_get_primary(void);
int  gpu_fill_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color);
int  gpu_copy_rect(uint32_t src_x, uint32_t src_y, uint32_t dst_x, uint32_t dst_y, uint32_t w, uint32_t h, uint32_t pitch);
void gpu_wait_idle(void);
void gpu_run_benchmark(void);

#endif /* GPU_H */
