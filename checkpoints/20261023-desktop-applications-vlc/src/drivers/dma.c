#include "dma.h"
#include "string.h"

#ifndef __KERNEL_HOST_TEST__
#include "pmm.h"
#include "vmm.h"
#include "serial.h"

void *dma_alloc_coherent(size_t size, uint64_t *phys_out) {
    if (size == 0) return NULL;
    size_t pages = (size + PMM_PAGE_SIZE - 1) / PMM_PAGE_SIZE;
    void *frames = pmm_alloc_frames(pages);
    if (!frames) {
        kwarn("DMA: Failed to allocate %u contiguous frames (%u bytes)", (uint32_t)pages, (uint32_t)size);
        return NULL;
    }

    memset(frames, 0, pages * PMM_PAGE_SIZE);
    if (phys_out) {
        *phys_out = (uint64_t)(uintptr_t)frames;
    }
    return frames;
}

void dma_free_coherent(void *vaddr, size_t size) {
    if (!vaddr || size == 0) return;
    size_t pages = (size + PMM_PAGE_SIZE - 1) / PMM_PAGE_SIZE;
    pmm_free_frames(vaddr, pages);
}

void *mmio_map(uint64_t phys_addr, size_t size) {
    page_table_t *pml4 = vmm_get_kernel_pml4();
    if (!pml4) return (void *)(uintptr_t)phys_addr;

    uint64_t start_page = phys_addr & ~0xFFFULL;
    uint64_t end_page = (phys_addr + size + 0xFFFULL) & ~0xFFFULL;

    for (uint64_t addr = start_page; addr < end_page; addr += 4096) {
        vmm_map_page(pml4, addr, addr, PAGE_PRESENT | PAGE_WRITABLE | PAGE_PCD | PAGE_PWT);
    }
    return (void *)(uintptr_t)phys_addr;
}

#else

#include <stdlib.h>

void *dma_alloc_coherent(size_t size, uint64_t *phys_out) {
    if (size == 0) return NULL;
    void *ptr = NULL;
    if (posix_memalign(&ptr, 4096, (size + 4095) & ~4095UL) != 0) {
        return NULL;
    }
    memset(ptr, 0, size);
    if (phys_out) {
        *phys_out = (uint64_t)(uintptr_t)ptr;
    }
    return ptr;
}

void dma_free_coherent(void *vaddr, size_t size) {
    (void)size;
    free(vaddr);
}

void *mmio_map(uint64_t phys_addr, size_t size) {
    (void)size;
    return (void *)(uintptr_t)phys_addr;
}

#endif
