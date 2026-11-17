use crate::arch::x86_64::cpu::{hlt, sti};
use crate::mm::pmm::{alloc_frames, free_frames};
use crate::sync::Spinlock;

pub const MAX_TASKS: usize = 32;
pub const MAX_CPUS: usize = 8;
pub const TASK_STACK_SIZE: usize = 16384;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TaskState {
    Unused,
    Ready,
    Running,
    Sleeping,
    Blocked,
    Terminated,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TaskPriority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
}

pub struct Task {
    pub id: u32,
    pub name: [u8; 16],
    pub rsp: u64,
    pub stack_base: u64,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub time_slice_ticks: u32,
    pub ticks_remaining: u32,
    pub sleep_until_ticks: u64,
    pub cpu_affinity: u64,
    pub last_cpu: u32,
    pub is_user: bool,
    pub cr3: u64,
    // POSIX Process Control Block
    pub ppid: u32,
    pub pgid: u32,
    pub exit_code: i32,
    pub is_zombie: bool,
    pub fd_table: [crate::kernel::posix::FileDescriptor; crate::kernel::posix::MAX_FD_PER_TASK],
    pub vmas: [crate::kernel::posix::Vma; crate::kernel::posix::MAX_VMAS_PER_TASK],
    pub cwd: [u8; 64],
    pub cwd_len: usize,
    pub umask: u32,
    pub brk_start: u64,
    pub brk_current: u64,
    pub sig_pending: u64,
    pub sig_mask: u64,
    pub sig_handlers: [u64; 64],
}

const fn default_fd_table() -> [crate::kernel::posix::FileDescriptor; crate::kernel::posix::MAX_FD_PER_TASK] {
    crate::kernel::posix::default_fd_table()
}

impl Task {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            name: [0; 16],
            rsp: 0,
            stack_base: 0,
            state: TaskState::Unused,
            priority: TaskPriority::Normal,
            time_slice_ticks: 10,
            ticks_remaining: 10,
            sleep_until_ticks: 0,
            cpu_affinity: !0,
            last_cpu: 0,
            is_user: false,
            cr3: 0,
            ppid: 0,
            pgid: 1,
            exit_code: 0,
            is_zombie: false,
            fd_table: default_fd_table(),
            vmas: [crate::kernel::posix::Vma::empty(); crate::kernel::posix::MAX_VMAS_PER_TASK],
            cwd: [b'/', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            cwd_len: 1,
            umask: 0o022,
            brk_start: 0x0000_0000_1000_0000,
            brk_current: 0x0000_0000_1000_0000,
            sig_pending: 0,
            sig_mask: 0,
            sig_handlers: [0; 64],
        }
    }
}

pub struct SchedState {
    pub tasks: [Task; MAX_TASKS],
    pub current: [Option<usize>; MAX_CPUS],
    pub next_id: u32,
    pub ticks: u64,
}

pub static SCHED: Spinlock<SchedState> = Spinlock::new(SchedState {
    tasks: [
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
        Task::empty(), Task::empty(), Task::empty(), Task::empty(),
    ],
    current: [None; MAX_CPUS],
    next_id: 1,
    ticks: 0,
});

extern "C" {
    fn switch_context(old_rsp: *mut u64, new_rsp: u64, lock_to_release: *const core::sync::atomic::AtomicBool);
    fn task_trampoline();
    #[allow(dead_code)]
    fn user_trampoline();
}

#[unsafe(no_mangle)]
pub extern "C" fn task_entry_wrapper(entry: extern "C" fn(*mut u8), arg: *mut u8) {
    sti();
    entry(arg);
    exit();
}

pub fn init() {
    let mut s = SCHED.lock();
    s.next_id = 1;

    // Task 0 is kernel main on CPU 0
    let main_idx = 0;
    let tid = s.next_id;
    s.next_id += 1;
    let t = &mut s.tasks[main_idx];
    t.id = tid;
    let name = b"kernel_main\0";
    t.name[0..name.len()].copy_from_slice(name);
    t.state = TaskState::Running;
    t.priority = TaskPriority::Normal;
    t.time_slice_ticks = 10;
    t.ticks_remaining = 10;
    t.cpu_affinity = 1;
    t.last_cpu = 0;
    s.current[0] = Some(main_idx);
    drop(s);

    // Spawn idle task (ID 2) with proper stack
    spawn("idle", idle_task, core::ptr::null_mut(), TaskPriority::Idle);

    crate::drivers::serial::println("Scheduler: AegisSched_64 / PurpleSched Pure Rust Scheduler Initialized");
}

extern "C" fn idle_task(_arg: *mut u8) {
    loop {
        hlt();
    }
}

pub fn init_ap(cpu_id: usize) {
    let mut s = SCHED.lock();
    // Idle task for this AP
    if let Some(idx) = alloc_task_slot(&mut s) {
        let tid = s.next_id;
        s.next_id += 1;
        let t = &mut s.tasks[idx];
        t.id = tid;
        let mut name = [0u8; 16];
        name[0] = b'i';
        name[1] = b'd';
        name[2] = b'l';
        name[3] = b'e';
        name[4] = b'-';
        name[5] = b'0' + (cpu_id as u8);
        t.name = name;
        t.state = TaskState::Running;
        t.priority = TaskPriority::Idle;
        t.cpu_affinity = 1 << cpu_id;
        t.last_cpu = cpu_id as u32;
        s.current[cpu_id] = Some(idx);
    }
}

fn alloc_task_slot(s: &mut SchedState) -> Option<usize> {
    for i in 0..MAX_TASKS {
        if s.tasks[i].state == TaskState::Unused {
            return Some(i);
        } else if s.tasks[i].state == TaskState::Terminated {
            // Safely reclaim terminated task's stack frames now that it has completely stopped running
            if s.tasks[i].stack_base != 0 {
                let _ = free_frames(s.tasks[i].stack_base as usize, 4);
                s.tasks[i].stack_base = 0;
            }
            if s.tasks[i].cr3 != 0 {
                let _ = crate::mm::pmm::free_frame(s.tasks[i].cr3 as usize);
                s.tasks[i].cr3 = 0;
            }
            s.tasks[i].state = TaskState::Unused;
            return Some(i);
        }
    }
    None
}

pub fn spawn(name_str: &str, entry: extern "C" fn(*mut u8), arg: *mut u8, prio: TaskPriority) -> Option<u32> {
    spawn_affined(name_str, entry, arg, prio, !0)
}

pub fn spawn_affined(name_str: &str, entry: extern "C" fn(*mut u8), arg: *mut u8, prio: TaskPriority, affinity: u64) -> Option<u32> {
    let stack_phys = alloc_frames(4)?;
    let stack_top = stack_phys + TASK_STACK_SIZE;

    let mut s = SCHED.lock();
    let idx = alloc_task_slot(&mut s)?;

    let tid = s.next_id;
    s.next_id += 1;

    let t = &mut s.tasks[idx];
    t.id = tid;
    let mut name = [0u8; 16];
    let b = name_str.as_bytes();
    let len = b.len().min(15);
    name[0..len].copy_from_slice(&b[0..len]);
    t.name = name;

    t.stack_base = stack_phys as u64;
    t.state = TaskState::Ready;
    t.priority = prio;
    t.time_slice_ticks = match prio {
        TaskPriority::High => 20,
        TaskPriority::Normal => 10,
        TaskPriority::Low => 5,
        TaskPriority::Idle => 1,
    };
    t.ticks_remaining = t.time_slice_ticks;
    t.cpu_affinity = affinity;
    t.is_user = false;

    // Setup initial kernel stack frame for switch_context:
    // [stack layout for task_trampoline]: entry function pointer, arg pointer
    // [rsp+0]: task_trampoline return address
    // [rsp+8..]: pushed callee-saved registers (r15, r14, r13, r12, rbp, rbx, rflags)
    unsafe {
        let mut sp = stack_top as *mut u64;

        // Arguments for task_trampoline
        sp = sp.sub(1);
        *sp = arg as u64;
        sp = sp.sub(1);
        *sp = entry as *const () as usize as u64;

        // Trampoline return address
        sp = sp.sub(1);
        *sp = task_trampoline as *const () as usize as u64;

        // Callee-saved registers popped by switch_context
        sp = sp.sub(1);
        *sp = 0x202; // RFLAGS with IF enabled
        sp = sp.sub(1);
        *sp = 0; // RBX
        sp = sp.sub(1);
        *sp = 0; // RBP
        sp = sp.sub(1);
        *sp = 0; // R12
        sp = sp.sub(1);
        *sp = 0; // R13
        sp = sp.sub(1);
        *sp = 0; // R14
        sp = sp.sub(1);
        *sp = 0; // R15

        t.rsp = sp as u64;
    }

    crate::kernel::smp::broadcast_resched();
    Some(tid)
}

pub fn spawn_user_task(
    name_str: &str,
    entry: extern "C" fn(*mut u8),
    arg: *mut u8,
    prio: TaskPriority,
) -> Option<u32> {
    let pml4 = crate::mm::vmm::create_pml4()?;
    let tid = spawn_affined(name_str, entry, arg, prio, !0)?;
    let mut s = SCHED.lock();
    for t in s.tasks.iter_mut() {
        if t.id == tid {
            t.is_user = true;
            t.cr3 = pml4 as u64;
            break;
        }
    }
    Some(tid)
}

#[inline(always)]
pub fn get_current_cpu() -> usize {
    crate::kernel::smp::get_cpu_id()
}

pub fn yield_now() {
    schedule();
}

pub fn sleep(ticks: u64) {
    let cpu_id = get_current_cpu();
    let mut s = SCHED.lock();
    if let Some(cur_idx) = s.current[cpu_id] {
        let now = s.ticks;
        s.tasks[cur_idx].sleep_until_ticks = now + ticks;
        s.tasks[cur_idx].state = TaskState::Sleeping;
    }
    drop(s);
    schedule();
}

pub fn exit() -> ! {
    let cpu_id = get_current_cpu();
    let mut s = SCHED.lock();
    if let Some(cur_idx) = s.current[cpu_id] {
        s.tasks[cur_idx].state = TaskState::Terminated;
        // Do NOT free stack while still executing on it; alloc_task_slot reaps it safely
    }
    drop(s);
    schedule();
    loop {
        hlt();
    }
}

pub fn kill(task_id: u32) -> Result<(), &'static str> {
    if task_id == 1 {
        return Err("Cannot kill kernel main");
    }
    let mut s = SCHED.lock();
    for i in 0..MAX_TASKS {
        if s.tasks[i].id == task_id && s.tasks[i].state != TaskState::Terminated && s.tasks[i].state != TaskState::Unused {
            s.tasks[i].state = TaskState::Terminated;
            // Let alloc_task_slot safely reclaim the stack
            return Ok(());
        }
    }
    Err("Task not found")
}

fn pick_next(s: &mut SchedState, cpu_id: usize) -> Option<usize> {
    let now = s.ticks;

    // Wake sleeping tasks
    for i in 0..MAX_TASKS {
        if s.tasks[i].state == TaskState::Sleeping && now >= s.tasks[i].sleep_until_ticks {
            s.tasks[i].state = TaskState::Ready;
        }
    }

    // Pick highest priority ready task matching affinity
    for prio in [TaskPriority::High, TaskPriority::Normal, TaskPriority::Low, TaskPriority::Idle] {
        for i in 0..MAX_TASKS {
            if s.tasks[i].state == TaskState::Ready
                && s.tasks[i].priority == prio
                && (s.tasks[i].cpu_affinity & (1 << cpu_id)) != 0
            {
                return Some(i);
            }
        }
    }

    // Current task can continue if ready
    if let Some(cur) = s.current[cpu_id] {
        if s.tasks[cur].state == TaskState::Running {
            return Some(cur);
        }
    }

    None
}

pub fn schedule() {
    let cpu_id = get_current_cpu();
    let mut s = SCHED.lock();

    let cur_idx_opt = s.current[cpu_id];
    let next_idx_opt = pick_next(&mut s, cpu_id);

    let next_idx = match next_idx_opt {
        Some(idx) => idx,
        None => return, // Idle or continue
    };

    if cur_idx_opt == Some(next_idx) {
        return;
    }

    let mut old_rsp_ptr: *mut u64 = core::ptr::null_mut();
    let new_rsp: u64 = s.tasks[next_idx].rsp;

    if let Some(cur_idx) = cur_idx_opt {
        if s.tasks[cur_idx].state == TaskState::Running {
            s.tasks[cur_idx].state = TaskState::Ready;
        }
        old_rsp_ptr = &mut s.tasks[cur_idx].rsp as *mut u64;
    }

    s.tasks[next_idx].state = TaskState::Running;
    s.tasks[next_idx].ticks_remaining = s.tasks[next_idx].time_slice_ticks;
    s.tasks[next_idx].last_cpu = cpu_id as u32;
    s.current[cpu_id] = Some(next_idx);

    let new_cr3: u64 = s.tasks[next_idx].cr3;
    let target_cr3 = if new_cr3 != 0 {
        new_cr3
    } else {
        crate::mm::vmm::get_kernel_pml4() as u64
    };
    if target_cr3 != 0 {
        let cur_cr3 = crate::arch::x86_64::cpu::read_cr3();
        if cur_cr3 != target_cr3 {
            crate::arch::x86_64::cpu::write_cr3(target_cr3);
        }
    }

    let raw_lock = SCHED.raw_lock_ptr();
    core::mem::forget(s);

    unsafe {
        if !old_rsp_ptr.is_null() {
            switch_context(old_rsp_ptr, new_rsp, raw_lock);
        } else {
            let mut discard_prev_rsp: u64 = 0;
            switch_context(&mut discard_prev_rsp, new_rsp, raw_lock);
        }
    }
}

pub fn timer_tick() {
    let mut s = SCHED.lock();
    s.ticks += 1;

    let cpu_id = get_current_cpu();
    let mut needs_sched = false;
    if let Some(cur_idx) = s.current[cpu_id] {
        if s.tasks[cur_idx].ticks_remaining > 0 {
            s.tasks[cur_idx].ticks_remaining -= 1;
        }
        if s.tasks[cur_idx].ticks_remaining == 0 {
            needs_sched = true;
        }
    }
    drop(s);

    if needs_sched {
        schedule();
    }
}
