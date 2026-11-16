use super::pmm::{alloc_frame, PAGE_SIZE};
use crate::arch::x86_64::cpu::{invlpg, read_cr0, read_cr3, write_cr0, write_cr3};
use crate::sync::Spinlock;

pub const PAGE_PRESENT: u64 = 1 << 0;
pub const PAGE_WRITABLE: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
pub const PAGE_WRITE_THROUGH: u64 = 1 << 3;
pub const PAGE_CACHE_DISABLE: u64 = 1 << 4;
pub const PAGE_HUGE: u64 = 1 << 7;
pub const PAGE_NO_EXECUTE: u64 = 1 << 63;
pub const PAGE_MMIO: u64 = PAGE_PRESENT | PAGE_WRITABLE | PAGE_CACHE_DISABLE | PAGE_WRITE_THROUGH | PAGE_NO_EXECUTE;
pub const ENTRY_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self { entries: [0; 512] }
    }
}

static VMM_LOCK: Spinlock<()> = Spinlock::new(());
static mut KERNEL_PML4: *mut PageTable = core::ptr::null_mut();

#[inline]
pub fn is_canonical(vaddr: u64) -> bool {
    let sign = (vaddr as i64) >> 47;
    sign == 0 || sign == -1
}

#[inline]
fn pml4_index(vaddr: u64) -> usize {
    ((vaddr >> 39) & 0x1FF) as usize
}

#[inline]
fn pdpt_index(vaddr: u64) -> usize {
    ((vaddr >> 30) & 0x1FF) as usize
}

#[inline]
fn pd_index(vaddr: u64) -> usize {
    ((vaddr >> 21) & 0x1FF) as usize
}

#[inline]
fn pt_index(vaddr: u64) -> usize {
    ((vaddr >> 12) & 0x1FF) as usize
}

pub fn init() {
    let _guard = VMM_LOCK.lock();
    let cr3 = read_cr3();
    unsafe {
        KERNEL_PML4 = (cr3 & ENTRY_ADDR_MASK) as *mut PageTable;
    }

    // Enforce Write Protect (CR0.WP, bit 16)
    let cr0 = read_cr0();
    write_cr0(cr0 | (1 << 16));

    crate::drivers::serial::print("VMM Initialized: Kernel PML4 at 0x");
    crate::drivers::serial::print_hex(cr3 & ENTRY_ADDR_MASK);
    crate::drivers::serial::println(", CR0.WP enforced");
}

pub fn get_kernel_pml4() -> *mut PageTable {
    unsafe { KERNEL_PML4 }
}

pub fn split_huge_page(pml4: *mut PageTable, vaddr: u64) -> Result<(), &'static str> {
    if !is_canonical(vaddr) {
        return Err("Non-canonical address");
    }
    if pml4.is_null() {
        return Err("Null PML4");
    }

    let pml4_i = pml4_index(vaddr);
    let pdpt_i = pdpt_index(vaddr);
    let pd_i = pd_index(vaddr);

    unsafe {
        let pml4_entry = (*pml4).entries[pml4_i];
        if (pml4_entry & PAGE_PRESENT) == 0 {
            return Err("PML4 entry not present");
        }
        let pdpt = (pml4_entry & ENTRY_ADDR_MASK) as *mut PageTable;

        let pdpt_entry = (*pdpt).entries[pdpt_i];
        if (pdpt_entry & PAGE_PRESENT) == 0 {
            return Err("PDPT entry not present");
        }
        let pd = (pdpt_entry & ENTRY_ADDR_MASK) as *mut PageTable;

        let pd_entry = (*pd).entries[pd_i];
        if (pd_entry & PAGE_PRESENT) == 0 {
            return Err("PD entry not present");
        }

        // If not huge (already 4 KiB), split is not needed
        if (pd_entry & PAGE_HUGE) == 0 {
            return Ok(());
        }

        let new_pt_frame = alloc_frame().ok_or("Out of memory splitting huge page")?;
        let new_pt = new_pt_frame as *mut PageTable;
        (*new_pt).entries.fill(0);

        let huge_phys_base = pd_entry & 0x000F_FFFF_FFE0_0000;
        let common_flags = (pd_entry & 0xFFF) & !PAGE_HUGE;
        let nx_flag = pd_entry & PAGE_NO_EXECUTE;

        for i in 0..512 {
            (*new_pt).entries[i] = (huge_phys_base + (i * PAGE_SIZE) as u64) | common_flags | nx_flag;
        }

        (*pd).entries[pd_i] = (new_pt_frame as u64) | PAGE_PRESENT | PAGE_WRITABLE | (pd_entry & PAGE_USER);
        write_cr3(read_cr3());
    }

    Ok(())
}

pub fn map_page(pml4: *mut PageTable, vaddr: u64, paddr: u64, flags: u64) -> Result<(), &'static str> {
    if !is_canonical(vaddr) {
        return Err("Non-canonical address");
    }
    if (vaddr & 0xFFF) != 0 || (paddr & 0xFFF) != 0 {
        return Err("Unaligned address");
    }
    if pml4.is_null() {
        return Err("Null PML4");
    }

    let _guard = VMM_LOCK.lock();

    let pml4_i = pml4_index(vaddr);
    let pdpt_i = pdpt_index(vaddr);
    let pd_i = pd_index(vaddr);
    let pt_i = pt_index(vaddr);

    unsafe {
        let pdpt: *mut PageTable;
        if ((*pml4).entries[pml4_i] & PAGE_PRESENT) == 0 {
            let frame = alloc_frame().ok_or("Out of memory for PDPT")?;
            pdpt = frame as *mut PageTable;
            (*pdpt).entries.fill(0);
            (*pml4).entries[pml4_i] = (frame as u64) | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        } else {
            pdpt = ((*pml4).entries[pml4_i] & ENTRY_ADDR_MASK) as *mut PageTable;
            if (flags & PAGE_USER) != 0 {
                (*pml4).entries[pml4_i] |= PAGE_USER;
            }
        }

        let pd: *mut PageTable;
        if ((*pdpt).entries[pdpt_i] & PAGE_PRESENT) == 0 {
            let frame = alloc_frame().ok_or("Out of memory for PD")?;
            pd = frame as *mut PageTable;
            (*pd).entries.fill(0);
            (*pdpt).entries[pdpt_i] = (frame as u64) | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        } else {
            pd = ((*pdpt).entries[pdpt_i] & ENTRY_ADDR_MASK) as *mut PageTable;
            if (flags & PAGE_USER) != 0 {
                (*pdpt).entries[pdpt_i] |= PAGE_USER;
            }
        }

        if ((*pd).entries[pd_i] & PAGE_HUGE) != 0 {
            split_huge_page(pml4, vaddr)?;
        }

        let pt: *mut PageTable;
        if ((*pd).entries[pd_i] & PAGE_PRESENT) == 0 {
            let frame = alloc_frame().ok_or("Out of memory for PT")?;
            pt = frame as *mut PageTable;
            (*pt).entries.fill(0);
            (*pd).entries[pd_i] = (frame as u64) | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        } else {
            pt = ((*pd).entries[pd_i] & ENTRY_ADDR_MASK) as *mut PageTable;
            if (flags & PAGE_USER) != 0 {
                (*pd).entries[pd_i] |= PAGE_USER;
            }
        }

        (*pt).entries[pt_i] = (paddr & ENTRY_ADDR_MASK) | flags | PAGE_PRESENT;
        invlpg(vaddr);
    }

    Ok(())
}

pub fn unmap_page(pml4: *mut PageTable, vaddr: u64) -> Result<(), &'static str> {
    if !is_canonical(vaddr) || pml4.is_null() {
        return Err("Invalid address or PML4");
    }

    let _guard = VMM_LOCK.lock();

    let pml4_i = pml4_index(vaddr);
    let pdpt_i = pdpt_index(vaddr);
    let pd_i = pd_index(vaddr);
    let pt_i = pt_index(vaddr);

    unsafe {
        if ((*pml4).entries[pml4_i] & PAGE_PRESENT) == 0 {
            return Err("Not mapped");
        }
        let pdpt = ((*pml4).entries[pml4_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pdpt).entries[pdpt_i] & PAGE_PRESENT) == 0 {
            return Err("Not mapped");
        }
        let pd = ((*pdpt).entries[pdpt_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pd).entries[pd_i] & PAGE_HUGE) != 0 {
            split_huge_page(pml4, vaddr)?;
        }

        if ((*pd).entries[pd_i] & PAGE_PRESENT) == 0 {
            return Err("Not mapped");
        }
        let pt = ((*pd).entries[pd_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pt).entries[pt_i] & PAGE_PRESENT) == 0 {
            return Err("Not mapped");
        }

        (*pt).entries[pt_i] = 0;
        invlpg(vaddr);
    }

    Ok(())
}

pub fn get_mapping(pml4: *mut PageTable, vaddr: u64) -> Option<(u64, u64)> {
    if !is_canonical(vaddr) || pml4.is_null() {
        return None;
    }

    let pml4_i = pml4_index(vaddr);
    let pdpt_i = pdpt_index(vaddr);
    let pd_i = pd_index(vaddr);
    let pt_i = pt_index(vaddr);

    unsafe {
        if ((*pml4).entries[pml4_i] & PAGE_PRESENT) == 0 {
            return None;
        }
        let pdpt = ((*pml4).entries[pml4_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pdpt).entries[pdpt_i] & PAGE_PRESENT) == 0 {
            return None;
        }
        let pd = ((*pdpt).entries[pdpt_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pd).entries[pd_i] & PAGE_HUGE) != 0 {
            let paddr = ((*pd).entries[pd_i] & 0x000F_FFFF_FFE0_0000) + (vaddr & 0x1F_FFFF);
            let flags = (*pd).entries[pd_i] & (0xFFF | PAGE_NO_EXECUTE);
            return Some((paddr, flags));
        }

        if ((*pd).entries[pd_i] & PAGE_PRESENT) == 0 {
            return None;
        }
        let pt = ((*pd).entries[pd_i] & ENTRY_ADDR_MASK) as *mut PageTable;

        if ((*pt).entries[pt_i] & PAGE_PRESENT) == 0 {
            return None;
        }

        let paddr = ((*pt).entries[pt_i] & ENTRY_ADDR_MASK) + (vaddr & 0xFFF);
        let flags = (*pt).entries[pt_i] & (0xFFF | PAGE_NO_EXECUTE);
        Some((paddr, flags))
    }
}

pub fn create_pml4() -> Option<*mut PageTable> {
    let frame = alloc_frame()?;
    let pml4 = frame as *mut PageTable;
    unsafe {
        (*pml4).entries.fill(0);
        if !KERNEL_PML4.is_null() {
            // Copy higher-half kernel space (entries 256..511)
            for i in 256..512 {
                (*pml4).entries[i] = (*KERNEL_PML4).entries[i];
            }
            // Keep lower 512 GiB identity mapping
            (*pml4).entries[0] = (*KERNEL_PML4).entries[0];
        }
    }
    Some(pml4)
}

pub fn switch_pml4(pml4: *mut PageTable) {
    if !pml4.is_null() {
        write_cr3(pml4 as u64);
    }
}

/// Identity-maps a physical MMIO aperture with Cache-Disable and Write-Through attributes
pub fn map_mmio_range(paddr: u64, size: usize) -> Result<(), &'static str> {
    let _guard = VMM_LOCK.lock();
    unsafe {
        let pml4 = KERNEL_PML4;
        if pml4.is_null() {
            return Err("VMM not initialized");
        }
        let mut curr = paddr & !0xFFF;
        let end = (paddr + size as u64 + 0xFFF) & !0xFFF;
        while curr < end {
            map_page(pml4, curr, curr, PAGE_MMIO)?;
            curr += PAGE_SIZE as u64;
        }
        Ok(())
    }
}
