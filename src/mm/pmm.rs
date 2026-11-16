use crate::arch::x86_64::boot::{MultibootInfo, MultibootMmapEntry, MULTIBOOT_INFO_MEM_MAP, MULTIBOOT_MEMORY_AVAILABLE};
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

pub fn init(mbi: Option<&MultibootInfo>, kernel_start: usize, kernel_end: usize) {
    let mut state = PMM.lock();

    state.alloc_bitmap.fill(0xFF);
    state.reserved_bitmap.fill(0xFF);
    state.total_frames = TOTAL_FRAMES;
    state.free_frames = 0;
    state.allocated_frames = TOTAL_FRAMES;
    state.reserved_frames = TOTAL_FRAMES;

    if let Some(info) = mbi {
        if (info.flags & MULTIBOOT_INFO_MEM_MAP) != 0 && info.mmap_length > 0 {
            let mut cur = info.mmap_addr as usize;
            let end = cur + info.mmap_length as usize;

            while cur < end {
                let entry = unsafe { core::ptr::read_unaligned(cur as *const MultibootMmapEntry) };
                if entry.size == 0 {
                    break;
                }

                if entry.entry_type == MULTIBOOT_MEMORY_AVAILABLE {
                    let base = entry.addr as usize;
                    let len = entry.len as usize;

                    if base < MAX_PHYS_ADDR {
                        let limit = (base + len).min(MAX_PHYS_ADDR);
                        let start_f = (base + PAGE_SIZE - 1) / PAGE_SIZE;
                        let end_f = limit / PAGE_SIZE;

                        for f in start_f..end_f.min(TOTAL_FRAMES) {
                            if bitmap_test(&state.reserved_bitmap, f) {
                                bitmap_clear(&mut state.reserved_bitmap, f);
                                bitmap_clear(&mut state.alloc_bitmap, f);
                                state.free_frames += 1;
                                state.allocated_frames -= 1;
                                state.reserved_frames -= 1;
                            }
                        }
                    }
                }

                cur += entry.size as usize + 4;
            }
        }
    }

    if state.free_frames == 0 {
        crate::drivers::serial::println("PMM: Safe fallback memory pool (16 MiB - 256 MiB)");
        let start_f = (16 * 1024 * 1024) / PAGE_SIZE;
        let end_f = (256 * 1024 * 1024) / PAGE_SIZE;
        for f in start_f..end_f {
            bitmap_clear(&mut state.reserved_bitmap, f);
            bitmap_clear(&mut state.alloc_bitmap, f);
            state.free_frames += 1;
            state.allocated_frames -= 1;
            state.reserved_frames -= 1;
        }
    }

    // Always reserve low 16 MiB (DMA, BIOS, AP trampoline at 0x8000, video)
    mark_region_reserved(&mut state, 0x0, 0x1000000);

    // Reserve kernel image
    mark_region_reserved(&mut state, kernel_start, kernel_end);

    crate::drivers::serial::print("PMM Initialized: Managed Frames=");
    crate::drivers::serial::print_u64(state.total_frames as u64);
    crate::drivers::serial::print(", Free=");
    crate::drivers::serial::print_u64(state.free_frames as u64);
    crate::drivers::serial::println(" frames");
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

pub fn free_frames_count() -> usize {
    PMM.lock().free_frames
}
