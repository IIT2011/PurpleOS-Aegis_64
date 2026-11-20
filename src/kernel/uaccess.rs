//! Aegis64 User Memory Access & Validation Primitives (x86_64)
//!
//! Enforces strict separation of user space and kernel space memory.
//! Validates canonical ranges, page table presence, PAGE_USER and PAGE_WRITABLE flags
//! before copying data between kernel buffers and userspace virtual addresses.
//! Zero unchecked raw user pointer dereferencing.

use crate::arch::x86_64::cpu::read_cr3;
use crate::kernel::posix::{EFAULT, ENAMETOOLONG};
use crate::mm::vmm::{self, PageTable, PAGE_PRESENT, PAGE_USER, PAGE_WRITABLE};

pub const USER_SPACE_MIN: u64 = 0x0001_0000; // Prevent null page and low 64 KiB attacks
pub const USER_SPACE_MAX: u64 = 0x0000_7FFF_FFFF_F000; // Canonical x86_64 user virtual address upper limit

/// Returns the currently active PML4 from CR3, or falls back to kernel PML4 if uninitialized.
#[inline]
pub fn get_active_pml4() -> *mut PageTable {
    let cr3 = read_cr3();
    let paddr = cr3 & vmm::ENTRY_ADDR_MASK;
    if paddr != 0 {
        paddr as *mut PageTable
    } else {
        vmm::get_kernel_pml4()
    }
}

/// Validates whether a virtual memory range [start, start + len) resides completely
/// within canonical user space and that all spanned pages are mapped with PAGE_USER,
/// and optionally PAGE_WRITABLE when `write` is true.
pub fn validate_user_range(start: u64, len: usize, write: bool) -> Result<(), i64> {
    if len == 0 {
        if start < USER_SPACE_MIN || start >= USER_SPACE_MAX {
            return Err(-EFAULT);
        }
        return Ok(());
    }

    let end = match start.checked_add(len as u64) {
        Some(e) => e,
        None => return Err(-EFAULT),
    };

    if start < USER_SPACE_MIN || end > USER_SPACE_MAX {
        return Err(-EFAULT);
    }

    let pml4 = get_active_pml4();
    if pml4.is_null() {
        return Err(-EFAULT);
    }

    let page_start = start & !0xFFF;
    let page_end = (end.saturating_add(0xFFF)) & !0xFFF;
    let mut curr_page = page_start;

    while curr_page < page_end {
        match vmm::get_mapping(pml4, curr_page) {
            Some((_paddr, flags)) => {
                if (flags & PAGE_PRESENT) == 0 || (flags & PAGE_USER) == 0 {
                    return Err(-EFAULT);
                }
                if write && (flags & PAGE_WRITABLE) == 0 {
                    return Err(-EFAULT);
                }
            }
            None => return Err(-EFAULT),
        }

        curr_page = match curr_page.checked_add(4096) {
            Some(next) => next,
            None => break,
        };
    }

    Ok(())
}

/// Safely copies `kernel_dst.len()` bytes from user address `user_src` to `kernel_dst`.
pub fn copy_from_user(user_src: u64, kernel_dst: &mut [u8]) -> Result<(), i64> {
    if kernel_dst.is_empty() {
        return Ok(());
    }
    validate_user_range(user_src, kernel_dst.len(), false)?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            user_src as *const u8,
            kernel_dst.as_mut_ptr(),
            kernel_dst.len(),
        );
    }
    Ok(())
}

/// Safely copies `kernel_src.len()` bytes from `kernel_src` to user address `user_dst`.
pub fn copy_to_user(kernel_src: &[u8], user_dst: u64) -> Result<(), i64> {
    if kernel_src.is_empty() {
        return Ok(());
    }
    validate_user_range(user_dst, kernel_src.len(), true)?;
    unsafe {
        core::ptr::copy_nonoverlapping(kernel_src.as_ptr(), user_dst as *mut u8, kernel_src.len());
    }
    Ok(())
}

/// Safely copies a null-terminated string from user space `user_src` into `kernel_dst`.
/// Returns the length of the string in bytes (excluding the null terminator).
pub fn copy_str_from_user(user_src: u64, kernel_dst: &mut [u8]) -> Result<usize, i64> {
    if kernel_dst.is_empty() || user_src < USER_SPACE_MIN || user_src >= USER_SPACE_MAX {
        return Err(-EFAULT);
    }

    let pml4 = get_active_pml4();
    if pml4.is_null() {
        return Err(-EFAULT);
    }

    let mut copied = 0usize;
    let max_len = kernel_dst.len();
    let mut curr_ptr = user_src;

    while copied < max_len {
        if curr_ptr < USER_SPACE_MIN || curr_ptr >= USER_SPACE_MAX {
            return Err(-EFAULT);
        }
        if copied == 0 || (curr_ptr & 0xFFF) == 0 {
            let page = curr_ptr & !0xFFF;
            match vmm::get_mapping(pml4, page) {
                Some((_paddr, flags)) => {
                    if (flags & PAGE_PRESENT) == 0 || (flags & PAGE_USER) == 0 {
                        return Err(-EFAULT);
                    }
                }
                None => return Err(-EFAULT),
            }
        }

        let b = unsafe { core::ptr::read_volatile(curr_ptr as *const u8) };
        if b == 0 {
            kernel_dst[copied] = 0;
            return Ok(copied);
        }
        kernel_dst[copied] = b;
        copied += 1;
        curr_ptr = match curr_ptr.checked_add(1) {
            Some(next) if next <= USER_SPACE_MAX => next,
            _ => return Err(-EFAULT),
        };
    }

    // String was not null-terminated within max_len
    Err(-ENAMETOOLONG)
}
