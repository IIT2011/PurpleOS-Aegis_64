#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "gpu.h"
#include "video.h"
#include "pci.h"

/* Mock video framebuffer for testing */
static uint32_t mock_fb[640 * 480];

int video_is_available(void) { return 1; }
uint32_t video_get_width(void) { return 640; }
uint32_t video_get_height(void) { return 480; }

void video_draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (x < 640 && y < 480) {
        mock_fb[y * 640 + x] = color;
    }
}

uint32_t video_get_pixel(uint32_t x, uint32_t y) {
    if (x < 640 && y < 480) {
        return mock_fb[y * 640 + x];
    }
    return 0;
}

void video_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color) {
    for (uint32_t r = 0; r < h; r++) {
        for (uint32_t c = 0; c < w; c++) {
            video_draw_pixel(x + c, y + r, color);
        }
    }
}

static void test_sw_fallback_basic(void) {
    memset(mock_fb, 0, sizeof(mock_fb));
    pci_init(); // no devices
    int rc = gpu_subsystem_init();
    assert(rc == 0);

    gpu_device_t *gpu = gpu_get_primary();
    assert(gpu != NULL);
    assert(gpu->is_hardware_accelerated == 0);
    assert(gpu->arch == GPU_ARCH_SW_FALLBACK);

    // Test solid fill
    gpu_fill_rect(10, 10, 20, 20, 0x00FF00AA);
    assert(mock_fb[10 * 640 + 10] == 0x00FF00AA);
    assert(mock_fb[29 * 640 + 29] == 0x00FF00AA);
    assert(mock_fb[9 * 640 + 10] == 0);

    // Test copy rect
    gpu_copy_rect(10, 10, 50, 50, 20, 20, 640 * 4);
    assert(mock_fb[50 * 640 + 50] == 0x00FF00AA);
    assert(mock_fb[69 * 640 + 69] == 0x00FF00AA);

    // Test fence
    uint32_t f = gpu->ops->emit_fence(gpu);
    assert(f > 0);
    assert(gpu->ops->wait_fence(gpu, f, 100) == 0);

    printf("[PASS] test_sw_fallback_basic\n");
}

static void test_intel_hardware_bcs(void) {
    memset(mock_fb, 0, sizeof(mock_fb));
    pci_init();

    // Register simulated Intel UHD 630 (0x8086:0x3E92)
    pci_device_t intel_pci = {
        .bus = 0, .dev = 2, .func = 0,
        .vendor_id = 0x8086,
        .device_id = 0x3E92,
        .class_code = 0x03,
        .subclass = 0x00,
        .bar = { 0xE0000000, 0, 0xD0000000, 0, 0, 0 }
    };
    pci_register_device(&intel_pci);

    gpu_subsystem_init();
    gpu_device_t *gpu = gpu_get_primary();
    assert(gpu != NULL);
    assert(gpu->vendor_id == 0x8086);
    assert(gpu->is_hardware_accelerated == 1);
    assert(gpu->arch == GPU_ARCH_INTEL_GEN9);
    assert(strstr(gpu->name, "Intel") != NULL);

    // Test Intel BCS command emission
    uint32_t init_tail = gpu->ring_tail;
    gpu_fill_rect(0, 0, 100, 50, 0x008800FF);

    // Ring should have advanced by 5 DWORDS
    uint32_t advanced = (gpu->ring_tail >= init_tail) ? (gpu->ring_tail - init_tail) :
                        (GPU_RING_NUM_DWORDS - init_tail + gpu->ring_tail);
    assert(advanced == 5);

    // Check packet header
    uint32_t cmd_header = gpu->ring_buffer[init_tail];
    assert((cmd_header & 0xFFFF0000) == INTEL_CMD_COLOR_BLT);

    // Test fence emission
    uint32_t fence_seq = gpu->ops->emit_fence(gpu);
    assert(fence_seq > 0);
    assert(gpu->status_page[0] == fence_seq);
    assert(gpu->ops->wait_fence(gpu, fence_seq, 50) == 0);

    printf("[PASS] test_intel_hardware_bcs\n");
}

static void test_amd_hardware_sdma(void) {
    memset(mock_fb, 0, sizeof(mock_fb));
    pci_init();

    // Register simulated AMD RX 580 (0x1002:0x67DF)
    pci_device_t amd_pci = {
        .bus = 1, .dev = 0, .func = 0,
        .vendor_id = 0x1002,
        .device_id = 0x67DF,
        .class_code = 0x03,
        .subclass = 0x00,
        .bar = { 0xF0000000, 0, 0xE0000000, 0, 0, 0 }
    };
    pci_register_device(&amd_pci);

    gpu_subsystem_init();
    gpu_device_t *gpu = gpu_get_primary();
    assert(gpu != NULL);
    assert(gpu->vendor_id == 0x1002);
    assert(gpu->is_hardware_accelerated == 1);
    assert(gpu->arch == GPU_ARCH_AMD_GCN4_POLARIS);
    assert(strstr(gpu->name, "Radeon") != NULL);

    // Test AMD SDMA command emission
    uint32_t init_tail = gpu->ring_tail;
    gpu_fill_rect(20, 20, 50, 50, 0x00112233);
    uint32_t advanced = (gpu->ring_tail >= init_tail) ? (gpu->ring_tail - init_tail) :
                        (GPU_RING_NUM_DWORDS - init_tail + gpu->ring_tail);
    assert(advanced == 4); // Opcode+count, addr, val, count

    // Test AMD fence
    uint32_t fence = gpu->ops->emit_fence(gpu);
    assert(fence > 0);
    assert(gpu->status_page[0] == fence);

    printf("[PASS] test_amd_hardware_sdma\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running GPU Hardware Acceleration & Subsystem Tests \n");
    printf("=====================================================\n");

    test_sw_fallback_basic();
    test_intel_hardware_bcs();
    test_amd_hardware_sdma();

    printf("=====================================================\n");
    printf(" GPU Subsystem Tests: ALL PASSED                     \n");
    printf("=====================================================\n");
    return 0;
}
