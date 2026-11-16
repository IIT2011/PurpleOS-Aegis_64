use core::arch::asm;

#[inline]
pub fn cli() {
    unsafe { asm!("cli", options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn sti() {
    unsafe { asm!("sti", options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn hlt() {
    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn pause() {
    unsafe { asm!("pause", options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn read_rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!("pushfq; pop {}", out(reg) rflags, options(nomem, preserves_flags));
    }
    rflags
}

#[inline]
pub fn read_and_disable_interrupts() -> u64 {
    let flags = read_rflags();
    cli();
    flags
}

#[inline]
pub fn restore_interrupts(flags: u64) {
    if (flags & 0x200) != 0 {
        sti();
    }
}

#[inline]
pub fn read_cr0() -> u64 {
    let cr0: u64;
    unsafe { asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags)) };
    cr0
}

#[inline]
pub fn write_cr0(val: u64) {
    unsafe { asm!("mov cr0, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn read_cr2() -> u64 {
    let cr2: u64;
    unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };
    cr2
}

#[inline]
pub fn read_cr3() -> u64 {
    let cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    cr3
}

#[inline]
pub fn write_cr3(val: u64) {
    unsafe { asm!("mov cr3, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn read_cr4() -> u64 {
    let cr4: u64;
    unsafe { asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags)) };
    cr4
}

#[inline]
pub fn write_cr4(val: u64) {
    unsafe { asm!("mov cr4, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline]
pub fn invlpg(vaddr: u64) {
    unsafe { asm!("invlpg [{}]", in(reg) vaddr, options(nostack, preserves_flags)) };
}

#[inline]
pub fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    }
    ((high as u64) << 32) | (low as u64)
}

#[inline]
pub fn wrmsr(msr: u32, val: u64) {
    let low = val as u32;
    let high = (val >> 32) as u32;
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high, options(nomem, nostack, preserves_flags));
    }
}

pub struct CpuIdResult {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

pub fn cpuid(leaf: u32, subleaf: u32) -> CpuIdResult {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            ebx_out = out(reg) ebx,
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            out("edx") edx,
            options(nostack, preserves_flags)
        );
    }
    CpuIdResult { eax, ebx, ecx, edx }
}

#[derive(Copy, Clone)]
pub struct CpuInfo {
    pub vendor: [u8; 13],
    pub brand: [u8; 49],
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_avx: bool,
    pub has_apic: bool,
    pub apic_id: u8,
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            vendor: [0; 13],
            brand: [0; 49],
            has_sse: false,
            has_sse2: false,
            has_avx: false,
            has_apic: false,
            apic_id: 0,
        }
    }
}

static mut BSP_INFO: CpuInfo = CpuInfo {
    vendor: [0; 13],
    brand: [0; 49],
    has_sse: false,
    has_sse2: false,
    has_avx: false,
    has_apic: false,
    apic_id: 0,
};

pub fn init_bsp() {
    let res0 = cpuid(0, 0);
    let mut vendor = [0u8; 13];
    vendor[0..4].copy_from_slice(&res0.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&res0.edx.to_le_bytes());
    vendor[8..12].copy_from_slice(&res0.ecx.to_le_bytes());

    let res1 = cpuid(1, 0);
    let has_sse = (res1.edx & (1 << 25)) != 0;
    let has_sse2 = (res1.edx & (1 << 26)) != 0;
    let has_apic = (res1.edx & (1 << 9)) != 0;
    let has_avx = (res1.ecx & (1 << 28)) != 0;
    let apic_id = ((res1.ebx >> 24) & 0xFF) as u8;

    let mut brand = [0u8; 49];
    let ext_max = cpuid(0x80000000, 0).eax;
    if ext_max >= 0x80000004 {
        let b1 = cpuid(0x80000002, 0);
        let b2 = cpuid(0x80000003, 0);
        let b3 = cpuid(0x80000004, 0);
        brand[0..4].copy_from_slice(&b1.eax.to_le_bytes());
        brand[4..8].copy_from_slice(&b1.ebx.to_le_bytes());
        brand[8..12].copy_from_slice(&b1.ecx.to_le_bytes());
        brand[12..16].copy_from_slice(&b1.edx.to_le_bytes());
        brand[16..20].copy_from_slice(&b2.eax.to_le_bytes());
        brand[20..24].copy_from_slice(&b2.ebx.to_le_bytes());
        brand[24..28].copy_from_slice(&b2.ecx.to_le_bytes());
        brand[28..32].copy_from_slice(&b2.edx.to_le_bytes());
        brand[32..36].copy_from_slice(&b3.eax.to_le_bytes());
        brand[36..40].copy_from_slice(&b3.ebx.to_le_bytes());
        brand[40..44].copy_from_slice(&b3.ecx.to_le_bytes());
        brand[44..48].copy_from_slice(&b3.edx.to_le_bytes());
    }

    unsafe {
        BSP_INFO = CpuInfo {
            vendor,
            brand,
            has_sse,
            has_sse2,
            has_avx,
            has_apic,
            apic_id,
        };
    }
}

pub fn get_bsp_info() -> &'static CpuInfo {
    unsafe { &BSP_INFO }
}
