#ifndef GPU_H
#define GPU_H

#include "types.h"
#include "pci.h"

/* Supported GPU Hardware Vendors */
typedef enum {
    GPU_VENDOR_NONE       = 0,
    GPU_VENDOR_INTEL      = 1,
    GPU_VENDOR_AMD        = 2,
    GPU_VENDOR_BOCHS_QEMU = 3
} gpu_vendor_t;

/* Intel HD/UHD/Iris/Xe GPU Generations */
typedef enum {
    INTEL_GEN_UNKNOWN = 0,
    INTEL_GEN4,       /* GMA 3000 / 965 / G35 / G45 */
    INTEL_GEN5,       /* Ironlake HD Graphics */
    INTEL_GEN6,       /* Sandy Bridge HD 2000 / 3000 */
    INTEL_GEN7,       /* Ivy Bridge HD 4000 / Haswell HD 4600 */
    INTEL_GEN8,       /* Broadwell Iris 6100 */
    INTEL_GEN9,       /* Skylake / Kaby Lake / Coffee Lake UHD 620/630 */
    INTEL_GEN11,      /* Ice Lake Iris Plus */
    INTEL_GEN12       /* Tiger Lake / Alder Lake / Iris Xe */
} intel_gen_t;

/* AMD Radeon GPU Generations */
typedef enum {
    AMD_GEN_UNKNOWN = 0,
    AMD_GEN_R600,       /* TeraScale 1 (HD 2000 / 3000) */
    AMD_GEN_EVERGREEN,  /* TeraScale 2 (HD 5000 / 6000) */
    AMD_GEN_GCN1,       /* GCN 1.0 Southern Islands (HD 7000 / R9 280) */
    AMD_GEN_GCN2,       /* GCN 2.0 Sea Islands (R9 290) */
    AMD_GEN_GCN3_4,     /* GCN 3/4 Tonga / Polaris (RX 480 / 580) */
    AMD_GEN_VEGA,       /* GCN 5.0 Vega 56/64 / Raven APU */
    AMD_GEN_RDNA        /* RDNA 1/2/3 Navi (RX 5000 / 6000 / 7000) */
} amd_gen_t;

/* Hardware Acceleration Capabilities */
#define GPU_CAP_2D_FILL         (1U << 0)   /* Hardware rectangle clear/solid fill */
#define GPU_CAP_2D_BLIT         (1U << 1)   /* Hardware surface blit / texture copy */
#define GPU_CAP_PAGE_FLIP       (1U << 2)   /* Hardware display page flipping */
#define GPU_CAP_HW_CURSOR       (1U << 3)   /* Hardware cursor overlay plane */
#define GPU_CAP_COMMAND_RING    (1U << 4)   /* Ring buffer command streamer */
#define GPU_CAP_FENCE_SYNC      (1U << 5)   /* Hardware sequence fence synchronization */

/* Driver Execution States */
typedef enum {
    GPU_STATE_UNINITIALIZED = 0,
    GPU_STATE_DETECTED      = 1,
    GPU_STATE_INITIALIZED   = 2,
    GPU_STATE_ACCELERATED   = 3,
    GPU_STATE_ERROR_FALLBACK= 4
} gpu_state_t;

struct gpu_device;

typedef struct gpu_ops {
    int (*init)(struct gpu_device *dev);
    int (*fill_rect)(struct gpu_device *dev, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color);
    int (*copy_rect)(struct gpu_device *dev, uint32_t sx, uint32_t sy, uint32_t dx, uint32_t dy, uint32_t w, uint32_t h);
    int (*submit_ring)(struct gpu_device *dev, const uint32_t *cmds, uint32_t dword_count);
    int (*wait_fence)(struct gpu_device *dev, uint32_t fence_seq);
    void (*dump_info)(struct gpu_device *dev);
} gpu_ops_t;

typedef struct gpu_device {
    char            name[48];
    char            family[32];
    gpu_vendor_t    vendor;
    uint16_t        vendor_id;
    uint16_t        device_id;
    uint8_t         revision;
    gpu_state_t     state;
    uint32_t        caps;

    /* Generation Classifications */
    intel_gen_t     intel_gen;
    amd_gen_t       amd_gen;

    /* MMIO & Aperture Memory */
    uint64_t        mmio_base;
    uint32_t        mmio_size;
    uint64_t        aperture_base;
    uint32_t        aperture_size;

    /* Command Streamer & Ring Buffer */
    uint32_t       *ring_buffer;
    uint64_t        ring_phys;
    uint32_t        ring_size;
    uint32_t        ring_head;
    uint32_t        ring_tail;
    volatile uint32_t *hws_page;    /* Hardware Status Page for fence counters */
    uint64_t        hws_phys;
    uint32_t        next_fence;
    uint32_t        completed_fence;

    /* Metrics */
    uint64_t        hw_ops_completed;
    uint64_t        sw_fallback_ops;

    const gpu_ops_t *ops;
    void            *priv;
} gpu_device_t;

/* GPU Subsystem Public Interface */
int           gpu_subsystem_init(void);
gpu_device_t *gpu_get_active_device(void);
int           gpu_is_hardware_accelerated(void);

/* Accelerated 2D Rendering Entrypoints (routes to HW if available, CPU fallback otherwise) */
int  gpu_fill_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color);
int  gpu_copy_rect(uint32_t sx, uint32_t sy, uint32_t dx, uint32_t dy, uint32_t w, uint32_t h);

/* Hardware Command Submission & Fencing */
int  gpu_submit_commands(const uint32_t *commands, uint32_t count, uint32_t *out_fence);
int  gpu_wait_fence(uint32_t fence_seq);

/* Diagnostics & Hardware Reports */
void gpu_dump_diagnostics(void);

/* Vendor Driver Initializers */
int  intel_gpu_probe(const pci_device_t *pdev, gpu_device_t *out_dev);
int  amd_gpu_probe(const pci_device_t *pdev, gpu_device_t *out_dev);

#endif /* GPU_H */
