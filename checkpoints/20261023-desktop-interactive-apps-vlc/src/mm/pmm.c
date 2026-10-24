#include "pmm.h"
#include "string.h"
#include "serial.h"

/* Dual bitmap storage: 128 KiB each for 1,048,576 frames (4 GiB address space) */
static uint8_t pmm_alloc_bitmap[PMM_BITMAP_SIZE];
static uint8_t pmm_reserved_bitmap[PMM_BITMAP_SIZE];

static size_t total_managed_frames = 0;
static size_t free_frames_count = 0;
static size_t allocated_frames_count = 0;
static size_t reserved_frames_count = 0;
static uintptr_t last_alloc_frame = 0;

static inline void bitmap_set(uint8_t *bitmap, uintptr_t frame) {
    bitmap[frame / 8] |= (1 << (frame % 8));
}

static inline void bitmap_clear(uint8_t *bitmap, uintptr_t frame) {
    bitmap[frame / 8] &= ~(1 << (frame % 8));
}

static inline int bitmap_test(const uint8_t *bitmap, uintptr_t frame) {
    return (bitmap[frame / 8] & (1 << (frame % 8))) != 0;
}

int pmm_is_frame_allocated(uintptr_t frame_idx) {
    if (frame_idx >= PMM_TOTAL_FRAMES) return 1;
    return bitmap_test(pmm_alloc_bitmap, frame_idx);
}

int pmm_is_frame_reserved(uintptr_t frame_idx) {
    if (frame_idx >= PMM_TOTAL_FRAMES) return 1;
    return bitmap_test(pmm_reserved_bitmap, frame_idx);
}

size_t pmm_get_total_frames(void) {
    return total_managed_frames;
}

size_t pmm_get_free_frames(void) {
    return free_frames_count;
}

size_t pmm_get_allocated_frames(void) {
    return allocated_frames_count;
}

size_t pmm_get_reserved_frames(void) {
    return reserved_frames_count;
}

static void mark_region_reserved(uint64_t start_addr, uint64_t end_addr) {
    if (start_addr >= PMM_MAX_PHYS_ADDR) return;
    if (end_addr > PMM_MAX_PHYS_ADDR) end_addr = PMM_MAX_PHYS_ADDR;

    uintptr_t start_frame = start_addr / PMM_PAGE_SIZE;
    uintptr_t end_frame = (end_addr + PMM_PAGE_SIZE - 1) / PMM_PAGE_SIZE;

    for (uintptr_t f = start_frame; f < end_frame && f < PMM_TOTAL_FRAMES; f++) {
        if (!bitmap_test(pmm_reserved_bitmap, f)) {
            bitmap_set(pmm_reserved_bitmap, f);
            if (!bitmap_test(pmm_alloc_bitmap, f)) {
                bitmap_set(pmm_alloc_bitmap, f);
                if (free_frames_count > 0) free_frames_count--;
            }
            reserved_frames_count++;
        }
    }
}

void pmm_init(struct multiboot_info *mbi, uintptr_t kernel_start, uintptr_t kernel_end) {
    /* Initially mark all frames as allocated and reserved until discovered from memory map */
    memset(pmm_alloc_bitmap, 0xFF, PMM_BITMAP_SIZE);
    memset(pmm_reserved_bitmap, 0xFF, PMM_BITMAP_SIZE);

    total_managed_frames = PMM_TOTAL_FRAMES;
    free_frames_count = 0;
    allocated_frames_count = PMM_TOTAL_FRAMES;
    reserved_frames_count = PMM_TOTAL_FRAMES;

    if (!mbi || !(mbi->flags & MULTIBOOT_INFO_MEM_MAP) || mbi->mmap_length == 0) {
        kwarn("PMM: No multiboot memory map available. Fallback: configuring safe 128 MiB pool.");
        /* Fallback: assume available from 16 MiB to 128 MiB */
        for (uintptr_t f = (16 * 1024 * 1024) / PMM_PAGE_SIZE; f < (128 * 1024 * 1024) / PMM_PAGE_SIZE; f++) {
            bitmap_clear(pmm_reserved_bitmap, f);
            bitmap_clear(pmm_alloc_bitmap, f);
            free_frames_count++;
            allocated_frames_count--;
            reserved_frames_count--;
        }
    } else {
        uintptr_t mmap_cur = (uintptr_t)mbi->mmap_addr;
        uintptr_t mmap_end = mmap_cur + mbi->mmap_length;

        while (mmap_cur < mmap_end) {
            struct multiboot_mmap_entry *entry = (struct multiboot_mmap_entry *)mmap_cur;
            if (entry->size == 0) break;

            if (entry->type == MULTIBOOT_MEMORY_AVAILABLE) {
                uint64_t base = entry->addr;
                uint64_t len = entry->len;

                /* Check for overflow */
                if (base + len < base) len = ~0ULL - base;

                if (base < PMM_MAX_PHYS_ADDR) {
                    uint64_t limit = base + len;
                    if (limit > PMM_MAX_PHYS_ADDR) limit = PMM_MAX_PHYS_ADDR;

                    uintptr_t start_f = (base + PMM_PAGE_SIZE - 1) / PMM_PAGE_SIZE;
                    uintptr_t end_f = limit / PMM_PAGE_SIZE;

                    for (uintptr_t f = start_f; f < end_f && f < PMM_TOTAL_FRAMES; f++) {
                        if (bitmap_test(pmm_reserved_bitmap, f)) {
                            bitmap_clear(pmm_reserved_bitmap, f);
                            bitmap_clear(pmm_alloc_bitmap, f);
                            free_frames_count++;
                            allocated_frames_count--;
                            reserved_frames_count--;
                        }
                    }
                }
            }

            mmap_cur += entry->size + sizeof(entry->size);
        }
    }

    /* Mandatory Historical Requirement: Always reserve physical 0 to 16 MiB */
    mark_region_reserved(0x0, 0x1000000);

    /* Mandatory: Always reserve kernel image memory */
    mark_region_reserved(kernel_start, kernel_end);

    /* Reserve multiboot structure and modules if within managed space */
    if (mbi) {
        mark_region_reserved((uintptr_t)mbi, (uintptr_t)mbi + sizeof(struct multiboot_info));
        if (mbi->flags & MULTIBOOT_INFO_MEM_MAP) {
            mark_region_reserved(mbi->mmap_addr, mbi->mmap_addr + mbi->mmap_length);
        }
    }

    kinfo("PMM Initialized: 4 GiB managed, %u free frames (%u MiB), %u reserved frames (%u MiB)",
          (uint32_t)free_frames_count, (uint32_t)(free_frames_count * 4 / 1024),
          (uint32_t)reserved_frames_count, (uint32_t)(reserved_frames_count * 4 / 1024));
}

void *pmm_alloc_frame(void) {
    if (free_frames_count == 0) {
        kerror("PMM: Physical frame allocation exhausted!");
        return NULL;
    }

    uintptr_t start = last_alloc_frame;
    for (uintptr_t step = 0; step < PMM_TOTAL_FRAMES; step++) {
        uintptr_t i = (start + step) % PMM_TOTAL_FRAMES;

        /* Fast skip if whole byte is allocated */
        if (i % 8 == 0 && pmm_alloc_bitmap[i / 8] == 0xFF) {
            step += 7; // Loop increment will add 1, skipping 8 total
            continue;
        }

        if (!bitmap_test(pmm_alloc_bitmap, i) && !bitmap_test(pmm_reserved_bitmap, i)) {
            bitmap_set(pmm_alloc_bitmap, i);
            free_frames_count--;
            allocated_frames_count++;
            last_alloc_frame = (i + 1) % PMM_TOTAL_FRAMES;
            return (void *)(i * PMM_PAGE_SIZE);
        }
    }
    kerror("PMM: Physical frame allocation exhausted!");
    return NULL;
}

void *pmm_alloc_frames(size_t count) {
    if (count == 0) return NULL;
    if (count == 1) return pmm_alloc_frame();

    uintptr_t run_start = 0;
    size_t run_len = 0;

    for (uintptr_t i = 0; i < PMM_TOTAL_FRAMES; i++) {
        if (!bitmap_test(pmm_alloc_bitmap, i) && !bitmap_test(pmm_reserved_bitmap, i)) {
            if (run_len == 0) run_start = i;
            run_len++;
            if (run_len == count) {
                for (uintptr_t j = run_start; j < run_start + count; j++) {
                    bitmap_set(pmm_alloc_bitmap, j);
                }
                free_frames_count -= count;
                allocated_frames_count += count;
                return (void *)(run_start * PMM_PAGE_SIZE);
            }
        } else {
            run_len = 0;
        }
    }
    kerror("PMM: Failed to allocate %u contiguous frames!", (uint32_t)count);
    return NULL;
}

int pmm_free_frame(void *paddr) {
    uintptr_t addr = (uintptr_t)paddr;
    if (addr % PMM_PAGE_SIZE != 0) {
        kerror("PMM: Attempt to free unaligned address %p", paddr);
        return PMM_ERR_INVALID_PARAM;
    }
    if (addr >= PMM_MAX_PHYS_ADDR) {
        kerror("PMM: Attempt to free out-of-bounds address %p", paddr);
        return PMM_ERR_OUT_OF_BOUNDS;
    }

    uintptr_t frame = addr / PMM_PAGE_SIZE;
    if (bitmap_test(pmm_reserved_bitmap, frame)) {
        kerror("PMM: Attempt to free reserved frame %p", paddr);
        return PMM_ERR_RESERVED;
    }
    if (!bitmap_test(pmm_alloc_bitmap, frame)) {
        kerror("PMM: Double-free detected on frame %p", paddr);
        return PMM_ERR_ALREADY_FREE;
    }

    bitmap_clear(pmm_alloc_bitmap, frame);
    free_frames_count++;
    allocated_frames_count--;
    return PMM_OK;
}

int pmm_free_frames(void *paddr, size_t count) {
    uintptr_t addr = (uintptr_t)paddr;
    for (size_t i = 0; i < count; i++) {
        int err = pmm_free_frame((void *)(addr + i * PMM_PAGE_SIZE));
        if (err != PMM_OK) return err;
    }
    return PMM_OK;
}
