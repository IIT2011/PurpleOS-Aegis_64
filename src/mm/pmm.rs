use crate::arch::x86_64::boot::{
    memory_map_entries, MultibootInfo, MULTIBOOT_INFO_FRAMEBUFFER, MULTIBOOT_INFO_MEM_MAP,
    MULTIBOOT_MEMORY_AVAILABLE,
};
use crate::sync::Spinlock;

pub const PAGE_SIZE: usize = 4096;
pub const MAX_PHYS_ADDR: usize = 0x100000000; // 4 GiB
pub const TOTAL_FRAMES: usize = MAX_PHYS_ADDR / PAGE_SIZE; // 1,048,576 frames
pub const BITMAP_SIZE: usize = TOTAL_FRAMES / 8; // 128 KiB

struct PmmState {
    alloc_bitmap: [u8; BITMAP_SIZE],
    reserved_bitmap: [u8; BITMAP_SIZE],
    total_frames: usize,
    free_frames: usize,
    allocated_frames: usize,
    reserved_frames: usize,
    last_alloc_frame: usize,
}

static PMM: Spinlock<PmmState> = Spinlock::new(PmmState {
    alloc_bitmap: [0xFF; BITMAP_SIZE],
    reserved_bitmap: [0xFF; BITMAP_SIZE],
    total_frames: TOTAL_FRAMES,
    free_frames: 0,
    allocated_frames: TOTAL_FRAMES,
    reserved_frames: TOTAL_FRAMES,
    last_alloc_frame: 0,
});

#[inline]
fn bitmap_set(bm: &mut [u8; BITMAP_SIZE], frame: usize) {
    bm[frame / 8] |= 1 << (frame % 8);
}

#[inline]
fn bitmap_clear(bm: &mut [u8; BITMAP_SIZE], frame: usize) {
    bm[frame / 8] &= !(1 << (frame % 8));
}

#[inline]
fn bitmap_test(bm: &[u8; BITMAP_SIZE], frame: usize) -> bool {
    (bm[frame / 8] & (1 << (frame % 8))) != 0
}

fn mark_region_reserved(state: &mut PmmState, start_addr: usize, end_addr: usize) {
    if start_addr >= MAX_PHYS_ADDR {
        return;
    }
    let end = end_addr.min(MAX_PHYS_ADDR);
    let start_frame = start_addr / PAGE_SIZE;
    let end_frame = (end + PAGE_SIZE - 1) / PAGE_SIZE;

    for f in start_frame..end_frame.min(TOTAL_FRAMES) {
        if !bitmap_test(&state.reserved_bitmap, f) {
            bitmap_set(&mut state.reserved_bitmap, f);
            if !bitmap_test(&state.alloc_bitmap, f) {
                bitmap_set(&mut state.alloc_bitmap, f);
                if state.free_frames > 0 {
                    state.free_frames -= 1;
                }
            }
            state.reserved_frames += 1;
        }
    }
}

pub fn init(info: &MultibootInfo, kernel_start: usize, kernel_end: usize) -> Result<(), &'static str> {
    if info.flags & MULTIBOOT_INFO_MEM_MAP == 0 || info.mmap_length == 0 {
        return Err("boot memory map unavailable");
    }
    let map_start = info.mmap_addr as usize;
    let map_length = info.mmap_length as usize;
    let map_end = map_start.checked_add(map_length).ok_or("boot memory map overflows")?;
    if map_start < PAGE_SIZE || map_end > MAX_PHYS_ADDR {
        return Err("boot memory map outside the identity mapping");
    }

    // The boot ABI supplies this physical range within the early identity map.
    let map = unsafe { core::slice::from_raw_parts(map_start as *const u8, map_length) };
    let mut state = PMM.lock();
    state.alloc_bitmap.fill(0xFF);
    state.reserved_bitmap.fill(0xFF);
    state.total_frames = 0;
    state.free_frames = 0;
    state.allocated_frames = 0;
    state.reserved_frames = 0;
    state.last_alloc_frame = 0;

    for record in memory_map_entries(map) {
        let entry = record?;
        if entry.entry_type != MULTIBOOT_MEMORY_AVAILABLE || entry.len == 0 {
            continue;
        }
        let base = entry.addr as usize;
        let limit = ((entry.addr + entry.len) as usize).min(MAX_PHYS_ADDR);
        if base >= limit {
            continue;
        }
        let first = base.div_ceil(PAGE_SIZE);
        let end = limit / PAGE_SIZE;
        state.total_frames = state.total_frames.max(end);
        for frame in first..end {
            if bitmap_test(&state.reserved_bitmap, frame) {
                bitmap_clear(&mut state.reserved_bitmap, frame);
                bitmap_clear(&mut state.alloc_bitmap, frame);
                state.free_frames += 1;
            }
        }
    }

    // A reserved descriptor wins even when firmware also reports overlapping RAM.
    for record in memory_map_entries(map) {
        let entry = record?;
        if entry.entry_type != MULTIBOOT_MEMORY_AVAILABLE && entry.len != 0 {
            mark_region_reserved(&mut state, entry.addr as usize, (entry.addr + entry.len) as usize);
        }
    }

    // Keep the firmware handoff and live framebuffer outside allocator ownership.
    mark_region_reserved(&mut state, 0, 0x1000000);
    mark_region_reserved(&mut state, kernel_start, kernel_end);
    mark_region_reserved(&mut state, map_start, map_end);
    let info_start = info as *const MultibootInfo as usize;
    mark_region_reserved(&mut state, info_start, info_start + core::mem::size_of::<MultibootInfo>());
    if info.flags & MULTIBOOT_INFO_FRAMEBUFFER != 0 {
        let base = info.framebuffer_addr;
        let bytes = u64::from(info.framebuffer_pitch) * u64::from(info.framebuffer_height);
        if let Some(end) = base.checked_add(bytes).filter(|&end| end > base) {
            mark_region_reserved(&mut state, base as usize, end as usize);
        }
    }

    if state.free_frames == 0 {
        return Err("boot memory map contains no usable frames");
    }
    state.reserved_frames = state.total_frames.saturating_sub(state.free_frames);
    crate::drivers::serial::print("PMM Initialized: Managed Frames=");
    crate::drivers::serial::print_u64(state.total_frames as u64);
    crate::drivers::serial::print(", Free=");
    crate::drivers::serial::print_u64(state.free_frames as u64);
    crate::drivers::serial::println(" frames");
    Ok(())
}

pub fn alloc_frame() -> Option<usize> {
    let mut state = PMM.lock();
    if state.free_frames == 0 {
        return None;
    }

    let start = state.last_alloc_frame;
    let mut step = 0;
    while step < TOTAL_FRAMES {
        let i = (start + step) % TOTAL_FRAMES;

        // Fast skip allocated bytes
        if i % 8 == 0 && state.alloc_bitmap[i / 8] == 0xFF {
            step += 8;
            continue;
        }

        if !bitmap_test(&state.alloc_bitmap, i) && !bitmap_test(&state.reserved_bitmap, i) {
            bitmap_set(&mut state.alloc_bitmap, i);
            state.free_frames -= 1;
            state.allocated_frames += 1;
            state.last_alloc_frame = (i + 1) % TOTAL_FRAMES;
            return Some(i * PAGE_SIZE);
        }

        step += 1;
    }
    None
}

pub fn alloc_frames(count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    if count == 1 {
        return alloc_frame();
    }

    let mut state = PMM.lock();
    if state.free_frames < count {
        return None;
    }
    let mut run_start = 0;
    let mut run_len = 0;

    for i in 0..TOTAL_FRAMES {
        if !bitmap_test(&state.alloc_bitmap, i) && !bitmap_test(&state.reserved_bitmap, i) {
            if run_len == 0 {
                run_start = i;
            }
            run_len += 1;
            if run_len == count {
                for j in run_start..(run_start + count) {
                    bitmap_set(&mut state.alloc_bitmap, j);
                }
                state.free_frames -= count;
                state.allocated_frames += count;
                return Some(run_start * PAGE_SIZE);
            }
        } else {
            run_len = 0;
        }
    }
    None
}

pub fn free_frame(paddr: usize) -> Result<(), &'static str> {
    if paddr % PAGE_SIZE != 0 {
        return Err("Unaligned physical address");
    }
    if paddr >= MAX_PHYS_ADDR {
        return Err("Out of bounds physical address");
    }

    let mut state = PMM.lock();
    let frame = paddr / PAGE_SIZE;

    if bitmap_test(&state.reserved_bitmap, frame) {
        return Err("Cannot free reserved frame");
    }
    if !bitmap_test(&state.alloc_bitmap, frame) {
        return Err("Double free on frame");
    }

    bitmap_clear(&mut state.alloc_bitmap, frame);
    state.free_frames += 1;
    state.allocated_frames -= 1;
    Ok(())
}

pub fn free_frames(paddr: usize, count: usize) -> Result<(), &'static str> {
    for i in 0..count {
        free_frame(paddr + i * PAGE_SIZE)?;
    }
    Ok(())
}

/// A temporary, identity-mapped buffer that bypasses the small kernel heap.
pub(crate) struct PageBuffer {
    address: usize,
    length: usize,
}

impl PageBuffer {
    pub fn new(length: usize) -> Option<Self> {
        if length == 0 {
            return None;
        }
        let address = alloc_frames(length.div_ceil(PAGE_SIZE))?;
        unsafe { (address as *mut u8).write_bytes(0, length) };
        Some(Self { address, length })
    }
}

impl core::ops::Deref for PageBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.address as *const u8, self.length) }
    }
}

impl core::ops::DerefMut for PageBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.address as *mut u8, self.length) }
    }
}

impl Drop for PageBuffer {
    fn drop(&mut self) {
        let _ = free_frames(self.address, self.length.div_ceil(PAGE_SIZE));
    }
}

pub fn free_frames_count() -> usize {
    PMM.lock().free_frames
}

pub fn total_frames_count() -> usize {
    PMM.lock().total_frames
}

pub fn allocated_frames_count() -> usize {
    PMM.lock().allocated_frames
}

pub fn reserved_frames_count() -> usize {
    PMM.lock().reserved_frames
}
