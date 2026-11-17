# Memory Management Design (PMM & VMM)

## 1. Physical Memory Manager (PMM)

- **Source:** [`src/mm/pmm.rs`](src/mm/pmm.rs)
- **Granularity:** 4 KiB frames.
- **Capacity:** Manages addresses from `0x00000000` to `0x100000000` (4 GiB address space, 1,048,576 frames).
- **Dual-Bitmap Architecture:**
  - `ALLOC_BITMAP[131072]`: Tracks dynamic allocation state (0 = free, 1 = allocated).
  - `RESERVED_BITMAP[131072]`: Tracks permanently reserved physical memory (BIOS, low RAM, hardware MMIO, kernel code).
- **Mandatory Reservations:**
  - `0x00000000` to `0x01000000` (first 16 MiB) is permanently reserved.
  - Kernel image (`_kernel_start` to `_kernel_end`) is permanently reserved.
  - Multiboot headers and memory maps are reserved during initialization.
- **Robustness:**
  - Protected against double frees, reserved page frees, and unaligned addresses.
  - Allocation exhaustion handled safely returning `None`.

## 2. Virtual Memory Manager (VMM)

- **Source:** [`src/mm/vmm.rs`](src/mm/vmm.rs)
- **Hierarchy:** 4-level paging (PML4, PDPT, PD, PT).
- **Huge Page Support:**
  - Bootstrap identity maps the first 4 GiB using 2 MiB huge pages.
  - Dynamic splitting: When a 4 KiB mapping request targets an address covered by a 2 MiB page, `split_huge_page()` automatically allocates a new page table and splits the huge page into 512 individual 4 KiB pages on demand.
- **W^X Enforcement:**
  - Code pages have `PAGE_NX` cleared and `PAGE_WRITABLE` cleared.
  - Data pages have `PAGE_NX` set and `PAGE_WRITABLE` set.
  - Supervisor write-protection (`CR0.WP`) is enforced.

## 3. Dynamic Kernel Heap Allocator

- **Source:** [`src/mm/heap.rs`](src/mm/heap.rs)
- **Design:** Address-ordered coalescing free-list allocator managing 2 MiB of dynamic heap starting at `0x1000000`.
- Implements Rust standard `GlobalAlloc` trait (`alloc`, `dealloc`).

## 4. Per-Process Virtual Address Spaces (Private PML4 & CR3 Switching)

- **Source:** [`src/kernel/task.rs`](src/kernel/task.rs), [`src/mm/vmm.rs`](src/mm/vmm.rs)
- **Private Page Tables:** `create_pml4()` allocates a dedicated 4 KiB frame for user tasks.
- **Kernel Mirroring:** Higher-half kernel space (PML4 entries 256..511) and lower 512 GiB identity mappings are mirrored into every task PML4.
- **Dynamic Context Switching:** Preemptive scheduler context switcher inspects `task.cr3` and dynamically reloads `%cr3` to activate the task's private virtual address space.
