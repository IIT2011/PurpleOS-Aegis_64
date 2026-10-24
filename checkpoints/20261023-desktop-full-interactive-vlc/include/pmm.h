#ifndef PMM_H
#define PMM_H

#include "types.h"
#include "multiboot.h"

#define PMM_PAGE_SIZE        4096
#define PMM_MAX_PHYS_ADDR    0x100000000ULL /* 4 GiB limit */
#define PMM_TOTAL_FRAMES     (PMM_MAX_PHYS_ADDR / PMM_PAGE_SIZE) /* 1,048,576 */
#define PMM_BITMAP_SIZE      (PMM_TOTAL_FRAMES / 8) /* 131,072 bytes = 128 KiB */

/* Status return codes */
#define PMM_OK               0
#define PMM_ERR_OUT_OF_BOUNDS -1
#define PMM_ERR_ALREADY_FREE -2
#define PMM_ERR_RESERVED     -3
#define PMM_ERR_EXHAUSTED    -4
#define PMM_ERR_INVALID_PARAM -5

void pmm_init(struct multiboot_info *mbi, uintptr_t kernel_start, uintptr_t kernel_end);
void *pmm_alloc_frame(void);
void *pmm_alloc_frames(size_t count);
int pmm_free_frame(void *paddr);
int pmm_free_frames(void *paddr, size_t count);

/* Diagnostic and accounting routines */
size_t pmm_get_total_frames(void);
size_t pmm_get_free_frames(void);
size_t pmm_get_allocated_frames(void);
size_t pmm_get_reserved_frames(void);

int pmm_is_frame_allocated(uintptr_t frame_idx);
int pmm_is_frame_reserved(uintptr_t frame_idx);

#endif /* PMM_H */
