#include "apexec.h"
#include "syscall.h"
#include "string.h"
#include "pmm.h"
#include "serial.h"

#ifdef __KERNEL_HOST_TEST__
#include <stdlib.h>
#endif

int apex_validate(const void *data, size_t size) {
    if (!data) return APEX_ERR_NULL_PTR;
    if (size < sizeof(apex_header_t)) return APEX_ERR_TOO_SMALL;

    const apex_header_t *hdr = (const apex_header_t *)data;

    if (hdr->magic != APEX_MAGIC) return APEX_ERR_BAD_MAGIC;
    if (hdr->version != APEX_VERSION_CURRENT) return APEX_ERR_BAD_VERSION;
    if (hdr->arch != APEX_ARCH_X86_64) return APEX_ERR_BAD_ARCH;
    if (hdr->abi_version != APEX_ABI_PURPLE_V1) return APEX_ERR_BAD_ABI;
    if (hdr->header_size != sizeof(apex_header_t)) return APEX_ERR_CORRUPT_HEADER;
    if (hdr->file_size != size) return APEX_ERR_CORRUPT_HEADER;
    if (hdr->num_segments == 0 || hdr->num_segments > APEX_MAX_SEGMENTS) return APEX_ERR_SEGMENT_BOUNDS;

    size_t min_size = sizeof(apex_header_t) + (hdr->num_segments * sizeof(apex_segment_t));
    if (size < min_size) return APEX_ERR_SEGMENT_BOUNDS;

    const apex_segment_t *segs = (const apex_segment_t *)((const uint8_t *)data + sizeof(apex_header_t));
    int has_exec_segment = 0;
    int entry_found = 0;

    for (uint16_t i = 0; i < hdr->num_segments; i++) {
        const apex_segment_t *s = &segs[i];

        /* W^X Security Enforcement: Write + Execute is forbidden */
        if ((s->flags & APEX_FLAG_WRITE) && (s->flags & APEX_FLAG_EXEC)) {
            kerror("APEX Security Violation: Segment %u violates W^X policy (Writable and Executable)", i);
            return APEX_ERR_SECURITY_WX;
        }

        /* Verify virtual address is within valid user address space */
        if (s->vaddr < USER_MIN_VADDR || s->vaddr >= USER_MAX_VADDR) {
            return APEX_ERR_INVALID_VADDR;
        }
        if (s->vaddr + s->mem_size < s->vaddr || s->vaddr + s->mem_size > USER_MAX_VADDR) {
            return APEX_ERR_INVALID_VADDR;
        }
        if (s->mem_size < s->file_size) {
            return APEX_ERR_SEGMENT_BOUNDS;
        }

        /* Check file payload boundary */
        if (s->type != APEX_SEG_BSS && s->file_size > 0) {
            if (s->file_offset + s->file_size < s->file_offset || s->file_offset + s->file_size > size) {
                return APEX_ERR_SEGMENT_BOUNDS;
            }
        }

        if (s->flags & APEX_FLAG_EXEC) {
            has_exec_segment = 1;
            if (hdr->entry_point >= s->vaddr && hdr->entry_point < (s->vaddr + s->mem_size)) {
                entry_found = 1;
            }
        }
    }

    if (!has_exec_segment || !entry_found) {
        return APEX_ERR_INVALID_ENTRY;
    }

    return APEX_OK;
}

int apex_get_header(const void *data, size_t size, apex_header_t *out_hdr) {
    if (!data || !out_hdr) return APEX_ERR_NULL_PTR;
    int rc = apex_validate(data, size);
    if (rc != APEX_OK) return rc;

    memcpy(out_hdr, data, sizeof(apex_header_t));
    return APEX_OK;
}

int apex_get_segment(const void *data, size_t size, uint16_t index, apex_segment_t *out_seg) {
    if (!data || !out_seg) return APEX_ERR_NULL_PTR;
    int rc = apex_validate(data, size);
    if (rc != APEX_OK) return rc;

    const apex_header_t *hdr = (const apex_header_t *)data;
    if (index >= hdr->num_segments) return APEX_ERR_SEGMENT_BOUNDS;

    const apex_segment_t *segs = (const apex_segment_t *)((const uint8_t *)data + sizeof(apex_header_t));
    memcpy(out_seg, &segs[index], sizeof(apex_segment_t));
    return APEX_OK;
}

int apex_load(page_table_t *pml4, const void *data, size_t size, apex_binary_info_t *out_info) {
    if (!pml4 || !data || !out_info) return APEX_ERR_NULL_PTR;

    int rc = apex_validate(data, size);
    if (rc != APEX_OK) return rc;

    const apex_header_t *hdr = (const apex_header_t *)data;
    const apex_segment_t *segs = (const apex_segment_t *)((const uint8_t *)data + sizeof(apex_header_t));

    uint64_t min_va = 0xFFFFFFFFFFFFFFFFULL;
    uint64_t max_va = 0;

    /* 1. Map Program Segments */
    for (uint16_t i = 0; i < hdr->num_segments; i++) {
        const apex_segment_t *s = &segs[i];
        if (s->vaddr < min_va) min_va = s->vaddr;
        if (s->vaddr + s->mem_size > max_va) max_va = s->vaddr + s->mem_size;

        uint64_t page_start = s->vaddr & ~0xFFFULL;
        uint64_t page_end   = (s->vaddr + s->mem_size + 0xFFFULL) & ~0xFFFULL;
        size_t pages = (size_t)((page_end - page_start) / PAGE_SIZE_4K);

        uint64_t map_flags = PAGE_PRESENT | PAGE_USER;
        if (s->flags & APEX_FLAG_WRITE) map_flags |= PAGE_WRITABLE;
        if (!(s->flags & APEX_FLAG_EXEC)) map_flags |= PAGE_NO_EXECUTE;

        for (size_t p = 0; p < pages; p++) {
            uint64_t va = page_start + (p * PAGE_SIZE_4K);
            void *frame = pmm_alloc_frame();
            if (!frame) return APEX_ERR_OUT_OF_MEMORY;

            memset(frame, 0, PAGE_SIZE_4K);

            /* If segment has file data overlapping this page, copy it */
            if (s->type != APEX_SEG_BSS && s->file_size > 0) {
                uint64_t seg_offset_in_page = (va >= s->vaddr) ? (va - s->vaddr) : 0;
                if (seg_offset_in_page < s->file_size) {
                    size_t bytes_to_copy = PAGE_SIZE_4K;
                    if (seg_offset_in_page + bytes_to_copy > s->file_size) {
                        bytes_to_copy = (size_t)(s->file_size - seg_offset_in_page);
                    }
                    const uint8_t *src = (const uint8_t *)data + s->file_offset + seg_offset_in_page;
                    uint64_t dst_offset = (va < s->vaddr) ? (s->vaddr - va) : 0;
                    memcpy((uint8_t *)frame + dst_offset, src, bytes_to_copy);
                }
            }

            vmm_map_page(pml4, va, (uint64_t)(uintptr_t)frame, map_flags);
        }
    }

    /* 2. Allocate and Map User Stack */
    uint32_t stack_size = hdr->stack_size ? hdr->stack_size : APEX_DEFAULT_STACK_SIZE;
    if (stack_size > APEX_MAX_STACK_SIZE) stack_size = APEX_MAX_STACK_SIZE;
    if (stack_size < PAGE_SIZE_4K) stack_size = PAGE_SIZE_4K;

    uint64_t stack_top = 0x00007FFFF0000000ULL; /* Top of user stack */
    uint64_t stack_base = stack_top - stack_size;
    size_t stack_pages = (stack_size + PAGE_SIZE_4K - 1) / PAGE_SIZE_4K;

    for (size_t p = 0; p < stack_pages; p++) {
        uint64_t va = stack_base + (p * PAGE_SIZE_4K);
        void *frame = pmm_alloc_frame();
        if (!frame) return APEX_ERR_OUT_OF_MEMORY;
        memset(frame, 0, PAGE_SIZE_4K);
        vmm_map_page(pml4, va, (uint64_t)(uintptr_t)frame, PAGE_PRESENT | PAGE_USER | PAGE_WRITABLE | PAGE_NO_EXECUTE);
    }

    out_info->entry_point  = hdr->entry_point;
    out_info->stack_top    = stack_top;
    out_info->stack_base   = stack_base;
    out_info->stack_size   = stack_size;
    out_info->min_vaddr    = min_va;
    out_info->max_vaddr    = max_va;
    out_info->num_segments = hdr->num_segments;

    kinfo("APEX: Successfully loaded executable at %p, entry %p, stack %p..%p",
          (void *)min_va, (void *)hdr->entry_point, (void *)stack_base, (void *)stack_top);

    return APEX_OK;
}
