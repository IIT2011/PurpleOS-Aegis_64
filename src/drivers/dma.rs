use crate::mm::pmm::{alloc_frames, free_frames, PAGE_SIZE};

pub unsafe fn alloc_coherent(bytes: usize) -> Option<(*mut u8, u64)> {
    if bytes == 0 {
        return None;
    }
    let pages = bytes.saturating_add(PAGE_SIZE - 1) / PAGE_SIZE;
    let paddr = alloc_frames(pages)?;
    let ptr = paddr as *mut u8;
    core::ptr::write_bytes(ptr, 0, pages * PAGE_SIZE);
    Some((ptr, paddr as u64))
}

pub unsafe fn free_coherent(ptr: *mut u8, bytes: usize) {
    if !ptr.is_null() && bytes > 0 {
        let pages = bytes.saturating_add(PAGE_SIZE - 1) / PAGE_SIZE;
        let _ = free_frames(ptr as usize, pages);
    }
}
