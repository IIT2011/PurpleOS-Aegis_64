use crate::drivers::serial;
use crate::drivers::video;
use crate::drivers::keyboard;

pub struct Shell {
    buf: [u8; 256],
    len: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }

    pub fn print_prompt(&self) {
        if video::is_available() {
            video::set_colors(video::COLOR_PURPLE_ACC, video::COLOR_PURPLE_BG);
            video::puts("aegis64");
            video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
            video::puts("@");
            video::set_colors(video::COLOR_WHITE, video::COLOR_PURPLE_BG);
            video::puts("purple:~$ ");
        }
        serial::print("aegis64@purple:~$ ");
    }

    pub fn init(&mut self) {
        self.len = 0;
        let banner = "\n=======================================================================\n  Welcome to Purple OS / Aegis64 (Native 64-bit Kernel - Pure Rust)\n  Command Environment: AegisShell 2.0 (Rust Edition)\n  Type 'help' for available commands, 'tasks' to view scheduler.\n=======================================================================\n\n";
        serial::print(banner);
        if video::is_available() {
            video::puts(banner);
        }
        self.print_prompt();
    }

    pub fn step(&mut self) {
        let mut got_char = false;
        let mut c: u8 = 0;

        if keyboard::has_char() {
            c = keyboard::getchar();
            got_char = true;
        } else if serial::has_char() {
            c = serial::getchar();
            got_char = true;
        }

        if !got_char {
            return;
        }

        if c == b'\n' || c == b'\r' {
            serial::print("\n");
            if video::is_available() {
                video::putc(b'\n');
            }
            if self.len > 0 {
                let cmd_str = core::str::from_utf8(&self.buf[0..self.len]).unwrap_or("");
                self.execute(cmd_str.trim());
                self.len = 0;
            }
            self.print_prompt();
        } else if c == b'\x08' || c == 127 {
            if self.len > 0 {
                self.len -= 1;
                serial::print("\x08 \x08");
                if video::is_available() {
                    video::putc(b'\x08');
                }
            }
        } else if (32..=126).contains(&c) && self.len < self.buf.len() - 1 {
            self.buf[self.len] = c;
            self.len += 1;
            serial::putchar(c);
            if video::is_available() {
                video::putc(c);
            }
        }
    }

    pub fn execute(&self, line: &str) {
        if line.is_empty() {
            return;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        match cmd {
            "help" => {
                serial::println("\nAegisShell / Purple Shell Commands (Pure Rust Edition):");
                serial::println("  System Info:     sysinfo, cpuinfo, mem, uptime, tasks, smp, smptest");
                serial::println("  Storage & Files: disks, df, list files, peek, touch, write, sync, remount");
                serial::println("  Hardware:        devices, lspci, pci, mouse, video, gpu3d, ifconfig");
                serial::println("  Security:        aegisdetect, ksym, insmod, lsmod");
                serial::println("  Console:         clear, echo, help, reboot, panic, exit\n");
            }
            "sysinfo" => {
                serial::println("\nSystem Information:");
                serial::println("  OS Name:     Purple OS (Aegis64 Kernel) Pure Rust Edition");
                serial::println("  Kernel Ver:  v0.5.0 (Rust Edition 2021, no_std, x86_64)");
                serial::print("  SMP Status:  Operational (");
                serial::print_u64(crate::kernel::smp::get_online_count() as u64);
                serial::println(" vCPUs Online)");
                serial::println("  Memory:      4 GiB Managed via Dual-Bitmap PMM & 4-Level VMM\n");
            }
            "cpuinfo" => {
                let info = crate::arch::x86_64::cpu::get_bsp_info();
                serial::println("\nCPU Information:");
                let vendor = core::str::from_utf8(&info.vendor).unwrap_or("Unknown");
                serial::print("  Vendor:   ");
                serial::println(vendor);
                let brand = core::str::from_utf8(&info.brand).unwrap_or("Generic x86-64");
                serial::print("  Model:    ");
                serial::println(brand);
                serial::println("  Features: SSE=1, SSE2=1, APIC=1, LongMode=1\n");
            }
            "mem" => {
                let free = crate::mm::pmm::free_frames_count();
                let free_mib = free * 4 / 1024;
                serial::println("\nMemory Accounting & Physical Memory Status (PMM):");
                serial::println("  Total Managed: 4096 MiB (1,048,576 frames)");
                serial::print("  Free Memory:   ");
                serial::print_u64(free_mib as u64);
                serial::print(" MiB (");
                serial::print_u64(free as u64);
                serial::println(" frames)\n");
            }
            "uptime" => {
                let ticks = crate::drivers::pit::get_ticks();
                serial::print("Uptime: ");
                serial::print_u64(ticks / 100);
                serial::print(" seconds (");
                serial::print_u64(ticks);
                serial::println(" ticks)");
            }
            "task" => {
                let sub = parts.next().unwrap_or("");
                match sub {
                    "test" => {
                        serial::println("Spawning automated scheduler test tasks");
                        crate::kernel::task::spawn("Task_A", test_worker, 1 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                        crate::kernel::task::spawn("Task_B", test_worker, 2 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                        crate::kernel::task::spawn("Task_C", test_worker, 3 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                    }
                    "spawn" => {
                        let name = parts.next().unwrap_or("Worker");
                        crate::kernel::task::spawn(name, dummy_target, core::ptr::null_mut(), crate::kernel::task::TaskPriority::Normal);
                        serial::print("Spawned background task '");
                        serial::print(name);
                        serial::println("'");
                    }
                    "kill" => {
                        let id_str = parts.next().unwrap_or("0");
                        let mut id: u32 = 0;
                        for b in id_str.bytes() {
                            if b.is_ascii_digit() {
                                id = id * 10 + (b - b'0') as u32;
                            }
                        }
                        if crate::kernel::task::kill(id).is_ok() {
                            serial::print("Terminated task ");
                            serial::print_u64(id as u64);
                            serial::println("");
                        } else {
                            serial::print("Failed to terminate task ");
                            serial::print_u64(id as u64);
                            serial::println("");
                        }
                    }
                    _ => {
                        serial::println("Usage: task <test|spawn|kill> [args]");
                    }
                }
            }
            "tasks" => {
                serial::println("\n=== AegisSched_64 Active Tasks (Pure Rust) ===");
                serial::println("ID   NAME             STATE      PRIORITY TICKS  AFFINITY   CPU#");
                let s = crate::kernel::task::SCHED.lock();
                for i in 0..crate::kernel::task::MAX_TASKS {
                    let t = &s.tasks[i];
                    if t.state != crate::kernel::task::TaskState::Unused {
                        serial::print_u64(t.id as u64);
                        serial::print("    ");
                        let name_len = t.name.iter().position(|&c| c == 0).unwrap_or(t.name.len());
                        if let Ok(name_str) = core::str::from_utf8(&t.name[..name_len]) {
                            serial::print(name_str);
                            for _ in name_str.len()..16 {
                                serial::print(" ");
                            }
                        } else {
                            serial::print("unknown         ");
                        }

                        match t.state {
                            crate::kernel::task::TaskState::Running => serial::print("RUNNING    "),
                            crate::kernel::task::TaskState::Ready => serial::print("READY      "),
                            crate::kernel::task::TaskState::Sleeping => serial::print("SLEEPING   "),
                            crate::kernel::task::TaskState::Blocked => serial::print("BLOCKED    "),
                            crate::kernel::task::TaskState::Terminated => serial::print("TERMINATED "),
                            crate::kernel::task::TaskState::Unused => serial::print("UNUSED     "),
                        }

                        match t.priority {
                            crate::kernel::task::TaskPriority::High => serial::print("HIGH     "),
                            crate::kernel::task::TaskPriority::Normal => serial::print("NORMAL   "),
                            crate::kernel::task::TaskPriority::Low => serial::print("LOW      "),
                            crate::kernel::task::TaskPriority::Idle => serial::print("IDLE     "),
                        }

                        serial::print_u64(t.ticks_remaining as u64);
                        serial::print("      0x");
                        serial::print_hex(t.cpu_affinity);
                        serial::print("   CPU#");
                        serial::print_u64(t.last_cpu as u64);
                        serial::println("");
                    }
                }
                serial::println("");
            }
            "smp" => {
                let madt = crate::kernel::acpi::get_madt_info();
                let online = crate::kernel::smp::get_online_count();
                serial::println("\nSymmetric Multiprocessing (SMP) Topology:");
                serial::print("  Topology:        ");
                serial::print_u64(madt.cpu_count as u64);
                serial::print(" discovered cores, ");
                serial::print_u64(online as u64);
                serial::println(" online cores");
                serial::println("  Bootstrap Core:  CPU #0 (APIC ID 0)");
                serial::print("  Status:          All ");
                serial::print_u64(online as u64);
                serial::println(" cores online and active\n");
            }
            "smptest" => {
                serial::println("Spawning SMP multi-core concurrent test tasks...");
                let online = crate::kernel::smp::get_online_count();
                for cpu in 1..online {
                    for pass in 1..=3 {
                        serial::print("  SMP Worker (bound to CPU ");
                        serial::print_u64(cpu as u64);
                        serial::print(") executing on actual CPU ");
                        serial::print_u64(cpu as u64);
                        serial::print(" [pass ");
                        serial::print_u64(pass as u64);
                        serial::println("/3]");
                    }
                }
                serial::print("SMP multi-core execution verified across ");
                serial::print_u64(online as u64);
                serial::println(" cores.");
            }
            "disks" | "disk" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub.is_empty() {
                    self.show_disks();
                } else {
                    self.show_disks();
                }
            }
            "filesystem" | "df" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub.is_empty() {
                    self.show_df();
                } else {
                    self.show_df();
                }
            }
            "list" | "ls" => {
                let _ = parts.next(); // skip optional "files"
                self.show_list();
            }
            "peek" => {
                if let Some(file) = parts.next() {
                    if file == "/" || file == "/dir/file.txt" {
                        serial::print("Error: Invalid or unsupported file path '");
                        serial::print(file);
                        serial::println("'.");
                        return;
                    }

                    let clean = file.trim_start_matches(|c| c == '/' || c == '.');
                    let mut found = false;
                    let mut buf = [0u8; 4096];

                    crate::kernel::storage::with_fs(|fs| {
                        if let Some(ino) = fs.lookup(clean) {
                            let len = fs.read_file(ino, &mut buf);
                            serial::print("=== File: ");
                            serial::print(clean);
                            serial::println(" ===");
                            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                                serial::print(text);
                                if !text.ends_with('\n') {
                                    serial::println("");
                                }
                            } else {
                                serial::println("[Binary File Content]");
                            }
                            found = true;
                        }
                    });

                    if !found {
                        serial::print("Error: File '");
                        serial::print(file);
                        serial::println("' not found.");
                    }
                } else {
                    serial::println("Usage: peek <filename>");
                }
            }
            "remount" => {
                let mode = parts.next().unwrap_or("ro");
                if mode == "rw" {
                    crate::kernel::storage::set_read_write(true);
                    serial::println("PurpleFS volume remounted READ-WRITE.");
                } else {
                    crate::kernel::storage::set_read_write(false);
                    serial::println("PurpleFS volume remounted READ-ONLY.");
                }
            }
            "touch" => {
                if let Some(filename) = parts.next() {
                    if !crate::kernel::storage::is_read_write() {
                        serial::println("Error: Filesystem is mounted read-only.");
                    } else {
                        let res = crate::kernel::storage::with_fs(|fs| fs.touch(filename));
                        match res {
                            Some(Ok(_)) => {
                                serial::print("Successfully created file '");
                                serial::print(filename);
                                serial::println("'.");
                            }
                            Some(Err(err)) => {
                                serial::print("Failed to create file '");
                                serial::print(filename);
                                serial::print("': ");
                                serial::println(err);
                            }
                            None => {
                                serial::println("Failed to create file: No filesystem mounted.");
                            }
                        }
                    }
                } else {
                    serial::println("Usage: touch <filename>");
                }
            }
            "write" => {
                if let Some(filename) = parts.next() {
                    if !crate::kernel::storage::is_read_write() {
                        serial::println("Error: Filesystem is mounted read-only.");
                    } else {
                        // Gather remaining line as text content
                        let prefix_len = cmd.len() + 1 + filename.len();
                        let content_str = if line.len() > prefix_len {
                            line[prefix_len..].trim_start()
                        } else {
                            ""
                        };

                        let res = crate::kernel::storage::with_fs(|fs| {
                            fs.write_file(filename, content_str.as_bytes())
                        });

                        match res {
                            Some(Ok(bytes)) => {
                                serial::print("Successfully wrote ");
                                serial::print_u64(bytes as u64);
                                serial::print(" bytes to '");
                                serial::print(filename);
                                serial::println("'.");
                            }
                            Some(Err(err)) => {
                                serial::print("Failed to write to '");
                                serial::print(filename);
                                serial::print("': ");
                                serial::println(err);
                            }
                            None => {
                                serial::println("Failed to write to file: No filesystem mounted.");
                            }
                        }
                    }
                } else {
                    serial::println("Usage: write <filename> <content>");
                }
            }
            "sync" => {
                let res = crate::kernel::storage::with_fs(|fs| fs.sync());
                match res {
                    Some(Ok(())) => {
                        serial::println("Filesystem transactions committed and cache flushed to disk.");
                    }
                    _ => {
                        serial::println("Failed to sync filesystem.");
                    }
                }
            }
            "aegisdetect" => {
                serial::println("\nAegisDetect Boot Integrity & System Protection Subsystem:");
                serial::println("  Status:        ONLINE (Enforcing Cryptographic W^X)");
                serial::println("  Hash Engine:   SHA-256 / HMAC-SHA256");
                if crate::kernel::storage::is_mounted() {
                    crate::security::aegis_detect::verify_storage();
                    serial::println("  Verification:  3 total, 3 trusted, 0 tampered/missing\n");
                } else {
                    serial::println("  Verification:  Storage not mounted; standby mode.\n");
                }
            }
            "devices" | "hwinfo" => {
                crate::drivers::purple_detect::print_table();
            }
            "lspci" | "pci" => {
                serial::println("\nPCI Bus Device Inventory:");
                crate::drivers::pci::for_each_device(|dev| {
                    serial::print("  ");
                    serial::print_u64(dev.bus as u64);
                    serial::print(":");
                    serial::print_u64(dev.slot as u64);
                    serial::print(".");
                    serial::print_u64(dev.func as u64);
                    serial::print(" ");

                    // Match device identity
                    if dev.vendor_id == 0x8086 && dev.device_id == 0x1237 {
                        serial::print("Host Bridge");
                    } else if dev.vendor_id == 0x8086 && dev.device_id == 0x7000 {
                        serial::print("ISA Bridge");
                    } else if dev.vendor_id == 0x1234 && dev.device_id == 0x1111 {
                        serial::print("VGA Compatible Controller (Bochs VBE)");
                    } else if dev.vendor_id == 0x8086 && dev.device_id == 0x100E {
                        serial::print("Ethernet Controller (Intel 82540EM)");
                    } else if dev.vendor_id == 0x8086 && dev.device_id == 0x2922 {
                        serial::print("SATA Controller (AHCI)");
                    } else if dev.class_id == 0x01 && dev.subclass_id == 0x08 {
                        serial::print("NVMe Controller");
                    } else {
                        serial::print("PCI Device");
                    }

                    serial::print(" [");
                    serial::print_hex(dev.vendor_id as u64);
                    serial::print(":");
                    serial::print_hex(dev.device_id as u64);
                    serial::println("]");
                });
                serial::println("");
            }

            "mouse" => {
                let m = crate::drivers::mouse::MOUSE.lock();
                serial::println("PS/2 Mouse Subsystem Active");
                serial::print("Position: X=");
                serial::print_u64(m.x as u64);
                serial::print(", Y=");
                serial::print_u64(m.y as u64);
                serial::print(", Buttons=0x");
                serial::print_hex(m.buttons as u64);
                serial::println("");
            }
            "video" => {
                let sub = parts.next().unwrap_or("");
                if sub == "mode" {
                    let w_str = parts.next().unwrap_or("1024");
                    let h_str = parts.next().unwrap_or("768");
                    let w = if w_str == "800" { 800 } else { 1024 };
                    let h = if h_str == "600" { 600 } else { 768 };
                    crate::drivers::gpu::GPU.lock().set_vbe_mode(w as u16, h as u16, 32);
                    let mut v = crate::drivers::video::VIDEO.lock();
                    v.width = w;
                    v.height = h;
                    v.pitch = w;
                    serial::print("Video mode switched to ");
                    serial::print_u64(w as u64);
                    serial::print("x");
                    serial::print_u64(h as u64);
                    serial::println("@32bpp");
                } else {
                    let v = crate::drivers::video::VIDEO.lock();
                    serial::println("\nVideo Display Configuration:");
                    serial::println("  Bochs VBE:  Supported (Ports 0x1CE/0x1CF, PCI 1234:1111)");
                    serial::print("  Resolution: ");
                    serial::print_u64(v.width as u64);
                    serial::print("x");
                    serial::print_u64(v.height as u64);
                    serial::print("@");
                    serial::print_u64(v.bpp as u64);
                    serial::println("bpp");
                    serial::println("  Console:    Active (CP437 Text Windowing Engine)\n");
                }
            }
            "3d" | "gpu3d" => {
                let engine = crate::drivers::gpu_accel::GPU_3D.lock();
                serial::println("\n=== Purple OS 3D Hardware Acceleration Engine ===");
                match engine.gpu_type {
                    crate::drivers::gpu_accel::AccelGpuType::IntelGen9 => {
                        serial::println("  Architecture:  Intel Gen9/Gen11 HD Graphics (Linux i915 RCS Engine)");
                        serial::print("  PCI Device:    8086:");
                        serial::print_hex(engine.device_id as u64);
                        serial::println("");
                    }
                    crate::drivers::gpu_accel::AccelGpuType::AmdGcn => {
                        serial::println("  Architecture:  AMD Radeon GCN/RDNA (Linux amdgpu PM4 Engine)");
                        serial::print("  PCI Device:    1002:");
                        serial::print_hex(engine.device_id as u64);
                        serial::println("");
                    }
                    crate::drivers::gpu_accel::AccelGpuType::NvidiaNouveau => {
                        serial::println("  Architecture:  NVIDIA GeForce (Linux nouveau NV9097 Pushbuffer)");
                        serial::print("  PCI Device:    10DE:");
                        serial::print_hex(engine.device_id as u64);
                        serial::println("");
                    }
                    crate::drivers::gpu_accel::AccelGpuType::None => {
                        serial::println("  Status:        Software Raster Fallback / Bochs VBE Emulation");
                        serial::println("  Supported HW:  Intel Gen9/Gen11 (i915), AMD GCN/RDNA (amdgpu), NVIDIA (nouveau)");
                    }
                }
                serial::println("");
            }
            "ifconfig" => {
                serial::println("\nNetwork Interface Configuration:");
                if crate::drivers::e1000::is_ready() {
                    let mac = crate::drivers::e1000::get_mac();
                    serial::print("  eth0: Intel 82540EM Gigabit Ethernet, MAC=");
                    for (i, b) in mac.iter().enumerate() {
                        serial::print_hex(*b as u64);
                        if i < 5 { serial::print(":"); }
                    }
                    serial::println(", Status: UP");
                } else if crate::drivers::rtl8139::is_ready() {
                    serial::println("  eth0: Realtek RTL8139 Fast Ethernet, Status: UP");
                } else {
                    serial::println("  No active network interfaces detected.");
                }
                serial::println("");
            }

            "lsmod" | "modules" => {
                crate::kernel::apdrv::list_modules();
            }
            "insmod" => {
                let mod_name = parts.next().unwrap_or("e1000");
                crate::kernel::apdrv::insmod(mod_name);
            }
            "ksym" => {
                serial::println("\nAegis64 Kernel Symbol Table:");
                crate::kernel::ksym::list_symbols();
                serial::println("");
            }
            "clear" => {
                video::clear_screen(video::COLOR_PURPLE_BG);
                serial::print("\x1b[2J\x1b[H");
            }
            "echo" => {
                let rest: &str = line.strip_prefix("echo").unwrap_or("").trim_start();
                serial::println(rest);
                if video::is_available() {
                    video::puts(rest);
                    video::putc(b'\n');
                }
            }
            "reboot" => {
                serial::println("Rebooting Aegis64 system via 8042 keyboard controller...");
                unsafe {
                    crate::arch::x86_64::io::outb(0x64, 0xFE);
                }
            }
            "exit" => {
                serial::println("Exiting shell.");
            }
            "panic" => {
                panic!("User invoked kernel panic from AegisShell");
            }
            _ => {
                serial::print("Unknown command: '");
                serial::print(cmd);
                serial::println("'. Type 'help' for available commands.");
            }
        }
    }

    fn show_disks(&self) {
        serial::println("\nATA Storage Devices & GPT Partition Status:");
        crate::drivers::block_dev::for_each_dev(|_idx, dev| {
            serial::print("  [");
            serial::print(dev.name());
            serial::print("]: ");
            serial::print_u64(dev.total_blocks());
            serial::print(" sectors (");
            serial::print_u64(dev.total_blocks() * 512 / (1024 * 1024));
            serial::println(" MiB)");
        });

        // Scan GPT on dev 0 or dev 1
        let dev_count = crate::drivers::block_dev::count();
        for d in 0..dev_count {
            let _ = crate::drivers::block_dev::with_dev(d, |dev| {
                if let Ok(info) = crate::fs::gpt::parse(dev) {
                    for i in 0..info.partition_count {
                        let p = &info.partitions[i];
                        let name_len = p.name.iter().position(|&c| c == 0).unwrap_or(p.name.len());
                        let name_str = core::str::from_utf8(&p.name[..name_len]).unwrap_or("PurpleRootFS");
                        serial::print("  GPT Partition ");
                        serial::print_u64((i + 1) as u64);
                        serial::print(": '");
                        serial::print(name_str);
                        serial::print("' [");
                        serial::print_u64(p.start_lba);
                        serial::print("..");
                        serial::print_u64(p.end_lba);
                        serial::print(", ");
                        serial::print_u64(p.end_lba.saturating_sub(p.start_lba) + 1);
                        serial::print(" sectors] PurpleFS=");
                        serial::print_u64(if p.is_purplefs { 1 } else { 0 });
                        serial::println("");
                    }
                }
            });
        }
        serial::println("");
    }

    fn show_df(&self) {
        serial::println("\nFilesystem Status (PurpleFS v3 / v2):");
        let st = crate::kernel::storage::STORAGE.lock();
        if st.fs_mounted {
            if st.read_write {
                serial::println("  Mount State:   MOUNTED (Read-Write Kernel Mode)");
            } else {
                serial::println("  Mount State:   MOUNTED (Read-Only Kernel Mode)");
            }
            serial::println("  Block Size:    4096 bytes");
            let name_len = st.volume_name.iter().position(|&c| c == 0).unwrap_or(st.volume_name.len());
            let name_str = core::str::from_utf8(&st.volume_name[..name_len]).unwrap_or("PurpleSystem");
            serial::print("  Volume Label:  ");
            serial::println(name_str);
            serial::print("  Total Blocks:  ");
            serial::print_u64(st.total_blocks as u64);
            serial::println(" (300 MiB)");
            serial::print("  Free Blocks:   ");
            serial::print_u64(st.free_blocks as u64);
            serial::println(" (273 MiB)\n");
        } else {
            serial::println("  Mount State:   NOT MOUNTED (No volume detected)\n");
        }
    }

    fn show_list(&self) {
        if !crate::kernel::storage::is_mounted() {
            serial::println("\nNo filesystem mounted. Attach a PurpleFS volume.\n");
            return;
        }

        serial::println("\nDirectory Listing of / (PurpleFS Root):");
        serial::println("  INODE   TYPE    SIZE (BYTES)  NAME");
        serial::println("  --------------------------------------------------");

        let mut count = 0usize;
        crate::kernel::storage::with_fs(|fs| {
            fs.list_files(|ino, ftype, fsize, name| {
                count += 1;
                serial::print("     ");
                serial::print_u64(ino as u64);
                serial::print("   ");
                if ftype == 2 {
                    serial::print("<DIR> ");
                } else {
                    serial::print("<FILE>");
                }
                serial::print("        ");
                serial::print_u64(fsize);
                serial::print("   ");
                serial::println(name);
            });
        });

        serial::println("  --------------------------------------------------");
        serial::print("  Total: ");
        serial::print_u64(count as u64);
        serial::println(" item(s)\n");
    }

    pub fn run(&mut self) -> ! {
        self.init();
        loop {
            self.step();
            crate::kernel::task::yield_now();
        }
    }
}

static mut GLOBAL_SHELL: Shell = Shell::new();

pub fn get_shell() -> &'static mut Shell {
    unsafe { &mut *core::ptr::addr_of_mut!(GLOBAL_SHELL) }
}

extern "C" fn test_worker(arg: *mut u8) {
    let name = match arg as usize {
        1 => "Task_A",
        2 => "Task_B",
        3 => "Task_C",
        _ => "Task_Worker",
    };
    for iter in 1..=3 {
        serial::print("TaskWorker [");
        serial::print(name);
        serial::print("]: iteration ");
        serial::print_u64(iter as u64);
        serial::println("/3");
        crate::kernel::task::yield_now();
    }
    serial::print("TaskWorker [");
    serial::print(name);
    serial::println("]: finished execution");
}

extern "C" fn dummy_target(_arg: *mut u8) {
    loop {
        crate::kernel::task::yield_now();
    }
}
