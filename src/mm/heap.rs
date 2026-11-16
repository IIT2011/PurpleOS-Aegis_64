//! Aegis64 Kernel Dynamic Memory Heap Allocator
//!
//! Provides a thread-safe, lock-protected Free-List Heap Allocator implementing
//! `core::alloc::GlobalAlloc`. Unlocks standard dynamic data structures:
//! `Vec`, `String`, `Box`, `BTreeMap`, and heap-allocated driver structures.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use crate::sync::Spinlock;

/// Minimum block allocation size (must fit free list node)
const MIN_ALLOC_SIZE: usize = 32;

#[repr(C)]
struct FreeListNode {
    size: usize,
    next: *mut FreeListNode,
}

pub struct Heap {
    head: *mut FreeListNode,
    pub total_bytes: usize,
    pub allocated_bytes: usize,
}

unsafe impl Send for Heap {}

impl Heap {
    pub const fn empty() -> Self {
        Self {
            head: null_mut(),
            total_bytes: 0,
            allocated_bytes: 0,
        }
    }

    /// Initializes heap with a memory region [start, start + size)
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        // Align start address to 16 bytes
        let aligned_start = (start + 15) & !15;
        let aligned_size = (start + size).saturating_sub(aligned_start);

        if aligned_size >= core::mem::size_of::<FreeListNode>() {
            let node = aligned_start as *mut FreeListNode;
            (*node).size = aligned_size;
            (*node).next = null_mut();
            self.head = node;
            self.total_bytes = aligned_size;
            self.allocated_bytes = 0;
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(core::mem::align_of::<usize>()).max(16);
        let size = layout.size().max(MIN_ALLOC_SIZE);

        let mut prev: *mut FreeListNode = null_mut();
        let mut curr = self.head;

        while !curr.is_null() {
            unsafe {
                let curr_addr = curr as usize;
                // Calculate aligned user payload address after block header (size: usize)
                let header_size = core::mem::size_of::<usize>();
                let payload_start = (curr_addr + header_size + (align - 1)) & !(align - 1);
                let needed_size = (payload_start - curr_addr) + size;

                if (*curr).size >= needed_size {
                    let remaining = (*curr).size - needed_size;

                    if remaining >= MIN_ALLOC_SIZE + header_size {
                        // Split block
                        let next_block = (curr_addr + needed_size) as *mut FreeListNode;
                        (*next_block).size = remaining;
                        (*next_block).next = (*curr).next;

                        if prev.is_null() {
                            self.head = next_block;
                        } else {
                            (*prev).next = next_block;
                        }

                        // Store actual allocated block size right before payload
                        let size_header = (payload_start - header_size) as *mut usize;
                        *size_header = needed_size;
                    } else {
                        // Consume whole block
                        if prev.is_null() {
                            self.head = (*curr).next;
                        } else {
                            (*prev).next = (*curr).next;
                        }
                        let size_header = (payload_start - header_size) as *mut usize;
                        *size_header = (*curr).size;
                    }

                    self.allocated_bytes += size;
                    return payload_start as *mut u8;
                }

                prev = curr;
                curr = (*curr).next;
            }
        }

        null_mut()
    }

    pub fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        unsafe {
            let header_size = core::mem::size_of::<usize>();
            let size_header = (ptr as usize - header_size) as *const usize;
            let block_size = *size_header;
            let block_start = ptr as usize - header_size;

            let freed_node = block_start as *mut FreeListNode;
            (*freed_node).size = block_size;
            (*freed_node).next = self.head;
            self.head = freed_node;

            self.allocated_bytes = self.allocated_bytes.saturating_sub(layout.size());
        }
    }
}

pub struct LockedHeap(Spinlock<Heap>);

impl LockedHeap {
    pub const fn empty() -> Self {
        Self(Spinlock::new(Heap::empty()))
    }

    pub fn lock(&self) -> crate::sync::SpinlockGuard<'_, Heap> {
        self.0.lock()
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(ptr, layout)
    }
}

#[global_allocator]
pub static GLOBAL_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initializes 2 MiB Kernel Dynamic Heap from Physical Frame Allocator
pub fn init() {
    // Allocate 512 contiguous 4 KiB frames (2 MiB heap pool)
    if let Some(heap_phys) = crate::mm::pmm::alloc_frames(512) {
        let heap_size = 512 * 4096;
        unsafe {
            GLOBAL_ALLOCATOR.lock().init(heap_phys as usize, heap_size);
        }
        crate::drivers::serial::print("Heap: Dynamic Kernel Heap Initialized (2 MiB at 0x");
        crate::drivers::serial::print_hex(heap_phys as u64);
        crate::drivers::serial::println(", GlobalAlloc Active)");
    } else {
        crate::drivers::serial::println("Heap: FAILED to allocate frames for kernel heap!");
    }
}

pub fn get_stats() -> (usize, usize, usize) {
    let h = GLOBAL_ALLOCATOR.lock();
    (h.total_bytes, h.allocated_bytes, h.total_bytes.saturating_sub(h.allocated_bytes))
}
