# Memory Management Design (PMM & VMM)

## 1. Physical Memory Manager (PMM)

- **Source:** [`src/mm/pmm.c`](file:///home/saptanshu-ganguly/OS/src/mm/pmm.c), [`include/pmm.h`](file:///home/saptanshu-ganguly/OS/include/pmm.h)
- **Granularity:** 4 KiB frames.
- **Capacity:** Manages addresses from `0x00000000` to `0x100000000` (4 GiB address space, 1,048,576 frames).
- **Dual-Bitmap Architecture:**
  - `pmm_alloc_bitmap[131072]`: Tracks dynamic allocation state (0 = free, 1 = allocated).
  - `pmm_reserved_bitmap[131072]`: Tracks permanently reserved physical memory (BIOS, low RAM, hardware MMIO, kernel code).
- **Mandatory Reservations:**
  - `0x00000000` to `0x01000000` (first 16 MiB) is permanently reserved.
  - Kernel image (`_kernel_start` to `_kernel_end`) is permanently reserved.
  - Multiboot headers and memory maps are reserved during initialization.
- **Robustness:**
  - Protected against double frees (`PMM_ERR_ALREADY_FREE`).
  - Protected against reserved page frees (`PMM_ERR_RESERVED`).
  - Protected against out-of-bounds addresses (`PMM_ERR_OUT_OF_BOUNDS`).
  - Protected against unaligned frees (`PMM_ERR_INVALID_PARAM`).
  - Allocation exhaustion handled safely (`pmm_alloc_frame()` returns `NULL`).

## 2. Virtual Memory Manager (VMM)

- **Source:** [`src/mm/vmm.c`](file:///home/saptanshu-ganguly/OS/src/mm/vmm.c), [`include/vmm.h`](file:///home/saptanshu-ganguly/OS/include/vmm.h)
- **Hierarchy:** 4-level paging (PML4, PDPT, PD, PT).
- **Huge Page Support:**
  - Bootstrap identity maps the first 4 GiB using 2 MiB huge pages.
  - Dynamic splitting: When a 4 KiB mapping request targets an address covered by a 2 MiB page, `vmm_split_huge_page()` automatically allocates a new page table and splits the huge page into 512 individual 4 KiB pages on demand.
- **Rollback on Allocation Failure:**
  - Intermediate page table allocations (PDPT, PD, PT) rollback and release allocated frames if any step fails.
- **W^X Enforcement:**
  - Code pages have `PAGE_NO_EXECUTE` cleared and `PAGE_WRITABLE` cleared.
  - Data pages have `PAGE_NO_EXECUTE` set and `PAGE_WRITABLE` set.
