//! Aegis64 / PurpleOS Native System Call Architecture & ABI (Pure Rust)
//!
//! Exclusively Native First-Class System Calls for High-Performance, Heavy-Load Workloads:
//!   - Category 1: Linear Framebuffer & 2D/3D GPU Hardware Acceleration
//!   - Category 2: Digital Audio Processing & Hardware DSP Streaming
//!   - Category 3: Low-Latency Zero-Copy IPC & High-Speed Ring Channels
//!   - Category 4: Multi-Core SMP Parallelism, Affinity & Execution Control
//!   - Category 5: High-Precision Timers, Clocks & Monotonic Performance Counters
//!   - Category 6: Raw Networking, Layer-2 Direct DMA & Packet Filtering
//!   - Category 7: Hardware Introspection, PurpleDetect & Device Telemetry
//!   - Category 8: Advanced Raw Block I/O & Storage Telemetry
//!   - Category 9: Real-Time Input Subsystem & Raw Event Streaming
//!   - Category 10: Process Management & Native Sandbox Security Capabilities
//!
//! Strict Aegis64 Rules:
//!   - 100% Pure Bare-Metal Rust (#![no_std]). Zero C/C++ runtime code.
//!   - Strictly Native PurpleOS ABI: Range 0x1000..0x10AF (Zero POSIX/Linux overlaps).
//!   - Safe memory validation: Strict separation of user and kernel virtual address spaces.

use crate::drivers::pit::{self, get_ticks, play_tone};
use crate::drivers::rtc::{self, get_time, WallTime};
use crate::drivers::video;
use crate::drivers::audio;
use crate::drivers::purple_detect;
use crate::drivers::block_dev;
use crate::drivers::keyboard;
use crate::drivers::mouse;
use crate::drivers::pci;
use crate::drivers::serial;
use crate::kernel::posix::{
    self, E2BIG, EAGAIN, EBADF, EBUSY, EFAULT, EINVAL, EIO, ENODEV, ENOMEM, ENOSYS, EOPNOTSUPP, EOVERFLOW, EPERM, ESRCH,
};
use crate::kernel::smp;
use crate::kernel::task::{self, SCHED, TaskPriority, TaskState, MAX_TASKS};
use crate::kernel::uaccess;
use crate::mm::pmm::{self, PAGE_SIZE};
use crate::mm::heap;
use crate::sync::Spinlock;

/* =========================================================================
 * 1. NATIVE SYSTEM CALL NUMBERS (PurpleOS Custom Native ABI: 0x1000 - 0x10AF)
 * ========================================================================= */
pub const SYS_PURPLE_BASE: u64 = 0x1000;
pub const SYS_PURPLE_MAX: u64  = 0x10AF;

// Category 1: Linear Framebuffer & 2D/3D GPU Acceleration (0x1000..0x101F)
pub const SYS_PURPLE_FB_GET_INFO: u64       = 0x1000;
pub const SYS_PURPLE_FB_ACQUIRE: u64        = 0x1001;
pub const SYS_PURPLE_FB_RELEASE: u64        = 0x1002;
pub const SYS_PURPLE_FB_FLIP: u64           = 0x1003;
pub const SYS_PURPLE_GPU_CLEAR: u64         = 0x1004;
pub const SYS_PURPLE_GPU_RECT_FILL: u64     = 0x1005;
pub const SYS_PURPLE_GPU_BLIT: u64          = 0x1006;
pub const SYS_PURPLE_GPU_DRAW_LINE: u64     = 0x1007;
pub const SYS_PURPLE_GPU_SUBMIT_3D: u64     = 0x1008;
pub const SYS_PURPLE_GPU_GET_CAPS: u64      = 0x1009;

// Category 2: Digital Audio Processing & Hardware DSP (0x1020..0x102F)
pub const SYS_PURPLE_AUDIO_GET_CAPS: u64    = 0x1020;
pub const SYS_PURPLE_AUDIO_STREAM_START: u64 = 0x1021;
pub const SYS_PURPLE_AUDIO_STREAM_STOP: u64 = 0x1022;
pub const SYS_PURPLE_AUDIO_SUBMIT_PCM: u64  = 0x1023;
pub const SYS_PURPLE_AUDIO_SET_VOLUME: u64  = 0x1024;
pub const SYS_PURPLE_AUDIO_GET_VOLUME: u64  = 0x1025;
pub const SYS_PURPLE_AUDIO_PLAY_SYNTH_TONE: u64 = 0x1026;
pub const SYS_PURPLE_AUDIO_GET_POSITION: u64 = 0x1027;

// Category 3: Low-Latency Zero-Copy IPC & Native Ring Channels (0x1030..0x103F)
pub const SYS_PURPLE_CHAN_CREATE: u64       = 0x1030;
pub const SYS_PURPLE_CHAN_SEND: u64         = 0x1031;
pub const SYS_PURPLE_CHAN_RECV: u64         = 0x1032;
pub const SYS_PURPLE_CHAN_CLOSE: u64        = 0x1033;
pub const SYS_PURPLE_CHAN_STAT: u64         = 0x1034;
pub const SYS_PURPLE_SHM_CREATE: u64        = 0x1035;
pub const SYS_PURPLE_SHM_MAP: u64           = 0x1036;
pub const SYS_PURPLE_SHM_UNMAP: u64         = 0x1037;

// Category 4: Multi-Core SMP Parallelism, Affinity & Execution Control (0x1040..0x104F)
pub const SYS_PURPLE_CPU_COUNT: u64         = 0x1040;
pub const SYS_PURPLE_CPU_GET_CURRENT: u64   = 0x1041;
pub const SYS_PURPLE_CPU_SET_AFFINITY: u64  = 0x1042;
pub const SYS_PURPLE_CPU_GET_AFFINITY: u64  = 0x1043;
pub const SYS_PURPLE_TASK_SET_PRIORITY: u64 = 0x1044;
pub const SYS_PURPLE_TASK_GET_PRIORITY: u64 = 0x1045;
pub const SYS_PURPLE_TASK_SET_SLICE: u64    = 0x1046;
pub const SYS_PURPLE_CORE_YIELD_TO: u64     = 0x1047;
pub const SYS_PURPLE_SPIN_PAUSE: u64        = 0x1048;

// Category 5: High-Precision Timers, Clocks & Monotonic Performance Counters (0x1050..0x105F)
pub const SYS_PURPLE_CLOCK_WALL: u64        = 0x1050;
pub const SYS_PURPLE_CLOCK_TICKS: u64       = 0x1051;
pub const SYS_PURPLE_CLOCK_NANOS: u64       = 0x1052;
pub const SYS_PURPLE_CLOCK_CYCLES: u64      = 0x1053;
pub const SYS_PURPLE_TIMER_CREATE: u64      = 0x1054;
pub const SYS_PURPLE_TIMER_CANCEL: u64      = 0x1055;
pub const SYS_PURPLE_MICRO_SLEEP: u64       = 0x1056;

// Category 6: Raw Networking, Layer-2 Direct DMA & Packet Filtering (0x1060..0x106F)
pub const SYS_PURPLE_NET_GET_DEV_INFO: u64  = 0x1060;
pub const SYS_PURPLE_NET_SEND_RAW: u64      = 0x1061;
pub const SYS_PURPLE_NET_RECV_RAW: u64      = 0x1062;
pub const SYS_PURPLE_NET_GET_STATS: u64     = 0x1063;
pub const SYS_PURPLE_NET_PING: u64          = 0x1064;
pub const SYS_PURPLE_NET_SET_CONFIG: u64    = 0x1065;

// Category 7: Hardware Introspection, PurpleDetect & Device Telemetry (0x1070..0x107F)
pub const SYS_PURPLE_DEVICE_COUNT: u64      = 0x1070;
pub const SYS_PURPLE_DEVICE_GET_INFO: u64   = 0x1071;
pub const SYS_PURPLE_MEM_METRICS: u64       = 0x1072;
pub const SYS_PURPLE_SYS_TELEMETRY: u64     = 0x1073;
pub const SYS_PURPLE_SEC_INTEGRITY_CHECK: u64 = 0x1074;
pub const SYS_PURPLE_PCI_QUERY: u64         = 0x1075;

// Category 8: Advanced Raw Block I/O & Storage Telemetry (0x1080..0x108F)
pub const SYS_PURPLE_STORAGE_COUNT: u64     = 0x1080;
pub const SYS_PURPLE_STORAGE_INFO: u64      = 0x1081;
pub const SYS_PURPLE_STORAGE_READ_BLOCKS: u64 = 0x1082;
pub const SYS_PURPLE_STORAGE_WRITE_BLOCKS: u64 = 0x1083;
pub const SYS_PURPLE_STORAGE_FLUSH: u64     = 0x1084;

// Category 9: Real-Time Input Subsystem & Raw Event Streaming (0x1090..0x109F)
pub const SYS_PURPLE_INPUT_MOUSE_STATE: u64 = 0x1090;
pub const SYS_PURPLE_INPUT_KEY_EVENT: u64   = 0x1091;
pub const SYS_PURPLE_INPUT_POLL_EVENTS: u64 = 0x1092;

// Category 10: Process Management & Native Sandbox Security (0x10A0..0x10AF)
pub const SYS_PURPLE_TASK_SPAWN: u64        = 0x10A0;
pub const SYS_PURPLE_TASK_KILL: u64         = 0x10A1;
pub const SYS_PURPLE_TASK_GET_INFO: u64     = 0x10A2;
pub const SYS_PURPLE_TASK_LIST: u64         = 0x10A3;
pub const SYS_PURPLE_SEC_CAP_DROP: u64      = 0x10A4;
pub const SYS_PURPLE_SEC_CAP_GET: u64       = 0x10A5;

/* =========================================================================
 * 2. NATIVE SANDBOX CAPABILITIES
 * ========================================================================= */
pub const PURPLE_CAP_RAW_IO: u64      = 1 << 0;
pub const PURPLE_CAP_RAW_NET: u64     = 1 << 1;
pub const PURPLE_CAP_RAW_STORAGE: u64 = 1 << 2;
pub const PURPLE_CAP_DIRECT_GPU: u64  = 1 << 3;
pub const PURPLE_CAP_DIRECT_AUDIO: u64 = 1 << 4;
pub const PURPLE_CAP_MULTI_CORE: u64  = 1 << 5;
pub const PURPLE_CAP_SYS_ADMIN: u64   = 1 << 6;
pub const PURPLE_CAP_ALL: u64         = !0;
const MAX_STORAGE_BLOCKS_PER_SYSCALL: u64 = 32;
const MAX_FRAMEBUFFER_RETRACE_POLLS: u32 = 100_000;
const MAX_TASK_SLICE_TICKS: u64 = 1_000;
const MAX_SPIN_PAUSE_CYCLES: u64 = 100_000;
const MAX_MICRO_SLEEP_US: u64 = 5_000_000;
const MAX_TONE_DURATION_MS: u64 = 5_000;
const AUDIO_RATE_MASK_44100: u32 = 1 << 0;
const AUDIO_DEPTH_MASK_16: u32 = 1 << 0;

/* =========================================================================
 * 3. USER-FACING C-COMPATIBLE DATA STRUCTURES
 * ========================================================================= */

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleFbInfo {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
    pub fb_phys: u64,
    pub flags: u32,
    pub _reserved: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
    pub _reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleGpuBlit {
    pub src_ptr: u64,
    pub dst_x: u32,
    pub dst_y: u32,
    pub width: u32,
    pub height: u32,
    pub src_pitch: u32,
    pub _reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleGpuCaps {
    pub vendor_id: u16,
    pub device_id: u16,
    pub flags: u32,
    pub max_texture_dim: u32,
    pub vram_bytes: u64,
    pub name: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleAudioCaps {
    pub sample_rates_mask: u32,
    pub bit_depths_mask: u32,
    pub channels: u32,
    pub current_volume: u32,
    pub is_hardware_ready: u32,
    pub codecs_count: u32,
    pub dma_ring_bytes: u32,
    pub _reserved: u32,
}

pub const PURPLE_CHAN_MAX_MSG_LEN: usize = 256;
pub const PURPLE_CHAN_QUEUE_CAP: usize = 16;
pub const PURPLE_MAX_CHANNELS: usize = 32;
pub const PURPLE_MAX_SHM_REGIONS: usize = 16;
pub const PURPLE_SHM_MAX_PAGES: usize = 256;
pub const ETH_MIN_FRAME_LEN: usize = 14;
pub const ETH_MAX_FRAME_LEN: usize = 1518;
pub const VGA_INPUT_STATUS_1: u16 = 0x03DA;
pub const KERNEL_VERSION_MAJOR: u16 = 0;
pub const KERNEL_VERSION_MINOR: u16 = 5;
pub const KERNEL_VERSION_PATCH: u16 = 0;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleChanStat {
    pub id: u32,
    pub messages_queued: u32,
    pub max_capacity: u32,
    pub readers_count: u32,
    pub writers_count: u32,
    pub _reserved: u32,
    pub total_messages_sent: u64,
    pub total_messages_recv: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleWallClock {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub _pad: u8,
    pub epoch_seconds: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleNetDevInfo {
    pub mac: [u8; 6],
    pub link_speed_mbps: u16,
    pub ip: [u8; 4],
    pub subnet: [u8; 4],
    pub gateway: [u8; 4],
    pub mtu: u16,
    pub link_up: u8,
    pub driver_type: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleNetStats {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_dropped: u64,
    pub rx_dropped: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleDeviceDesc {
    pub id: u32,
    pub category: u32,
    pub state: u32,
    pub irq: u8,
    pub bus_str: [u8; 8],
    pub name_str: [u8; 16],
    pub vendor_str: [u8; 24],
    pub model_str: [u8; 32],
    pub address: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleMemMetrics {
    pub total_frames: u64,
    pub free_frames: u64,
    pub allocated_frames: u64,
    pub reserved_frames: u64,
    pub frame_size: u64,
    pub heap_allocated_bytes: u64,
    pub heap_total_bytes: u64,
    pub _reserved: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleSysTelemetry {
    pub uptime_ticks: u64,
    pub online_cpus: u32,
    pub active_tasks: u32,
    pub total_context_switches: u64,
    pub kernel_version_major: u16,
    pub kernel_version_minor: u16,
    pub kernel_version_patch: u16,
    pub _reserved: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleIntegrityReport {
    pub verified_components: u32,
    pub total_components: u32,
    pub tampered_components: u32,
    pub is_trusted: u32,
    pub timestamp_epoch: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurplePciDeviceInfo {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub irq_line: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u8,
    pub subclass_id: u8,
    pub prog_if: u8,
    pub _pad: u8,
    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar3: u32,
    pub bar4: u32,
    pub bar5: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleStorageDevInfo {
    pub dev_index: u32,
    pub block_size: u32,
    pub total_blocks: u64,
    pub category: u32,
    pub name: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleMouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u32,
    pub scroll_delta: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleInputEvent {
    pub event_type: u16,
    pub keycode: u8,
    pub ascii: u8,
    pub modifiers: u16,
    pub data1: i32,
    pub data2: i32,
    pub timestamp_ticks: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PurpleTaskInfo {
    pub id: u32,
    pub state: u32,
    pub priority: u32,
    pub last_cpu: u32,
    pub time_slice_ticks: u32,
    pub ticks_remaining: u32,
    pub cpu_affinity: u64,
    pub is_user: u32,
    pub name: [u8; 16],
    pub cap_mask: u64,
}

/* =========================================================================
 * 4. INTERNAL KERNEL MANAGERS FOR NATIVE SUBSYSTEMS
 * ========================================================================= */

// Channel IPC
#[derive(Copy, Clone)]
struct PurpleMessage {
    len: usize,
    data: [u8; PURPLE_CHAN_MAX_MSG_LEN],
    sender_tid: u32,
}

impl PurpleMessage {
    const fn empty() -> Self {
        Self {
            len: 0,
            data: [0; PURPLE_CHAN_MAX_MSG_LEN],
            sender_tid: 0,
        }
    }
}

struct PurpleChannel {
    active: bool,
    queue: [PurpleMessage; PURPLE_CHAN_QUEUE_CAP],
    head: usize,
    tail: usize,
    count: usize,
    readers_count: u32,
    writers_count: u32,
    total_sent: u64,
    total_recv: u64,
}

impl PurpleChannel {
    const fn empty() -> Self {
        Self {
            active: false,
            queue: [PurpleMessage::empty(); PURPLE_CHAN_QUEUE_CAP],
            head: 0,
            tail: 0,
            count: 0,
            readers_count: 0,
            writers_count: 0,
            total_sent: 0,
            total_recv: 0,
        }
    }
}

static CHANNELS: Spinlock<[PurpleChannel; PURPLE_MAX_CHANNELS]> = Spinlock::new([
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
    PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(), PurpleChannel::empty(),
]);

// Network Telemetry
static NET_STATS: Spinlock<PurpleNetStats> = Spinlock::new(PurpleNetStats {
    tx_packets: 0,
    rx_packets: 0,
    tx_bytes: 0,
    rx_bytes: 0,
    tx_dropped: 0,
    rx_dropped: 0,
});

// Framebuffer direct rendering lock
static FB_ACQUIRED_BY: Spinlock<Option<u32>> = Spinlock::new(None);

/* =========================================================================
 * 5. SAFE MEMORY TRANSFER UTILITIES (USER / KERNEL DUAL MODE)
 * ========================================================================= */

fn is_caller_user() -> bool {
    let cpu_id = task::get_current_cpu();
    let s = SCHED.lock();
    s.current[cpu_id]
        .map(|idx| s.tasks[idx].is_user)
        .unwrap_or(false)
}

fn current_task_id() -> u32 {
    task::get_current_task_id()
}

fn current_task_has_cap(cap: u64) -> bool {
    let cpu_id = task::get_current_cpu();
    let s = SCHED.lock();
    let idx = match s.current[cpu_id] {
        Some(idx) => idx,
        None => return false,
    };
    let current = &s.tasks[idx];
    (current.cap_mask & cap) == cap
}

pub fn safe_copy_to_user(kernel_src: &[u8], user_dst: u64) -> Result<(), i64> {
    if kernel_src.is_empty() {
        return Ok(());
    }
    if user_dst == 0 {
        return Err(-EFAULT);
    }
    if is_caller_user() {
        uaccess::copy_to_user(kernel_src, user_dst)
    } else {
        unsafe {
            core::ptr::copy_nonoverlapping(kernel_src.as_ptr(), user_dst as *mut u8, kernel_src.len());
        }
        Ok(())
    }
}

pub fn safe_copy_from_user(user_src: u64, kernel_dst: &mut [u8]) -> Result<(), i64> {
    if kernel_dst.is_empty() {
        return Ok(());
    }
    if user_src == 0 {
        return Err(-EFAULT);
    }
    if is_caller_user() {
        uaccess::copy_from_user(user_src, kernel_dst)
    } else {
        unsafe {
            core::ptr::copy_nonoverlapping(user_src as *const u8, kernel_dst.as_mut_ptr(), kernel_dst.len());
        }
        Ok(())
    }
}

/* =========================================================================
 * 6. BACKWARD COMPATIBLE EXPORTS
 * ========================================================================= */
pub fn get_wall_clock() -> WallTime {
    get_time()
}

pub fn get_system_ticks() -> u64 {
    get_ticks()
}

/* =========================================================================
 * 7. DISPATCH NATIVE SYSTEM CALLS
 * ========================================================================= */

#[inline]
pub fn dispatch_native_syscall(
    sys_no: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    _a6: u64,
) -> i64 {
    match sys_no {
        /* =====================================================================
         * Category 1: Linear Framebuffer & 2D/3D GPU Acceleration
         * ===================================================================== */
        SYS_PURPLE_FB_GET_INFO => {
            if let Some((w, h, pitch, bpp, fb_phys)) = video::get_fb_info() {
                let info = PurpleFbInfo {
                    width: w as u32,
                    height: h as u32,
                    pitch: pitch as u32,
                    bpp: bpp as u32,
                    fb_phys,
                    flags: 0x01, // LFB active
                    _reserved: [0; 2],
                };
                let slice = unsafe {
                    core::slice::from_raw_parts(&info as *const PurpleFbInfo as *const u8, core::mem::size_of::<PurpleFbInfo>())
                };
                match safe_copy_to_user(slice, a1) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -ENODEV
            }
        }

        SYS_PURPLE_FB_ACQUIRE => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            let tid = current_task_id();
            let mut acq = FB_ACQUIRED_BY.lock();
            match *acq {
                Some(owner) if owner != tid => -EBUSY,
                _ => {
                    *acq = Some(tid);
                    0
                }
            }
        }

        SYS_PURPLE_FB_RELEASE => {
            let tid = current_task_id();
            let mut acq = FB_ACQUIRED_BY.lock();
            if *acq == Some(tid) {
                *acq = None;
                0
            } else {
                -EPERM
            }
        }

        SYS_PURPLE_FB_FLIP => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            if !crate::drivers::gpu::GPU.lock().double_buffered {
                return -EOPNOTSUPP;
            }
            if a1 != 0 {
                let mut timeout = MAX_FRAMEBUFFER_RETRACE_POLLS;
                while timeout > 0 {
                    let status = unsafe { crate::arch::x86_64::io::inb(VGA_INPUT_STATUS_1) };
                    if (status & 0x08) != 0 {
                        break;
                    }
                    timeout -= 1;
                    core::hint::spin_loop();
                }
                if timeout == 0 {
                    return -EAGAIN;
                }
            }
            crate::drivers::gpu::GPU.lock().flip_page();
            0
        }

        SYS_PURPLE_GPU_CLEAR => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            if !video::is_available() {
                return -ENODEV;
            }
            video::clear_screen(a1 as u32);
            0
        }

        SYS_PURPLE_GPU_RECT_FILL => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            if !video::is_available() {
                return -ENODEV;
            }
            let mut rect = PurpleGpuRect::default();
            let slice = unsafe {
                core::slice::from_raw_parts_mut(&mut rect as *mut PurpleGpuRect as *mut u8, core::mem::size_of::<PurpleGpuRect>())
            };
            if let Err(e) = safe_copy_from_user(a1, slice) {
                return e;
            }
            video::draw_rect(rect.x as usize, rect.y as usize, rect.width as usize, rect.height as usize, rect.color);
            0
        }

        SYS_PURPLE_GPU_BLIT => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            if !video::is_available() {
                return -ENODEV;
            }
            let mut blit = PurpleGpuBlit::default();
            let slice = unsafe {
                core::slice::from_raw_parts_mut(&mut blit as *mut PurpleGpuBlit as *mut u8, core::mem::size_of::<PurpleGpuBlit>())
            };
            if let Err(e) = safe_copy_from_user(a1, slice) {
                return e;
            }
            let width = blit.width as usize;
            let height = blit.height as usize;
            let pitch = blit.src_pitch as usize;
            let row_bytes = match width.checked_mul(core::mem::size_of::<u32>()) {
                Some(bytes) => bytes,
                None => return -EOVERFLOW,
            };
            if width == 0 || height == 0 || pitch < row_bytes {
                return -EINVAL;
            }
            let source_span = match (height - 1).checked_mul(pitch).and_then(|offset| offset.checked_add(row_bytes)) {
                Some(bytes) => bytes,
                None => return -EOVERFLOW,
            };
            if source_span > isize::MAX as usize {
                return -E2BIG;
            }
            let mut copied = 0usize;
            let mut row_buffer = [0u32; 1024];
            for row in 0..height {
                let source_row = match (row as u64)
                    .checked_mul(pitch as u64)
                    .and_then(|offset| blit.src_ptr.checked_add(offset))
                {
                    Some(address) => address,
                    None => return -EOVERFLOW,
                };
                let mut column = 0usize;
                while column < width {
                    let count = (width - column).min(row_buffer.len());
                    let byte_count = count * core::mem::size_of::<u32>();
                    let source = match source_row.checked_add((column * 4) as u64) {
                        Some(address) => address,
                        None => return -EOVERFLOW,
                    };
                    let bytes = unsafe {
                        core::slice::from_raw_parts_mut(row_buffer.as_mut_ptr() as *mut u8, byte_count)
                    };
                    if let Err(e) = safe_copy_from_user(source, bytes) {
                        return e;
                    }
                    let draw_x = (blit.dst_x as usize).saturating_add(column);
                    let draw_y = (blit.dst_y as usize).saturating_add(row);
                    let drawn = video::blit_rect(&row_buffer[..count], draw_x, draw_y, count, 1, count);
                    copied = copied.saturating_add(drawn);
                    column += count;
                }
            }
            copied as i64
        }

        SYS_PURPLE_GPU_DRAW_LINE => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_GPU) {
                return -EPERM;
            }
            if !video::is_available() {
                return -ENODEV;
            }
            let x0 = a1 as usize;
            let y0 = a2 as usize;
            let x1 = a3 as usize;
            let y1 = a4 as usize;
            if x0 >= video::get_width() || x1 >= video::get_width()
                || y0 >= video::get_height() || y1 >= video::get_height()
            {
                return -EINVAL;
            }
            let color = a5 as u32;
            video::draw_line(x0, y0, x1, y1, color);
            0
        }

        SYS_PURPLE_GPU_SUBMIT_3D => -EOPNOTSUPP,

        SYS_PURPLE_GPU_GET_CAPS => {
            let gpu = crate::drivers::gpu::GPU.lock();
            let caps = PurpleGpuCaps {
                vendor_id: gpu.vendor_id,
                device_id: gpu.device_id,
                flags: if gpu.is_ready { 0x01 } else { 0 }, // linear framebuffer only
                max_texture_dim: 0,
                vram_bytes: 0, // Total VRAM is not exposed by this driver.
                name: [0; 32],
            };
            drop(gpu);
            let mut caps = caps;
            let dev_name = b"PurpleOS framebuffer";
            caps.name[..dev_name.len()].copy_from_slice(dev_name);
            let slice = unsafe {
                core::slice::from_raw_parts(&caps as *const PurpleGpuCaps as *const u8, core::mem::size_of::<PurpleGpuCaps>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        /* =====================================================================
         * Category 2: Digital Audio Processing & Hardware DSP
         * ===================================================================== */
        SYS_PURPLE_AUDIO_GET_CAPS => {
            let hda = audio::HDA.lock();
            let num_codecs = hda.num_codecs as u32;
            let is_ready = hda.is_ready;
            drop(hda);

            let caps = PurpleAudioCaps {
                sample_rates_mask: if is_ready { AUDIO_RATE_MASK_44100 } else { 0 },
                bit_depths_mask: if is_ready { AUDIO_DEPTH_MASK_16 } else { 0 },
                channels: if is_ready { 2 } else { 0 },
                current_volume: audio::get_volume() as u32,
                is_hardware_ready: if is_ready && num_codecs > 0 { 1 } else { 0 },
                codecs_count: num_codecs,
                dma_ring_bytes: crate::drivers::audio::HDA_DMA_BUFFER_SIZE as u32,
                _reserved: 0,
            };
            let slice = unsafe {
                core::slice::from_raw_parts(&caps as *const PurpleAudioCaps as *const u8, core::mem::size_of::<PurpleAudioCaps>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_AUDIO_STREAM_START | SYS_PURPLE_AUDIO_STREAM_STOP => -EOPNOTSUPP,

        SYS_PURPLE_AUDIO_SUBMIT_PCM => -EOPNOTSUPP,

        SYS_PURPLE_AUDIO_SET_VOLUME => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_AUDIO) {
                return -EPERM;
            }
            let vol = (a1 as u32).min(100) as u8;
            audio::set_volume(vol);
            0
        }

        SYS_PURPLE_AUDIO_GET_VOLUME => {
            audio::get_volume() as i64
        }

        SYS_PURPLE_AUDIO_PLAY_SYNTH_TONE => {
            if !current_task_has_cap(PURPLE_CAP_DIRECT_AUDIO) {
                return -EPERM;
            }
            if a1 == 0 || a1 > crate::drivers::pit::PIT_MAX_TONE_FREQUENCY_HZ as u64 || a2 > MAX_TONE_DURATION_MS {
                return -EINVAL;
            }
            play_tone(a1 as u32, a2 as u32);
            0
        }

        SYS_PURPLE_AUDIO_GET_POSITION => {
            audio::get_stream_position() as i64
        }

        /* =====================================================================
         * Category 3: Low-Latency Zero-Copy IPC & Native Ring Channels
         * ===================================================================== */
        SYS_PURPLE_CHAN_CREATE => {
            let mut channels = CHANNELS.lock();
            for (i, ch) in channels.iter_mut().enumerate() {
                if !ch.active {
                    ch.active = true;
                    ch.head = 0;
                    ch.tail = 0;
                    ch.count = 0;
                    ch.readers_count = 1;
                    ch.writers_count = 1;
                    ch.total_sent = 0;
                    ch.total_recv = 0;
                    return (i as i64) + 1; // 1-based channel descriptor
                }
            }
            -ENOMEM
        }

        SYS_PURPLE_CHAN_SEND => {
            let chan_id = a1 as usize;
            if chan_id == 0 || chan_id > PURPLE_MAX_CHANNELS {
                return -EINVAL;
            }
            let len = a3 as usize;
            if len == 0 || len > PURPLE_CHAN_MAX_MSG_LEN {
                return -EINVAL;
            }
            let mut msg_buf = [0u8; PURPLE_CHAN_MAX_MSG_LEN];
            if let Err(e) = safe_copy_from_user(a2, &mut msg_buf[..len]) {
                return e;
            }

            let mut channels = CHANNELS.lock();
            let ch = &mut channels[chan_id - 1];
            if !ch.active {
                return -EBADF;
            }
            if ch.count >= PURPLE_CHAN_QUEUE_CAP {
                return -EAGAIN;
            }

            let slot = &mut ch.queue[ch.head];
            slot.len = len;
            slot.data[..len].copy_from_slice(&msg_buf[..len]);
            slot.sender_tid = current_task_id();
            ch.head = (ch.head + 1) % PURPLE_CHAN_QUEUE_CAP;
            ch.count += 1;
            ch.total_sent = ch.total_sent.saturating_add(1);
            len as i64
        }

        SYS_PURPLE_CHAN_RECV => {
            let chan_id = a1 as usize;
            if chan_id == 0 || chan_id > PURPLE_MAX_CHANNELS {
                return -EINVAL;
            }
            let max_len = a3 as usize;
            if max_len == 0 {
                return -EINVAL;
            }

            let mut channels = CHANNELS.lock();
            let ch = &mut channels[chan_id - 1];
            if !ch.active {
                return -EBADF;
            }
            if ch.count == 0 {
                return -EAGAIN;
            }

            let slot = &ch.queue[ch.tail];
            let copy_len = slot.len.min(max_len);
            let mut temp = [0u8; PURPLE_CHAN_MAX_MSG_LEN];
            temp[..copy_len].copy_from_slice(&slot.data[..copy_len]);

            ch.tail = (ch.tail + 1) % PURPLE_CHAN_QUEUE_CAP;
            ch.count -= 1;
            ch.total_recv = ch.total_recv.saturating_add(1);
            drop(channels);

            if let Err(e) = safe_copy_to_user(&temp[..copy_len], a2) {
                return e;
            }
            copy_len as i64
        }

        SYS_PURPLE_CHAN_CLOSE => {
            let chan_id = a1 as usize;
            if chan_id == 0 || chan_id > PURPLE_MAX_CHANNELS {
                return -EINVAL;
            }
            let mut channels = CHANNELS.lock();
            let ch = &mut channels[chan_id - 1];
            if !ch.active {
                return -EBADF;
            }
            ch.active = false;
            ch.count = 0;
            0
        }

        SYS_PURPLE_CHAN_STAT => {
            let chan_id = a1 as usize;
            if chan_id == 0 || chan_id > PURPLE_MAX_CHANNELS {
                return -EINVAL;
            }
            let channels = CHANNELS.lock();
            let ch = &channels[chan_id - 1];
            if !ch.active {
                return -EBADF;
            }
            let stat = PurpleChanStat {
                id: chan_id as u32,
                messages_queued: ch.count as u32,
                max_capacity: PURPLE_CHAN_QUEUE_CAP as u32,
                readers_count: ch.readers_count,
                writers_count: ch.writers_count,
                _reserved: 0,
                total_messages_sent: ch.total_sent,
                total_messages_recv: ch.total_recv,
            };
            drop(channels);

            let slice = unsafe {
                core::slice::from_raw_parts(&stat as *const PurpleChanStat as *const u8, core::mem::size_of::<PurpleChanStat>())
            };
            match safe_copy_to_user(slice, a2) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_SHM_CREATE | SYS_PURPLE_SHM_MAP | SYS_PURPLE_SHM_UNMAP => -EOPNOTSUPP,

        /* =====================================================================
         * Category 4: Multi-Core SMP Parallelism, Affinity & Execution Control
         * ===================================================================== */
        SYS_PURPLE_CPU_COUNT => {
            smp::get_online_count() as i64
        }

        SYS_PURPLE_CPU_GET_CURRENT => {
            smp::get_cpu_id() as i64
        }

        SYS_PURPLE_CPU_SET_AFFINITY => {
            if !current_task_has_cap(PURPLE_CAP_MULTI_CORE) {
                return -EPERM;
            }
            if a1 > u32::MAX as u64 {
                return -EINVAL;
            }
            let target_tid = a1 as u32;
            let mask = a2;
            let online = smp::get_online_count() as usize;
            let online_mask = if online >= u64::BITS as usize { u64::MAX } else { (1u64 << online) - 1 };
            if mask == 0 || mask & !online_mask != 0 {
                return -EINVAL;
            }
            let is_admin = current_task_has_cap(PURPLE_CAP_SYS_ADMIN);

            let mut s = SCHED.lock();
            let cpu_id = task::get_current_cpu();
            let cur_tid = s.current[cpu_id].map(|idx| s.tasks[idx].id).unwrap_or(0);
            let tid = if target_tid == 0 { cur_tid } else { target_tid };
            if tid != cur_tid && !is_admin {
                return -EPERM;
            }
            if tid == cur_tid && mask & (1u64 << cpu_id) == 0 {
                return -EINVAL;
            }
            for t in s.tasks.iter_mut() {
                if t.id == tid && t.state != TaskState::Unused {
                    t.cpu_affinity = mask;
                    return 0;
                }
            }
            -ESRCH
        }

        SYS_PURPLE_CPU_GET_AFFINITY => {
            if a1 > u32::MAX as u64 { return -EINVAL; }
            let target_tid = a1 as u32;
            let s = SCHED.lock();
            let cpu_id = task::get_current_cpu();
            let cur_tid = s.current[cpu_id].map(|idx| s.tasks[idx].id).unwrap_or(0);
            let tid = if target_tid == 0 { cur_tid } else { target_tid };

            for t in s.tasks.iter() {
                if t.id == tid && t.state != TaskState::Unused {
                    return t.cpu_affinity as i64;
                }
            }
            -ESRCH
        }

        SYS_PURPLE_TASK_SET_PRIORITY => {
            if a1 > u32::MAX as u64 || a2 > 3 { return -EINVAL; }
            let target_tid = a1 as u32;
            let prio_val = a2 as u32;
            let prio = TaskPriority::from_u32(prio_val);
            let is_admin = current_task_has_cap(PURPLE_CAP_SYS_ADMIN);
            if prio == TaskPriority::High && !is_admin {
                return -EPERM;
            }
            let mut s = SCHED.lock();
            let cpu_id = task::get_current_cpu();
            let cur_tid = s.current[cpu_id].map(|idx| s.tasks[idx].id).unwrap_or(0);
            let tid = if target_tid == 0 { cur_tid } else { target_tid };
            if tid != cur_tid && !is_admin {
                return -EPERM;
            }
            for t in s.tasks.iter_mut() {
                if t.id == tid && t.state != TaskState::Unused {
                    t.priority = prio;
                    return 0;
                }
            }
            -ESRCH
        }

        SYS_PURPLE_TASK_GET_PRIORITY => {
            if a1 > u32::MAX as u64 { return -EINVAL; }
            let target_tid = a1 as u32;
            let s = SCHED.lock();
            let cpu_id = task::get_current_cpu();
            let cur_tid = s.current[cpu_id].map(|idx| s.tasks[idx].id).unwrap_or(0);
            let tid = if target_tid == 0 { cur_tid } else { target_tid };

            for t in s.tasks.iter() {
                if t.id == tid && t.state != TaskState::Unused {
                    return t.priority.as_u32() as i64;
                }
            }
            -ESRCH
        }

        SYS_PURPLE_TASK_SET_SLICE => {
            if !current_task_has_cap(PURPLE_CAP_SYS_ADMIN) {
                return -EPERM;
            }
            if a1 > u32::MAX as u64 || a2 == 0 || a2 > MAX_TASK_SLICE_TICKS { return -EINVAL; }
            let target_tid = a1 as u32;
            let slice_ticks = a2 as u32;
            let mut s = SCHED.lock();
            let cpu_id = task::get_current_cpu();
            let cur_tid = s.current[cpu_id].map(|idx| s.tasks[idx].id).unwrap_or(0);
            let tid = if target_tid == 0 { cur_tid } else { target_tid };

            for t in s.tasks.iter_mut() {
                if t.id == tid && t.state != TaskState::Unused {
                    t.time_slice_ticks = slice_ticks;
                    return 0;
                }
            }
            -ESRCH
        }

        SYS_PURPLE_CORE_YIELD_TO => -EOPNOTSUPP,

        SYS_PURPLE_SPIN_PAUSE => {
            let cycles = a1.min(MAX_SPIN_PAUSE_CYCLES) as u32;
            for _ in 0..cycles {
                core::hint::spin_loop();
            }
            0
        }

        /* =====================================================================
         * Category 5: High-Precision Timers, Clocks & Monotonic Performance
         * ===================================================================== */
        SYS_PURPLE_CLOCK_WALL => {
            let wt = rtc::get_time();
            let epoch_sec = posix::rtc_to_unix_epoch(&wt) as u64;
            let wc = PurpleWallClock {
                year: wt.year,
                month: wt.month,
                day: wt.day,
                hour: wt.hour,
                minute: wt.minute,
                second: wt.second,
                _pad: 0,
                epoch_seconds: epoch_sec,
            };
            let slice = unsafe {
                core::slice::from_raw_parts(&wc as *const PurpleWallClock as *const u8, core::mem::size_of::<PurpleWallClock>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_CLOCK_TICKS => {
            pit::get_ticks() as i64
        }

        SYS_PURPLE_CLOCK_NANOS => {
            let ticks = pit::get_ticks();
            match i64::try_from(pit::ticks_to_nanos(ticks)).ok() {
                Some(ns) => ns,
                None => i64::MAX,
            }
        }

        SYS_PURPLE_CLOCK_CYCLES => {
            let tsc = unsafe { core::arch::x86_64::_rdtsc() };
            tsc as i64
        }

        SYS_PURPLE_TIMER_CREATE | SYS_PURPLE_TIMER_CANCEL => -EOPNOTSUPP,

        SYS_PURPLE_MICRO_SLEEP => {
            let us = a1.min(MAX_MICRO_SLEEP_US); // Maximum blocking delay
            crate::drivers::pit::delay_us_long(us);
            0
        }

        /* =====================================================================
         * Category 6: Raw Networking, Layer-2 Direct DMA & Packet Filtering
         * ===================================================================== */
        SYS_PURPLE_NET_GET_DEV_INFO => {
            let mut dev_info = PurpleNetDevInfo::default();
            if let Some(mac) = crate::net::get_my_mac() {
                dev_info.mac = mac;
            }
            dev_info.ip = crate::net::get_ip();
            dev_info.gateway = crate::net::get_gateway();
            dev_info.subnet = crate::net::get_subnet();
            dev_info.driver_type = if crate::drivers::e1000::is_ready() {
                dev_info.mtu = crate::net::IPV4_DEFAULT_MTU as u16;
                1
            } else if crate::drivers::rtl8139::is_ready() {
                dev_info.mtu = crate::net::IPV4_DEFAULT_MTU as u16;
                2
            } else {
                0
            };

            let slice = unsafe {
                core::slice::from_raw_parts(&dev_info as *const PurpleNetDevInfo as *const u8, core::mem::size_of::<PurpleNetDevInfo>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_NET_SEND_RAW => {
            if !current_task_has_cap(PURPLE_CAP_RAW_NET) {
                return -EPERM;
            }
            if a2 < ETH_MIN_FRAME_LEN as u64 || a2 > ETH_MAX_FRAME_LEN as u64 {
                return -EINVAL;
            }
            let len = a2 as usize;
            if len < ETH_MIN_FRAME_LEN || len > ETH_MAX_FRAME_LEN {
                return -EINVAL;
            }
            let mut frame = [0u8; ETH_MAX_FRAME_LEN];
            if let Err(e) = safe_copy_from_user(a1, &mut frame[..len]) {
                return e;
            }
            match crate::net::send_raw(&frame[..len]) {
                Ok(()) => {
                    let mut st = NET_STATS.lock();
                    st.tx_packets = st.tx_packets.saturating_add(1);
                    st.tx_bytes = st.tx_bytes.saturating_add(len as u64);
                    len as i64
                }
                Err(_) => {
                    let mut st = NET_STATS.lock();
                    st.tx_dropped = st.tx_dropped.saturating_add(1);
                    -EIO
                }
            }
        }

        SYS_PURPLE_NET_RECV_RAW => {
            if !current_task_has_cap(PURPLE_CAP_RAW_NET) {
                return -EPERM;
            }
            if a2 == 0 || a2 > ETH_MAX_FRAME_LEN as u64 {
                return -EINVAL;
            }
            let max_len = a2 as usize;
            let mut frame = [0u8; ETH_MAX_FRAME_LEN];
            let read_bytes = if crate::drivers::e1000::is_ready() {
                crate::drivers::e1000::recv(&mut frame[..max_len])
            } else if crate::drivers::rtl8139::is_ready() {
                crate::drivers::rtl8139::recv(&mut frame[..max_len])
            } else {
                0
            };

            if read_bytes == 0 {
                return -EAGAIN;
            }
            if let Err(e) = safe_copy_to_user(&frame[..read_bytes], a1) {
                return e;
            }
            let mut st = NET_STATS.lock();
            st.rx_packets = st.rx_packets.saturating_add(1);
            st.rx_bytes = st.rx_bytes.saturating_add(read_bytes as u64);
            read_bytes as i64
        }

        SYS_PURPLE_NET_GET_STATS => {
            let stats = *NET_STATS.lock();
            let slice = unsafe {
                core::slice::from_raw_parts(&stats as *const PurpleNetStats as *const u8, core::mem::size_of::<PurpleNetStats>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_NET_PING => {
            if [a1, a2, a3, a4].iter().any(|octet| *octet > u8::MAX as u64) {
                return -EINVAL;
            }
            let ip0 = a1 as u8;
            let ip1 = a2 as u8;
            let ip2 = a3 as u8;
            let ip3 = a4 as u8;
            match crate::net::ping([ip0, ip1, ip2, ip3]) {
                Ok(rtt) => rtt as i64,
                Err(_) => -EIO,
            }
        }

        SYS_PURPLE_NET_SET_CONFIG => {
            if !current_task_has_cap(PURPLE_CAP_SYS_ADMIN) {
                return -EPERM;
            }
            if [a1, a2, a3].iter().any(|value| *value > u32::MAX as u64) {
                return -EINVAL;
            }
            let ip_u32 = a1 as u32;
            let gw_u32 = a2 as u32;
            let sub_u32 = a3 as u32;
            let ip = [(ip_u32 >> 24) as u8, (ip_u32 >> 16) as u8, (ip_u32 >> 8) as u8, ip_u32 as u8];
            let gw = [(gw_u32 >> 24) as u8, (gw_u32 >> 16) as u8, (gw_u32 >> 8) as u8, gw_u32 as u8];
            let sub = [(sub_u32 >> 24) as u8, (sub_u32 >> 16) as u8, (sub_u32 >> 8) as u8, sub_u32 as u8];
            if crate::net::set_config(ip, gw, sub).is_ok() { 0 } else { -EINVAL }
        }

        /* =====================================================================
         * Category 7: Hardware Introspection, PurpleDetect & Device Telemetry
         * ===================================================================== */
        SYS_PURPLE_DEVICE_COUNT => {
            purple_detect::get_device_count() as i64
        }

        SYS_PURPLE_DEVICE_GET_INFO => {
            let idx = a1 as usize;
            if let Some(dev) = purple_detect::get_device(idx) {
                let desc = PurpleDeviceDesc {
                    id: dev.id,
                    category: dev.category as u32,
                    state: dev.state as u32,
                    irq: dev.irq,
                    bus_str: dev.bus,
                    name_str: dev.name,
                    vendor_str: dev.vendor,
                    model_str: dev.model,
                    address: dev.address,
                };
                let slice = unsafe {
                    core::slice::from_raw_parts(&desc as *const PurpleDeviceDesc as *const u8, core::mem::size_of::<PurpleDeviceDesc>())
                };
                match safe_copy_to_user(slice, a2) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -ENODEV
            }
        }

        SYS_PURPLE_MEM_METRICS => {
            let (heap_alloc, heap_tot) = heap::get_status();
            let metrics = PurpleMemMetrics {
                total_frames: pmm::total_frames_count() as u64,
                free_frames: pmm::free_frames_count() as u64,
                allocated_frames: pmm::allocated_frames_count() as u64,
                reserved_frames: pmm::reserved_frames_count() as u64,
                frame_size: PAGE_SIZE as u64,
                heap_allocated_bytes: heap_alloc as u64,
                heap_total_bytes: heap_tot as u64,
                _reserved: 0,
            };
            let slice = unsafe {
                core::slice::from_raw_parts(&metrics as *const PurpleMemMetrics as *const u8, core::mem::size_of::<PurpleMemMetrics>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_SYS_TELEMETRY => {
            let s = SCHED.lock();
            let mut active = 0u32;
            for t in s.tasks.iter() {
                if t.state != TaskState::Unused {
                    active += 1;
                }
            }
            let telem = PurpleSysTelemetry {
                uptime_ticks: pit::get_ticks(),
                online_cpus: smp::get_online_count(),
                active_tasks: active,
                total_context_switches: s.ticks,
                kernel_version_major: KERNEL_VERSION_MAJOR,
                kernel_version_minor: KERNEL_VERSION_MINOR,
                kernel_version_patch: KERNEL_VERSION_PATCH,
                _reserved: 0,
            };
            drop(s);

            let slice = unsafe {
                core::slice::from_raw_parts(&telem as *const PurpleSysTelemetry as *const u8, core::mem::size_of::<PurpleSysTelemetry>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_SEC_INTEGRITY_CHECK => {
            let (verified, total, unverified, missing) = crate::security::aegis_detect::verify_storage();
            let wt = rtc::get_time();
            let rep = PurpleIntegrityReport {
                verified_components: verified as u32,
                total_components: total as u32,
                tampered_components: unverified as u32,
                is_trusted: if total > 0 && verified == total && unverified == 0 && missing == 0 { 1 } else { 0 },
                timestamp_epoch: posix::rtc_to_unix_epoch(&wt) as u64,
            };
            let slice = unsafe {
                core::slice::from_raw_parts(&rep as *const PurpleIntegrityReport as *const u8, core::mem::size_of::<PurpleIntegrityReport>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_PCI_QUERY => {
            let target_idx = a1 as usize;
            let mut found_dev = None;
            let mut cur_idx = 0usize;
            pci::for_each_device(|dev| {
                if cur_idx == target_idx {
                    found_dev = Some(*dev);
                }
                cur_idx += 1;
            });

            if let Some(dev) = found_dev {
                let info = PurplePciDeviceInfo {
                    bus: dev.bus,
                    slot: dev.slot,
                    func: dev.func,
                    irq_line: dev.irq_line,
                    vendor_id: dev.vendor_id,
                    device_id: dev.device_id,
                    class_id: dev.class_id,
                    subclass_id: dev.subclass_id,
                    prog_if: dev.prog_if,
                    _pad: 0,
                    bar0: dev.bar0,
                    bar1: dev.bar1,
                    bar2: dev.bar2,
                    bar3: dev.bar3,
                    bar4: dev.bar4,
                    bar5: dev.bar5,
                };
                let slice = unsafe {
                    core::slice::from_raw_parts(&info as *const PurplePciDeviceInfo as *const u8, core::mem::size_of::<PurplePciDeviceInfo>())
                };
                match safe_copy_to_user(slice, a2) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -ENODEV
            }
        }

        /* =====================================================================
         * Category 8: Advanced Raw Block I/O & Storage Telemetry
         * ===================================================================== */
        SYS_PURPLE_STORAGE_COUNT => {
            block_dev::count() as i64
        }

        SYS_PURPLE_STORAGE_INFO => {
            let dev_idx = a1 as usize;
            let mut info = PurpleStorageDevInfo::default();
            info.dev_index = dev_idx as u32;

            let exists = block_dev::with_dev(dev_idx, |dev| {
                info.block_size = dev.block_size();
                info.total_blocks = dev.total_blocks();
                info.category = dev.category() as u32;
                let name_bytes = dev.name().as_bytes();
                let len = name_bytes.len().min(info.name.len() - 1);
                info.name[..len].copy_from_slice(&name_bytes[..len]);
            });

            if exists.is_some() {
                let slice = unsafe {
                    core::slice::from_raw_parts(&info as *const PurpleStorageDevInfo as *const u8, core::mem::size_of::<PurpleStorageDevInfo>())
                };
                match safe_copy_to_user(slice, a2) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -ENODEV
            }
        }

        SYS_PURPLE_STORAGE_READ_BLOCKS => {
            if !current_task_has_cap(PURPLE_CAP_RAW_STORAGE) {
                return -EPERM;
            }
            let dev_idx = a1 as usize;
            let lba = a2;
            if a3 > MAX_STORAGE_BLOCKS_PER_SYSCALL {
                return -EINVAL;
            }
            let count = a3 as u32;
            if count == 0 {
                return 0;
            }
            let buf_ptr = a4;
            let mut kbuf = [0u8; 16384];

            let bsize_opt = block_dev::with_dev(dev_idx, |dev| dev.block_size() as usize);
            let bsize = match bsize_opt {
                Some(bs) => bs,
                None => return -ENODEV,
            };
            let byte_count = (count as usize) * bsize;
            if byte_count > kbuf.len() {
                return -EINVAL;
            }

            let res = block_dev::with_dev(dev_idx, |dev| {
                dev.read_blocks(lba, count, &mut kbuf[..byte_count])
            });

            match res {
                Some(block_dev::BLOCK_DEV_OK) => {
                    if let Err(e) = safe_copy_to_user(&kbuf[..byte_count], buf_ptr) {
                        e
                    } else {
                        count as i64
                    }
                }
                Some(_) => -EIO,
                None => -ENODEV,
            }
        }

        SYS_PURPLE_STORAGE_WRITE_BLOCKS => {
            if !current_task_has_cap(PURPLE_CAP_RAW_STORAGE) {
                return -EPERM;
            }
            let dev_idx = a1 as usize;
            let lba = a2;
            if a3 > MAX_STORAGE_BLOCKS_PER_SYSCALL {
                return -EINVAL;
            }
            let count = a3 as u32;
            if count == 0 {
                return 0;
            }
            let buf_ptr = a4;
            let mut kbuf = [0u8; 16384];

            let bsize_opt = block_dev::with_dev(dev_idx, |dev| dev.block_size() as usize);
            let bsize = match bsize_opt {
                Some(bs) => bs,
                None => return -ENODEV,
            };
            let byte_count = (count as usize) * bsize;
            if byte_count > kbuf.len() {
                return -EINVAL;
            }
            if let Err(e) = safe_copy_from_user(buf_ptr, &mut kbuf[..byte_count]) {
                return e;
            }

            let res = block_dev::with_dev(dev_idx, |dev| {
                dev.write_blocks(lba, count, &kbuf[..byte_count])
            });

            match res {
                Some(block_dev::BLOCK_DEV_OK) => count as i64,
                Some(_) => -EIO,
                None => -ENODEV,
            }
        }

        SYS_PURPLE_STORAGE_FLUSH => {
            if !current_task_has_cap(PURPLE_CAP_RAW_STORAGE) {
                return -EPERM;
            }
            let dev_idx = a1 as usize;
            let res = block_dev::with_dev(dev_idx, |dev| {
                dev.flush()
            });
            match res {
                Some(block_dev::BLOCK_DEV_OK) => 0,
                Some(_) => -EIO,
                None => -ENODEV,
            }
        }

        /* =====================================================================
         * Category 9: Real-Time Input Subsystem & Raw Event Streaming
         * ===================================================================== */
        SYS_PURPLE_INPUT_MOUSE_STATE => {
            let m = mouse::MOUSE.lock();
            let state = PurpleMouseState {
                x: m.x,
                y: m.y,
                buttons: m.buttons as u32,
                scroll_delta: 0,
            };
            drop(m);

            let slice = unsafe {
                core::slice::from_raw_parts(&state as *const PurpleMouseState as *const u8, core::mem::size_of::<PurpleMouseState>())
            };
            match safe_copy_to_user(slice, a1) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        SYS_PURPLE_INPUT_KEY_EVENT => {
            if keyboard::has_char() {
                let c = keyboard::try_getchar().unwrap_or(0);
                let ev = PurpleInputEvent {
                    event_type: 1, // KeyPress
                    keycode: c,
                    ascii: c,
                    modifiers: 0,
                    data1: c as i32,
                    data2: 0,
                    timestamp_ticks: pit::get_ticks(),
                };
                let slice = unsafe {
                    core::slice::from_raw_parts(&ev as *const PurpleInputEvent as *const u8, core::mem::size_of::<PurpleInputEvent>())
                };
                match safe_copy_to_user(slice, a1) {
                    Ok(()) => 1,
                    Err(e) => e,
                }
            } else {
                -EAGAIN
            }
        }

        SYS_PURPLE_INPUT_POLL_EVENTS => {
            let max_ev = (a2 as usize).min(16);
            if max_ev == 0 {
                return 0;
            }
            let mut ev_count = 0usize;
            let mut events = [PurpleInputEvent::default(); 16];

            while ev_count < max_ev && keyboard::has_char() {
                if let Some(c) = keyboard::try_getchar() {
                    events[ev_count] = PurpleInputEvent {
                        event_type: 1,
                        keycode: c,
                        ascii: c,
                        modifiers: 0,
                        data1: c as i32,
                        data2: 0,
                        timestamp_ticks: pit::get_ticks(),
                    };
                    ev_count += 1;
                }
            }

            if ev_count > 0 {
                let byte_len = ev_count * core::mem::size_of::<PurpleInputEvent>();
                let slice = unsafe {
                    core::slice::from_raw_parts(events.as_ptr() as *const u8, byte_len)
                };
                if let Err(e) = safe_copy_to_user(slice, a1) {
                    return e;
                }
            }
            ev_count as i64
        }

        /* =====================================================================
         * Category 10: Process Management & Native Sandbox Security
         * ===================================================================== */
        SYS_PURPLE_TASK_SPAWN => -EOPNOTSUPP,

        SYS_PURPLE_TASK_KILL => {
            if !current_task_has_cap(PURPLE_CAP_SYS_ADMIN) {
                return -EPERM;
            }
            if a1 > u32::MAX as u64 { return -EINVAL; }
            let tid = a1 as u32;
            match task::kill(tid) {
                Ok(()) => 0,
                Err(_) => -ESRCH,
            }
        }

        SYS_PURPLE_TASK_GET_INFO => {
            if a1 > u32::MAX as u64 { return -EINVAL; }
            let tid = a1 as u32;
            let s = SCHED.lock();
            for t in s.tasks.iter() {
                if t.id == tid && t.state != TaskState::Unused {
                    let info = PurpleTaskInfo {
                        id: t.id,
                        state: t.state as u32,
                        priority: t.priority.as_u32(),
                        last_cpu: t.last_cpu,
                        time_slice_ticks: t.time_slice_ticks,
                        ticks_remaining: t.ticks_remaining,
                        cpu_affinity: t.cpu_affinity,
                        is_user: if t.is_user { 1 } else { 0 },
                        name: t.name,
                        cap_mask: t.cap_mask,
                    };
                    drop(s);
                    let slice = unsafe {
                        core::slice::from_raw_parts(&info as *const PurpleTaskInfo as *const u8, core::mem::size_of::<PurpleTaskInfo>())
                    };
                    return match safe_copy_to_user(slice, a2) {
                        Ok(()) => 0,
                        Err(e) => e,
                    };
                }
            }
            -ESRCH
        }

        SYS_PURPLE_TASK_LIST => {
            let max_count = (a2 as usize).min(MAX_TASKS);
            let mut list = [0u32; MAX_TASKS];
            let mut count = 0usize;
            let s = SCHED.lock();
            for t in s.tasks.iter() {
                if t.state != TaskState::Unused && count < max_count {
                    list[count] = t.id;
                    count += 1;
                }
            }
            drop(s);

            let byte_len = count * 4;
            let slice = unsafe {
                core::slice::from_raw_parts(list.as_ptr() as *const u8, byte_len)
            };
            if let Err(e) = safe_copy_to_user(slice, a1) {
                return e;
            }
            count as i64
        }

        SYS_PURPLE_SEC_CAP_DROP => {
            let cpu_id = task::get_current_cpu();
            let mut s = SCHED.lock();
            if let Some(idx) = s.current[cpu_id] {
                s.tasks[idx].cap_mask &= !a1;
                0
            } else {
                -ESRCH
            }
        }

        SYS_PURPLE_SEC_CAP_GET => {
            let cpu_id = task::get_current_cpu();
            let s = SCHED.lock();
            if let Some(idx) = s.current[cpu_id] {
                s.tasks[idx].cap_mask as i64
            } else {
                -ESRCH
            }
        }

        _ => -ENOSYS,
    }
}

/* =========================================================================
 * 8. IN-KERNEL VALIDATION & TEST SUITE FOR NATIVE SYSTEM CALLS
 * ========================================================================= */

pub fn run_native_syscall_self_test() -> (usize, usize) {
    serial::println("\n=======================================================");
    serial::println("  Aegis64 / PurpleOS Native System Call Validation Suite");
    serial::println("  Native syscall subsystem smoke checks");
    serial::println("=======================================================");

    let mut passed = 0usize;
    let mut skipped = 0usize;
    let mut total = 0usize;

    // Test 1: Category 1 - Framebuffer & 2D Rasterization
    total += 1;
    serial::print("[NATIVE TEST 1/10] Linear Framebuffer & 2D/3D GPU Info... ");
    let mut fb_info = PurpleFbInfo::default();
    let res = dispatch_native_syscall(SYS_PURPLE_FB_GET_INFO, &raw mut fb_info as u64, 0, 0, 0, 0, 0);
    if res == 0 && fb_info.width > 0 && fb_info.height > 0 {
        serial::print("[PASSED] (");
        serial::print_u64(fb_info.width as u64);
        serial::print("x");
        serial::print_u64(fb_info.height as u64);
        serial::println(" LFB verified)");
        passed += 1;
    } else {
        serial::println("[FAILED: No Framebuffer]");
    }

    // Test 2: Category 2 - Audio DSP & Volume Control
    total += 1;
    serial::print("[NATIVE TEST 2/10] Audio capability query and volume state... ");
    let mut audio_caps = PurpleAudioCaps::default();
    let res = dispatch_native_syscall(SYS_PURPLE_AUDIO_GET_CAPS, &raw mut audio_caps as u64, 0, 0, 0, 0, 0);
    let _ = dispatch_native_syscall(SYS_PURPLE_AUDIO_SET_VOLUME, 75, 0, 0, 0, 0, 0);
    let cur_vol = dispatch_native_syscall(SYS_PURPLE_AUDIO_GET_VOLUME, 0, 0, 0, 0, 0, 0);
    if res == 0 && cur_vol == 75 {
        serial::println("[PASSED] (Volume state set/get returned 75%)");
        passed += 1;
    } else {
        serial::println("[FAILED: Audio Caps mismatch]");
    }

    // Test 3: Category 3 - Low-Latency Channel IPC Ring Buffers
    total += 1;
    serial::print("[NATIVE TEST 3/10] Native Zero-Copy Channel IPC (Create, Send, Recv, Close)... ");
    let chan = dispatch_native_syscall(SYS_PURPLE_CHAN_CREATE, 0, 0, 0, 0, 0, 0);
    if chan > 0 {
        let msg = b"PURPLE_OS_HIGH_SPEED_IPC_TEST";
        let sent = dispatch_native_syscall(SYS_PURPLE_CHAN_SEND, chan as u64, msg.as_ptr() as u64, msg.len() as u64, 0, 0, 0);
        let mut recv_buf = [0u8; 64];
        let recv = dispatch_native_syscall(SYS_PURPLE_CHAN_RECV, chan as u64, recv_buf.as_mut_ptr() as u64, recv_buf.len() as u64, 0, 0, 0);
        let _ = dispatch_native_syscall(SYS_PURPLE_CHAN_CLOSE, chan as u64, 0, 0, 0, 0, 0);
        if sent == msg.len() as i64 && recv == msg.len() as i64 && &recv_buf[..msg.len()] == msg {
            serial::println("[PASSED] (Zero-copy payload matched)");
            passed += 1;
        } else {
            serial::println("[FAILED: Payload mismatch]");
        }
    } else {
        serial::println("[FAILED: Cannot create channel]");
    }

    // Test 4: Category 4 - Multi-Core SMP Parallelism & Affinity
    total += 1;
    serial::print("[NATIVE TEST 4/10] Multi-Core SMP Core Count & Affinity Controls... ");
    let cpus = dispatch_native_syscall(SYS_PURPLE_CPU_COUNT, 0, 0, 0, 0, 0, 0);
    let cur_cpu = dispatch_native_syscall(SYS_PURPLE_CPU_GET_CURRENT, 0, 0, 0, 0, 0, 0);
    let _ = dispatch_native_syscall(SYS_PURPLE_CPU_SET_AFFINITY, 0, 0x01, 0, 0, 0, 0);
    let aff = dispatch_native_syscall(SYS_PURPLE_CPU_GET_AFFINITY, 0, 0, 0, 0, 0, 0);
    if cpus >= 1 && cur_cpu >= 0 && (aff & 0x01) != 0 {
        serial::print("[PASSED] (Online CPUs: ");
        serial::print_u64(cpus as u64);
        serial::println(", Affinity mask: 0x01)");
        passed += 1;
    } else {
        serial::println("[FAILED: SMP CPU query error]");
    }

    // Test 5: Category 5 - High-Precision Clocks & Monotonic Timers
    total += 1;
    serial::print("[NATIVE TEST 5/10] High-Precision RTC Wall-Clock & Monotonic TSC Cycles... ");
    let mut wall = PurpleWallClock::default();
    let res = dispatch_native_syscall(SYS_PURPLE_CLOCK_WALL, &raw mut wall as u64, 0, 0, 0, 0, 0);
    let ticks = dispatch_native_syscall(SYS_PURPLE_CLOCK_TICKS, 0, 0, 0, 0, 0, 0);
    let cycles = dispatch_native_syscall(SYS_PURPLE_CLOCK_CYCLES, 0, 0, 0, 0, 0, 0);
    if res == 0 && wall.year >= 2024 && ticks > 0 && cycles > 0 {
        serial::print("[PASSED] (Year: ");
        serial::print_u64(wall.year as u64);
        serial::print(", Ticks: ");
        serial::print_u64(ticks as u64);
        serial::println(")");
        passed += 1;
    } else {
        serial::println("[FAILED: Clock invalid]");
    }

    // Test 6: Category 6 - Raw Layer-2 Direct DMA Networking
    total += 1;
    serial::print("[NATIVE TEST 6/10] Raw Layer-2 Network Device Telemetry & Stats... ");
    let mut net_info = PurpleNetDevInfo::default();
    let res = dispatch_native_syscall(SYS_PURPLE_NET_GET_DEV_INFO, &raw mut net_info as u64, 0, 0, 0, 0, 0);
    let mut stats = PurpleNetStats::default();
    let _ = dispatch_native_syscall(SYS_PURPLE_NET_GET_STATS, &raw mut stats as u64, 0, 0, 0, 0, 0);
    if res == 0 && net_info.driver_type != 0 {
        serial::print("[PASSED] (IP: ");
        serial::print_u64(net_info.ip[0] as u64);
        serial::print(".");
        serial::print_u64(net_info.ip[1] as u64);
        serial::print(".");
        serial::print_u64(net_info.ip[2] as u64);
        serial::print(".");
        serial::print_u64(net_info.ip[3] as u64);
        serial::println(")");
        passed += 1;
    } else {
        serial::println("[FAILED: Net info failed]");
    }

    // Test 7: Category 7 - PurpleDetect & Memory Manager Telemetry
    total += 1;
    serial::print("[NATIVE TEST 7/10] Hardware Device Introspection & PMM Memory Metrics... ");
    let dev_count = dispatch_native_syscall(SYS_PURPLE_DEVICE_COUNT, 0, 0, 0, 0, 0, 0);
    let mut mem_met = PurpleMemMetrics::default();
    let res = dispatch_native_syscall(SYS_PURPLE_MEM_METRICS, &raw mut mem_met as u64, 0, 0, 0, 0, 0);
    if dev_count >= 1 && res == 0 && mem_met.free_frames > 0 {
        serial::print("[PASSED] (Devices: ");
        serial::print_u64(dev_count as u64);
        serial::print(", Free Frames: ");
        serial::print_u64(mem_met.free_frames);
        serial::println(")");
        passed += 1;
    } else {
        serial::println("[FAILED: Devices / Memory metrics failed]");
    }

    // Test 8: Category 8 - Raw Block Storage Subsystem
    total += 1;
    serial::print("[NATIVE TEST 8/10] Advanced Raw Block I/O Device Inventory & Geometry... ");
    let stor_count = dispatch_native_syscall(SYS_PURPLE_STORAGE_COUNT, 0, 0, 0, 0, 0, 0);
    if stor_count >= 1 {
        let mut stor_info = PurpleStorageDevInfo::default();
        let res = dispatch_native_syscall(SYS_PURPLE_STORAGE_INFO, 0, &raw mut stor_info as u64, 0, 0, 0, 0);
        if res == 0 && stor_info.block_size == 512 {
            serial::print("[PASSED] (Disks: ");
            serial::print_u64(stor_count as u64);
            serial::print(", Block size: ");
            serial::print_u64(stor_info.block_size as u64);
            serial::println(" bytes)");
            passed += 1;
        } else {
            serial::println("[FAILED: Storage geometry error]");
        }
    } else {
        serial::println("[SKIPPED: No storage device detected]");
        skipped += 1;
    }

    // Test 9: Category 9 - Real-Time Input Subsystem (Mouse & Keyboard)
    total += 1;
    serial::print("[NATIVE TEST 9/10] Real-Time Input Subsystem (Mouse Coordinates & Keys)... ");
    let mut m_state = PurpleMouseState::default();
    let res = dispatch_native_syscall(SYS_PURPLE_INPUT_MOUSE_STATE, &raw mut m_state as u64, 0, 0, 0, 0, 0);
    if res == 0 {
        serial::println("[PASSED] (Mouse state query returned)");
        passed += 1;
    } else {
        serial::println("[FAILED: Input subsystem error]");
    }

    // Test 10: Category 10 - Process Management & Sandbox Security
    total += 1;
    serial::print("[NATIVE TEST 10/10] Native Process Capabilities & Security Sandbox... ");
    let caps = dispatch_native_syscall(SYS_PURPLE_SEC_CAP_GET, 0, 0, 0, 0, 0, 0);
    let _ = dispatch_native_syscall(SYS_PURPLE_SEC_CAP_DROP, PURPLE_CAP_RAW_STORAGE, 0, 0, 0, 0, 0);
    let dropped_caps = dispatch_native_syscall(SYS_PURPLE_SEC_CAP_GET, 0, 0, 0, 0, 0, 0);
    // Restore capability
    let cpu_id = task::get_current_cpu();
    let mut s = SCHED.lock();
    if let Some(idx) = s.current[cpu_id] {
        s.tasks[idx].cap_mask = PURPLE_CAP_ALL;
    }
    drop(s);
    if caps != 0 && (dropped_caps & PURPLE_CAP_RAW_STORAGE as i64) == 0 {
        serial::println("[PASSED] (Capability mask drop/get verified)");
        passed += 1;
    } else {
        serial::println("[FAILED: Sandbox capability failure]");
    }

    serial::println("-------------------------------------------------------");
    serial::print("Native Syscall Test Summary: ");
    serial::print_u64(passed as u64);
    serial::print("/");
    serial::print_u64(total as u64);
    serial::print(" Subsystem checks passed; skipped: ");
    serial::print_u64(skipped as u64);
    serial::println(" (not a conformance result)");
    serial::println("=======================================================\n");

    (passed, total)
}
