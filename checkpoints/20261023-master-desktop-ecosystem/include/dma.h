#ifndef DMA_H
#define DMA_H

#include "types.h"

/* Coherent (uncached / physically contiguous) DMA memory allocation */
void *dma_alloc_coherent(size_t size, uint64_t *phys_out);
void  dma_free_coherent(void *vaddr, size_t size);

/* MMIO mapping helper */
void *mmio_map(uint64_t phys_addr, size_t size);

#endif /* DMA_H */
