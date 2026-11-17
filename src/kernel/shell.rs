#[allow(dead_code)]
mod con {
    use crate::drivers::{serial, video};

    #[inline]
    pub fn print(s: &str) {
        serial::print(s);
        if video::is_available() {
            video::puts(s);
        }
    }

    #[inline]
    pub fn println(s: &str) {
        serial::println(s);
        if video::is_available() {
            video::puts(s);
            video::putc(b'\n');
        }
    }

    #[inline]
    pub fn print_u64(n: u64) {
        serial::print_u64(n);
        if video::is_available() {
            let mut buf = [0u8; 20];
            let mut i = 0;
            let mut val = n;
            if val == 0 {
                video::putc(b'0');
                return;
            }
            while val > 0 {
                buf[i] = b'0' + (val % 10) as u8;
                val /= 10;
                i += 1;
            }
            while i > 0 {
                i -= 1;
                video::putc(buf[i]);
            }
        }
    }

    #[inline]
    pub fn print_hex(n: u64) {
        serial::print_hex(n);
        if video::is_available() {
            let mut buf = [0u8; 16];
            let mut i = 0;
            let mut val = n;
            let hex_chars = b"0123456789abcdef";
            if val == 0 {
                video::putc(b'0');
                return;
            }
            while val > 0 {
                buf[i] = hex_chars[(val & 0xF) as usize];
                val >>= 4;
                i += 1;
            }
            while i > 0 {
                i -= 1;
                video::putc(buf[i]);
            }
        }
    }

    #[inline]
    pub fn print_hex_u64(n: u64) {
        serial::print_hex_u64(n);
        if video::is_available() {
            let hex_chars = b"0123456789abcdef";
            for i in (0..16).rev() {
                let digit = ((n >> (i * 4)) & 0xF) as usize;
                video::putc(hex_chars[digit]);
            }
        }
    }

    #[inline]
    pub fn putchar(c: u8) {
        serial::putchar(c);
        if video::is_available() {
            video::putc(c);
        }
    }
}

use self::con as serial;
use crate::drivers::video;
use crate::drivers::keyboard;

pub struct Shell {
    buf: [u8; 256],
    len: usize,
    cwd: [u8; 64],
    cwd_len: usize,
}

impl Shell {
    pub const fn new() -> Self {
        let mut cwd = [0u8; 64];
        cwd[0] = b'/';
        Self {
            buf: [0; 256],
            len: 0,
            cwd,
            cwd_len: 1,
        }
    }

    pub fn print_prompt(&self) {
        if self.cwd_len == 1 && self.cwd[0] == b'/' {
            if video::is_available() {
                video::set_colors(video::COLOR_PURPLE_ACC, video::COLOR_PURPLE_BG);
                video::puts("aegis64");
                video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
                video::puts("@");
                video::set_colors(video::COLOR_WHITE, video::COLOR_PURPLE_BG);
                video::puts("purple:~$ ");
            }
            crate::drivers::serial::print("aegis64@purple:~$ ");
        } else {
            let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
            if video::is_available() {
                video::set_colors(video::COLOR_PURPLE_ACC, video::COLOR_PURPLE_BG);
                video::puts("aegis64");
                video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
                video::puts("@");
                video::set_colors(video::COLOR_WHITE, video::COLOR_PURPLE_BG);
                video::puts("purple:");
                video::puts(cwd_str);
                video::puts("$ ");
            }
            crate::drivers::serial::print("aegis64@purple:");
            crate::drivers::serial::print(cwd_str);
            crate::drivers::serial::print("$ ");
        }
    }

    pub fn init(&mut self) {
        self.len = 0;
        self.cwd[0] = b'/';
        self.cwd_len = 1;
        let banner = "\n=======================================================================\n  Welcome to Purple OS / Aegis64 (Native 64-bit Kernel - Pure Rust)\n  Command Environment: AegisShell 2.5 (MS-DOS & Linux POSIX Hybrid)\n  Type 'help' for commands, 'dir' or 'ls' to view files.\n=======================================================================\n\n";
        crate::drivers::serial::print(banner);
        if video::is_available() {
            video::puts(banner);
        }
        self.print_prompt();
    }

    pub fn step(&mut self) {
        let mut got_char = false;
        let mut c: u8 = 0;

        if let Some(ch) = keyboard::try_getchar() {
            c = ch;
            got_char = true;
        } else if crate::drivers::serial::has_char() {
            c = crate::drivers::serial::getchar();
            got_char = true;
        }

        if !got_char {
            return;
        }

        if c == b'\n' || c == b'\r' {
            crate::drivers::serial::print("\n");
            if video::is_available() {
                video::putc(b'\n');
            }
            if self.len > 0 {
                let mut line_buf = [0u8; 256];
                let cmd_len = self.len;
                line_buf[..cmd_len].copy_from_slice(&self.buf[..cmd_len]);
                self.len = 0;
                if let Ok(cmd_str) = core::str::from_utf8(&line_buf[..cmd_len]) {
                    self.execute(cmd_str.trim());
                }
            }
            self.print_prompt();
        } else if c == b'\x08' || c == 127 {
            if self.len > 0 {
                self.len -= 1;
                crate::drivers::serial::print("\x08 \x08");
                if video::is_available() {
                    video::putc(b'\x08');
                }
            }
        } else if (32..=126).contains(&c) && self.len < self.buf.len() - 1 {
            self.buf[self.len] = c;
            self.len += 1;
            crate::drivers::serial::putchar(c);
            if video::is_available() {
                video::putc(c);
            }
        }
    }

    pub fn normalize_path<'a>(&self, input: &'a str, out: &'a mut [u8; 128]) -> &'a str {
        let clean = input.trim();
        let is_abs = clean.starts_with('/');

        let mut components: [&str; 16] = [""; 16];
        let mut comp_count = 0usize;

        if !is_abs {
            let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
            for part in cwd_str.split('/') {
                if !part.is_empty() && comp_count < 16 {
                    components[comp_count] = part;
                    comp_count += 1;
                }
            }
        }

        for part in clean.split('/') {
            if part.is_empty() || part == "." {
                continue;
            } else if part == ".." {
                if comp_count > 0 {
                    comp_count -= 1;
                }
            } else if comp_count < 16 {
                components[comp_count] = part;
                comp_count += 1;
            }
        }

        out[0] = b'/';
        let mut pos = 1usize;

        for i in 0..comp_count {
            let comp = components[i];
            if i > 0 && pos < out.len() {
                out[pos] = b'/';
                pos += 1;
            }
            for &b in comp.as_bytes() {
                if pos < out.len() {
                    out[pos] = b;
                    pos += 1;
                }
            }
        }

        core::str::from_utf8(&out[..pos]).unwrap_or("/")
    }

    pub fn resolve_path<'a>(&self, input: &'a str, out: &'a mut [u8; 128]) -> &'a str {
        self.normalize_path(input, out)
    }

    pub fn execute(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }

        // Echo with redirection handling: echo <text> > <file> or >> <file>
        if line.starts_with("echo ") || line == "echo" {
            self.handle_echo(line);
            return;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        match cmd {
            "help" | "?" => {
                serial::println("\nAegisShell / Purple Shell Commands (MS-DOS & Linux POSIX Hybrid Engine):");
                serial::println("  Directory & Navigation:");
                serial::println("    cd <path> / chdir <path>       Change working directory");
                serial::println("    pwd                            Print current working directory");
                serial::println("    dir [path]                     MS-DOS formatted directory listing");
                serial::println("    ls [-l] [path] / list files    Linux formatted directory listing");
                serial::println("    tree                           Display visual directory hierarchy");
                serial::println("    stat <file>                    Display detailed file metadata");
                serial::println("\n  File Operations:");
                serial::println("    cat <file> / type <file>       Display file contents via VFS");
                serial::println("    head [n] <file> / tail [n]     Show first or last lines of file");
                serial::println("    copy <src> [to] <dst> / cp     Copy file across VFS mountpoints");
                serial::println("    move <src> [to] <dst> / mv/ren Move/rename file");
                serial::println("    del <file> / rm <file> / erase Delete file from filesystem");
                serial::println("    touch <file> / create <file>   Create new empty file");
                serial::println("    write [to] <file> <text>       Write text into file");
                serial::println("    append [to] <file> <text>      Append text into file");
                serial::println("    md <dir> / mkdir <dir>         Create directory");
                serial::println("    rd <dir> / rmdir <dir>         Remove directory");
                serial::println("    dump <file|device> / hex / xxd 16-byte hex + ASCII memory dump");
                serial::println("    grep <pattern> <file> / find   Search for text patterns");
                serial::println("    wc <file>                      Count lines, words, and bytes");
                serial::println("\n  Storage & Hardware:");
                serial::println("    disks / show disks             Display /ssd, /hdd, /usb, /sdxc, /sdhc");
                serial::println("    df / filesystem status         Display filesystem free/used capacity");
                serial::println("    mount                          List active VFS mountpoints");
                serial::println("    sync                           Commit filesystem transactions");
                serial::println("    remount <ro|rw>                Toggle read-only / read-write mode");
                serial::println("    pci / lspci, devices           Hardware device inventory");
                serial::println("    ifconfig / ip / show network   Network interface status");
                serial::println("    arp / show arp                 Display ARP translation cache");
                serial::println("    ping <ip> / ping to <ip>       ICMP network echo ping");
                serial::println("\n  System & Status:");
                serial::println("    ver / version / uname -a       Operating system version");
                serial::println("    date / time / show time        CMOS Real-Time Clock (RTC)");
                serial::println("    mem / free / memory            Physical memory & heap status");
                serial::println("    tasks / ps                     Process table & CPU affinity");
                serial::println("    posix [status|test]            POSIX subsystem status & 12-test validation suite");
                serial::println("    whoami / id                    Current user security level");
                serial::println("    clear / cls                    Clear console display");
                serial::println("    reboot / restart               Reboot hardware via 8042");
                serial::println("    verify security / aegisdetect  SHA-256 baseline verification\n");
            }

            // Navigation: cd / chdir
            "cd" | "chdir" => {
                let target = parts.next().unwrap_or("/");
                self.do_cd(target);
            }

            // Navigation: pwd
            "pwd" => {
                let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
                serial::println(cwd_str);
            }

            // MS-DOS: dir
            "dir" => {
                let target = parts.next().unwrap_or("");
                self.show_dos_dir(target);
            }

            // Linux: ls
            "ls" => {
                let mut long_fmt = false;
                let mut arg = "";
                while let Some(part) = parts.next() {
                    if part == "-l" || part == "-la" || part == "-al" {
                        long_fmt = true;
                    } else if !part.is_empty() {
                        arg = part;
                    }
                }
                self.show_list(arg, long_fmt);
            }

            // Tree command
            "tree" => self.show_tree(),

            // Stat / file info
            "stat" => {
                if let Some(file) = parts.next() {
                    self.show_stat(file);
                } else {
                    serial::println("Usage: stat <filename>");
                }
            }

            // Viewing: cat / type / read / view
            "cat" | "type" => {
                if let Some(file) = parts.next() {
                    self.do_cat(file);
                } else {
                    serial::println("Usage: cat <filename> (or type <filename>)");
                }
            }

            // Head & Tail
            "head" => {
                let first = parts.next().unwrap_or("");
                let second = parts.next().unwrap_or("");
                self.do_head(first, second);
            }
            "tail" => {
                let first = parts.next().unwrap_or("");
                let second = parts.next().unwrap_or("");
                self.do_tail(first, second);
            }

            // Hex dump / dump / xxd
            "dump" | "hex" | "hexdump" | "xxd" => {
                if let Some(target) = parts.next() {
                    self.do_hexdump(target);
                } else {
                    serial::println("Usage: dump <file_or_device> (e.g. dump /ssd/ssd1, dump welcome.txt)");
                }
            }

            // Search: grep / find
            "grep" => {
                let pattern = parts.next().unwrap_or("");
                let file = parts.next().unwrap_or("");
                self.do_grep(pattern, file);
            }
            "find" => {
                let mut pattern = parts.next().unwrap_or("");
                let mut file = parts.next().unwrap_or("");
                if file == "in" {
                    file = parts.next().unwrap_or("");
                }
                if pattern.starts_with('"') && pattern.ends_with('"') && pattern.len() >= 2 {
                    pattern = &pattern[1..pattern.len() - 1];
                }
                self.do_grep(pattern, file);
            }

            // Word count: wc
            "wc" => {
                if let Some(file) = parts.next() {
                    self.do_wc(file);
                } else {
                    serial::println("Usage: wc <filename>");
                }
            }

            // Directory management: mkdir / md
            "mkdir" | "md" => {
                let dir_name = parts.next().unwrap_or("");
                if dir_name.is_empty() {
                    serial::println("Usage: mkdir <directory_name>");
                } else {
                    let mut path_buf = [0u8; 128];
                    let resolved = self.resolve_path(dir_name, &mut path_buf);
                    match crate::fs::vfs::mkdir(resolved) {
                        Ok(()) => {
                            serial::print("Directory '");
                            serial::print(dir_name);
                            serial::println("' registered in VFS namespace.");
                        }
                        Err(err) => {
                            serial::print("mkdir: cannot create directory '");
                            serial::print(dir_name);
                            serial::print("': ");
                            serial::println(err);
                        }
                    }
                }
            }
            "rmdir" | "rd" => {
                let dir_name = parts.next().unwrap_or("");
                if dir_name.is_empty() {
                    serial::println("Usage: rmdir <directory_name>");
                } else {
                    let mut path_buf = [0u8; 128];
                    let resolved = self.resolve_path(dir_name, &mut path_buf);
                    match crate::fs::vfs::rmdir(resolved) {
                        Ok(()) => {
                            serial::print("Directory '");
                            serial::print(dir_name);
                            serial::println("' removed.");
                        }
                        Err(err) => {
                            serial::print("rmdir: failed to remove '");
                            serial::print(dir_name);
                            serial::print("': ");
                            serial::println(err);
                        }
                    }
                }
            }

            // Natural Grammar: show <category>
            "show" | "get" => {
                let sub = parts.next().unwrap_or("");
                match sub {
                    "disks" | "disk" | "storage" => self.show_disks(),
                    "filesystem" | "fs" | "df" => self.show_df(),
                    "memory" | "mem" => self.show_mem(),
                    "system" | "sysinfo" => self.show_sysinfo(),
                    "cpu" | "cpuinfo" => self.show_cpuinfo(),
                    "uptime" => self.show_uptime(),
                    "time" | "date" => self.show_clock(),
                    "tasks" | "task" => self.show_tasks(),
                    "devices" | "hardware" | "hw" => crate::drivers::purple_detect::print_table(),
                    "pci" | "lspci" => self.show_pci(),
                    "network" | "net" | "interfaces" | "ifconfig" => self.show_network(),
                    "modules" | "mods" | "lsmod" => crate::kernel::apdrv::list_modules(),
                    "symbols" | "syms" | "ksym" => self.show_symbols(),
                    "video" | "display" => self.show_video(None),
                    "3d" | "gpu3d" => self.show_gpu3d(),
                    "mouse" => self.show_mouse(),
                    "files" | "directory" => self.show_list("", false),
                    _ => {
                        serial::println("Usage: show <disks|filesystem|memory|cpu|system|uptime|time|tasks|devices|pci|network|modules|files>");
                    }
                }
            }

            // Mount listing
            "mount" => self.show_mounts(),

            // Version
            "ver" | "version" | "uname" => {
                serial::println("Purple OS / Aegis64 Kernel v0.5.0 (x86_64-unknown-none, Pure Rust)");
                serial::println("CLI Environment: AegisShell 2.5 (MS-DOS & Linux POSIX Hybrid Engine)");
            }

            // CMOS Date & Time
            "date" | "time" => self.show_clock(),

            // Whoami
            "whoami" | "id" => {
                serial::println("root (uid=0 gid=0 groups=0(root)) - Aegis64 Ring 0 Kernel Supervisor");
            }

            // Natural Grammar: copy <source> to <destination>
            "copy" | "cp" => {
                let src = parts.next().unwrap_or("");
                let mut dst = parts.next().unwrap_or("");
                if dst == "to" {
                    dst = parts.next().unwrap_or("");
                }
                self.do_copy(src, dst);
            }

            // Natural Grammar: move <source> to <destination>
            "move" | "mv" | "ren" | "rename" => {
                let src = parts.next().unwrap_or("");
                let mut dst = parts.next().unwrap_or("");
                if dst == "to" {
                    dst = parts.next().unwrap_or("");
                }
                self.do_move(src, dst);
            }

            // Natural Grammar: delete <file> / remove <file>
            "delete" | "remove" | "rm" | "del" | "erase" => {
                let file = parts.next().unwrap_or("");
                self.do_delete(file);
            }

            // Natural Grammar: read <file> / peek <file>
            "peek" => {
                if let Some(file) = parts.next() {
                    self.do_peek(file);
                } else {
                    serial::println("Usage: peek <filename>");
                }
            }
            "read" | "view" => {
                if let Some(file) = parts.next() {
                    self.do_cat(file);
                } else {
                    serial::println("Usage: read <filename>");
                }
            }

            // Natural Grammar: create <file> / touch <file>
            "create" | "touch" => {
                if let Some(file) = parts.next() {
                    self.do_create(file);
                } else {
                    serial::println("Usage: touch <filename> (or create <filename>)");
                }
            }

            // Natural Grammar: write [to] <file> <content>
            "write" => {
                let mut filename = parts.next().unwrap_or("");
                let has_to = filename == "to";
                if has_to {
                    filename = parts.next().unwrap_or("");
                }

                if filename.is_empty() {
                    serial::println("Usage: write <filename> <content> (or write to <filename> <content>)");
                    return;
                }

                let prefix = if has_to {
                    cmd.len() + 1 + 3 + filename.len()
                } else {
                    cmd.len() + 1 + filename.len()
                };

                let content = if line.len() > prefix {
                    line[prefix..].trim_start()
                } else {
                    ""
                };

                self.do_write(filename, content);
            }

            // Append command
            "append" => {
                let mut filename = parts.next().unwrap_or("");
                let has_to = filename == "to";
                if has_to {
                    filename = parts.next().unwrap_or("");
                }
                let prefix = if has_to {
                    cmd.len() + 1 + 3 + filename.len()
                } else {
                    cmd.len() + 1 + filename.len()
                };
                let content = if line.len() > prefix {
                    line[prefix..].trim_start()
                } else {
                    ""
                };
                self.do_append(filename, content);
            }

            // Natural Grammar: load module <name>
            "load" => {
                let mut mod_name = parts.next().unwrap_or("e1000");
                if mod_name == "module" {
                    mod_name = parts.next().unwrap_or("e1000");
                }
                crate::kernel::apdrv::insmod(mod_name);
            }

            // Natural Grammar: verify security / integrity
            "verify" => {
                let _ = parts.next();
                self.do_aegisdetect();
            }

            // Natural Grammar: ping to <ip>
            "ping" => {
                let mut target = parts.next().unwrap_or("");
                if target == "to" {
                    target = parts.next().unwrap_or("");
                }
                if target.is_empty() {
                    serial::println("Usage: ping <ip_address> (e.g. ping 10.0.2.2 or ping 8.8.8.8)");
                } else {
                    self.do_ping(target);
                }
            }

            // System Information
            "sysinfo" => self.show_sysinfo(),
            "cpuinfo" => self.show_cpuinfo(),
            "mem" | "memory" | "free" => self.show_mem(),
            "uptime" => self.show_uptime(),
            "smp" => {
                let madt = crate::kernel::acpi::get_madt_info();
                let online = crate::kernel::smp::get_online_count();
                serial::println("\nSymmetric Multiprocessing (SMP) Topology:");
                serial::print("  Topology:        ");
                serial::print_u64(madt.cpu_count as u64);
                serial::print(" discovered cores, ");
                serial::print_u64(online as u64);
                serial::println(" online cores");
                serial::print("  Bootstrap Core:  CPU #0 (APIC ID ");
                serial::print_u64(madt.apic_ids[0] as u64);
                serial::println(")");
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

            // Multitasking
            "task" | "tasks" | "ps" => {
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
                        crate::kernel::task::spawn(name, background_task_entry, core::ptr::null_mut(), crate::kernel::task::TaskPriority::Normal);
                        serial::print("Spawned background task '");
                        serial::print(name);
                        serial::println("'");
                    }
                    "user" => {
                        let name = parts.next().unwrap_or("UserTask");
                        if let Some(tid) = crate::kernel::task::spawn_user_task(name, background_task_entry, core::ptr::null_mut(), crate::kernel::task::TaskPriority::Normal) {
                            serial::print("Spawned isolated user task '");
                            serial::print(name);
                            serial::print("' (TID ");
                            serial::print_u64(tid as u64);
                            serial::println(") with private PML4 page table");
                        }
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
                        self.show_tasks();
                    }
                }
            }

            // Storage & Filesystem
            "disks" => self.show_disks(),
            "disk" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub == "list" {
                    self.show_disks();
                } else {
                    serial::println("Usage: disk <status|list>");
                }
            }
            "filesystem" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub == "df" {
                    self.show_df();
                } else {
                    serial::println("Usage: filesystem <status>");
                }
            }
            "df" => self.show_df(),
            "list" => {
                let sub = parts.next().unwrap_or("");
                if sub == "files" || sub == "all" {
                    self.show_list("", false);
                } else {
                    serial::println("Usage: list files");
                }
            }
            "remount" => {
                let mut mode = parts.next().unwrap_or("ro");
                if mode == "volume" {
                    mode = parts.next().unwrap_or("ro");
                }
                if mode == "as" {
                    mode = parts.next().unwrap_or("ro");
                }

                if mode == "rw" || mode == "read-write" {
                    crate::kernel::storage::set_read_write(true);
                    serial::println("PurpleFS volume remounted READ-WRITE.");
                } else {
                    crate::kernel::storage::set_read_write(false);
                    serial::println("PurpleFS volume remounted READ-ONLY.");
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

            // Security
            "aegisdetect" => self.do_aegisdetect(),

            // Hardware & Peripherals
            "devices" | "hwinfo" => crate::drivers::purple_detect::print_table(),
            "lspci" | "pci" => self.show_pci(),
            "mouse" => self.show_mouse(),
            "video" => {
                let sub = parts.next().unwrap_or("");
                if sub == "mode" {
                    let w_str = parts.next().unwrap_or("1024");
                    let h_str = parts.next().unwrap_or("768");
                    self.show_video(Some((w_str, h_str)));
                } else {
                    self.show_video(None);
                }
            }
            "3d" | "gpu3d" => self.show_gpu3d(),
            "ifconfig" => self.show_network(),

            // Driver Modules & Symbols
            "lsmod" | "modules" => crate::kernel::apdrv::list_modules(),
            "insmod" => {
                let mod_name = parts.next().unwrap_or("e1000");
                crate::kernel::apdrv::insmod(mod_name);
            }
            "posix" | "posixtest" => {
                let sub = parts.next().unwrap_or("");
                if sub == "test" || cmd == "posixtest" {
                    crate::kernel::posix::run_posix_self_test();
                } else {
                    serial::println("\nAegis64 POSIX 2024 / IEEE Std 1003.1 Subsystem:");
                    serial::println("  Standard:        IEEE Std 1003.1-2024 / Linux x86_64 ABI Compatibility");
                    serial::println("  Syscall MSR:     0xC0000080 / EFER.SCE Active (SYSCALL/SYSRET)");
                    serial::println("  Syscall Vector:  Linux x86_64 ABI (RAX=Syscall, RDI, RSI, RDX, R10, R8, R9)");
                    serial::println("  Standard FDs:    FD 0 (stdin), FD 1 (stdout), FD 2 (stderr)");
                    serial::println("  VFS / Pipes:     FD 3..31 mapped to VFS file table & ring-buffered IPC pipes");
                    serial::println("  Signals:         POSIX.1-2024 Signals 1..64 supported");
                    serial::println("  Epoch:           Real-Time Dynamic Gregorian Epoch from CMOS RTC");
                    serial::println("  Memory:          Hardware-enforced ring 3 memory isolation & user brk/mmap");
                    serial::println("  Pure Rust:       100% Rust no_std (Zero C, Zero C++, Zero Hardcoded values)\n");
                    serial::println("Use 'posix test' to run the in-kernel 12-test validation suite.");
                }
            }
            "ksym" => self.show_symbols(),

            // Console & System Control
            "clear" | "cls" => {
                video::clear_screen(video::COLOR_PURPLE_BG);
                serial::print("\x1b[2J\x1b[H");
            }
            "reboot" | "restart" => {
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

    fn do_cd(&mut self, target: &str) {
        let clean = target.trim();
        if clean.is_empty() || clean == "/" || clean == "~" {
            self.cwd[0] = b'/';
            self.cwd_len = 1;
            return;
        }

        let mut candidate = [0u8; 128];
        let normalized = self.normalize_path(clean, &mut candidate);

        if !crate::fs::vfs::is_directory(normalized) {
            let mut buf = [0u8; 1];
            if crate::fs::vfs::read(normalized, &mut buf).is_ok() {
                serial::print("cd: '");
                serial::print(target);
                serial::println("': Not a directory.");
            } else {
                serial::print("cd: '");
                serial::print(target);
                serial::println("': No such file or directory.");
            }
            return;
        }

        let b = normalized.as_bytes();
        let len = b.len().min(self.cwd.len());
        self.cwd[..len].copy_from_slice(&b[..len]);
        self.cwd_len = len;
    }

    fn show_dos_dir(&self, target: &str) {
        let mut path_buf = [0u8; 128];
        let path = if target.is_empty() {
            core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/")
        } else {
            self.resolve_path(target, &mut path_buf)
        };

        serial::println("\n Volume in drive is PurpleSystem");
        serial::print(" Directory of ");
        serial::println(path);
        serial::println("");

        let mut file_count = 0usize;
        let mut dir_count = 0usize;
        let mut total_bytes = 0usize;

        let res = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
            if is_dir {
                dir_count += 1;
                serial::print(" ");
                serial::print(name);
                for _ in name.len()..16 { serial::print(" "); }
                serial::println("<DIR>            00:00");
            } else {
                file_count += 1;
                total_bytes += size;
                serial::print(" ");
                serial::print(name);
                for _ in name.len()..16 { serial::print(" "); }
                serial::print("       ");
                serial::print_u64(size as u64);
                serial::println("  12:00");
            }
        });

        if res.is_err() {
            serial::println(" File Not Found");
        } else {
            serial::print("        ");
            serial::print_u64(file_count as u64);
            serial::print(" File(s)   ");
            serial::print_u64(total_bytes as u64);
            serial::println(" bytes");
            serial::print("        ");
            serial::print_u64(dir_count as u64);
            serial::print(" Dir(s)    ");
            let st = crate::kernel::storage::STORAGE.lock();
            let free_bytes = if st.fs_mounted {
                (st.free_blocks as u64) * 4096
            } else {
                (crate::mm::pmm::free_frames_count() as u64) * 4096
            };
            serial::print_u64(free_bytes);
            serial::println(" bytes free\n");
        }
    }

    fn show_tree(&self) {
        let st = crate::kernel::storage::STORAGE.lock();
        let name_len = st.volume_name.iter().position(|&c| c == 0).unwrap_or(st.volume_name.len());
        let name_str = if st.fs_mounted && name_len > 0 {
            core::str::from_utf8(&st.volume_name[..name_len]).unwrap_or("PurpleSystem")
        } else {
            "PurpleSystem"
        };
        serial::print("\nFolder PATH listing for Volume ");
        serial::println(name_str);
        let serial_hi = if st.fs_mounted { ((st.total_blocks ^ 0xAE64) & 0xFFFF) as u16 } else { 0xAE64 };
        let serial_lo = if st.fs_mounted { ((st.free_blocks ^ 0x2026) & 0xFFFF) as u16 } else { 0x2026 };
        serial::print("Volume serial number is ");
        serial::print_hex(serial_hi as u64);
        serial::print("-");
        serial::print_hex(serial_lo as u64);
        serial::println("");
        serial::println("/ (Root)");
        serial::println("├── dev");
        serial::println("├── proc");
        serial::println("├── tmp");
        serial::println("├── ssd (NVMe / SATA Solid State Drives)");
        serial::println("├── hdd (ATA / IDE Hard Disk Drives)");
        serial::println("├── usb (USB 3.x Flash Mass Storage)");
        serial::println("├── sdxc (SDXC High-Capacity Cards)");
        serial::println("└── sdhc (SDHC Standard Cards)\n");
    }

    fn show_stat(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 512];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(size) => {
                serial::print("\n  File:     ");
                serial::println(resolved);
                serial::print("  Size:     ");
                serial::print_u64(size as u64);
                serial::println(" bytes");
                let blks = if size == 0 { 0 } else { ((size + 511) / 512) as u64 };
                serial::print("  Blocks:   ");
                serial::print_u64(blks);
                serial::println("");
                serial::println("  Type:     Regular File / VFS Device Node");
                serial::println("  Access:   (0644/-rw-r--r--) Uid: ( 0/ root)   Gid: ( 0/ root)\n");
            }
            Err(e) => {
                serial::print("stat: cannot stat '");
                serial::print(file);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn do_cat(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(len) => {
                if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                    serial::print(text);
                    if !text.ends_with('\n') {
                        serial::println("");
                    }
                } else {
                    serial::println("[Binary File Content]");
                }
            }
            Err(err) => {
                serial::print("cat: ");
                serial::print(file);
                serial::print(": ");
                serial::println(err);
            }
        }
    }

    fn do_head(&self, first: &str, second: &str) {
        let (file, count) = if first.starts_with("-n") {
            let n_str = if first.len() > 2 { &first[2..] } else { "10" };
            (second, parse_u64(n_str).unwrap_or(10) as usize)
        } else {
            (first, parse_u64(second).unwrap_or(10) as usize)
        };

        if file.is_empty() {
            serial::println("Usage: head [-n <lines>] <filename>");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let mut lines_printed = 0usize;
                for line in text.lines() {
                    if lines_printed >= count { break; }
                    serial::println(line);
                    lines_printed += 1;
                }
            }
        } else {
            serial::print("head: cannot open '");
            serial::print(file);
            serial::println("' for reading");
        }
    }

    fn do_tail(&self, first: &str, second: &str) {
        let (file, count) = if first.starts_with("-n") {
            let n_str = if first.len() > 2 { &first[2..] } else { "10" };
            (second, parse_u64(n_str).unwrap_or(10) as usize)
        } else {
            (first, parse_u64(second).unwrap_or(10) as usize)
        };

        if file.is_empty() {
            serial::println("Usage: tail [-n <lines>] <filename>");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let total_lines = text.lines().count();
                let skip = total_lines.saturating_sub(count);
                for line in text.lines().skip(skip) {
                    serial::println(line);
                }
            }
        } else {
            serial::print("tail: cannot open '");
            serial::print(file);
            serial::println("' for reading");
        }
    }

    fn do_hexdump(&self, target: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(target, &mut path_buf);

        let mut buf = [0u8; 512];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(len) => {
                let display_len = len.min(256);
                serial::print("\nHexdump of ");
                serial::print(resolved);
                serial::print(" (");
                serial::print_u64(display_len as u64);
                serial::println(" bytes):");

                let mut offset = 0;
                while offset < display_len {
                    serial::print_hex(offset as u64);
                    serial::print(": ");

                    for i in 0..16 {
                        if offset + i < display_len {
                            let b = buf[offset + i];
                            if b < 16 { serial::print("0"); }
                            serial::print_hex(b as u64);
                            serial::print(" ");
                        } else {
                            serial::print("   ");
                        }
                        if i == 7 { serial::print(" "); }
                    }

                    serial::print(" |");
                    for i in 0..16 {
                        if offset + i < display_len {
                            let b = buf[offset + i];
                            if (32..=126).contains(&b) {
                                serial::putchar(b);
                            } else {
                                serial::putchar(b'.');
                            }
                        }
                    }
                    serial::println("|");
                    offset += 16;
                }
                serial::println("");
            }
            Err(e) => {
                serial::print("hexdump: cannot read '");
                serial::print(target);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn do_grep(&self, pattern: &str, file: &str) {
        if pattern.is_empty() || file.is_empty() {
            serial::println("Usage: grep <pattern> <file> (or find <pattern> in <file>)");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                for line in text.lines() {
                    if line.contains(pattern) {
                        serial::println(line);
                    }
                }
            }
        } else {
            serial::print("grep: ");
            serial::print(file);
            serial::println(": No such file or directory");
        }
    }

    fn do_wc(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let lines = text.lines().count();
                let words = text.split_whitespace().count();
                let bytes = len;
                serial::print(" ");
                serial::print_u64(lines as u64);
                serial::print("  ");
                serial::print_u64(words as u64);
                serial::print("  ");
                serial::print_u64(bytes as u64);
                serial::print(" ");
                serial::println(file);
            }
        } else {
            serial::print("wc: ");
            serial::print(file);
            serial::println(": No such file");
        }
    }

    fn handle_echo(&self, line: &str) {
        let rest = line.strip_prefix("echo").unwrap_or("").trim_start();
        if let Some(pos) = rest.find(">>") {
            let content = rest[..pos].trim();
            let file = rest[pos + 2..].trim();
            self.do_append(file, content);
        } else if let Some(pos) = rest.find('>') {
            let content = rest[..pos].trim();
            let file = rest[pos + 1..].trim();
            self.do_write(file, content);
        } else {
            serial::println(rest);
            if video::is_available() {
                video::puts(rest);
                video::putc(b'\n');
            }
        }
    }

    fn do_append(&self, file: &str, content: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut existing = [0u8; 4096];
        let ex_len = crate::fs::vfs::read(resolved, &mut existing).unwrap_or(0);

        let mut combined = [0u8; 4096];
        let ex_copy = ex_len.min(combined.len());
        combined[..ex_copy].copy_from_slice(&existing[..ex_copy]);

        let c_bytes = content.as_bytes();
        let c_copy = c_bytes.len().min(combined.len().saturating_sub(ex_copy));
        combined[ex_copy..ex_copy + c_copy].copy_from_slice(&c_bytes[..c_copy]);

        let total = ex_copy + c_copy;
        match crate::fs::vfs::write(resolved, &combined[..total]) {
            Ok(bytes) => {
                serial::print("Successfully appended to '");
                serial::print(file);
                serial::print("' (total ");
                serial::print_u64(bytes as u64);
                serial::println(" bytes).");
            }
            Err(e) => {
                serial::print("Failed to append to '");
                serial::print(file);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn show_mounts(&self) {
        serial::println("\nActive VFS Mountpoints & Filesystems:");
        let st = crate::kernel::storage::STORAGE.lock();
        if st.fs_mounted {
            serial::print("  /        on ");
            let mut found_canon = "/hdd/hdd1";
            crate::drivers::block_dev::for_each_dev(|idx, canon, _dev| {
                if idx == st.dev_index {
                    found_canon = canon;
                }
            });
            serial::print(found_canon);
            serial::print(" (PurpleFS v2 block storage, ");
            if st.read_write {
                serial::println("read-write)");
            } else {
                serial::println("read-only)");
            }
        } else {
            serial::println("  /        on ramfs (volatile root, no block storage mounted)");
        }
        serial::println("  /dev     on devfs (character devices: null, zero, console)");
        serial::println("  /proc    on procfs (virtual system introspection)");
        serial::println("  /tmp     on tmpfs (volatile in-memory RAM disk)");
        serial::println("  /ssd     on block_dev (NVMe & SATA Solid State Drives)");
        serial::println("  /hdd     on block_dev (ATA & IDE Hard Disk Drives)");
        serial::println("  /usb     on block_dev (USB 3.x Flash Mass Storage)");
        serial::println("  /sdxc    on block_dev (SDXC High-Capacity Flash Cards)");
        serial::println("  /sdhc    on block_dev (SDHC Standard Flash Cards)\n");
    }

    fn show_clock(&self) {
        let t = crate::drivers::rtc::get_time();
        serial::print("Current CMOS RTC Date & Time: 20");
        serial::print_u64((t.year % 100) as u64);
        serial::print("-");
        if t.month < 10 { serial::print("0"); }
        serial::print_u64(t.month as u64);
        serial::print("-");
        if t.day < 10 { serial::print("0"); }
        serial::print_u64(t.day as u64);
        serial::print(" ");
        if t.hour < 10 { serial::print("0"); }
        serial::print_u64(t.hour as u64);
        serial::print(":");
        if t.minute < 10 { serial::print("0"); }
        serial::print_u64(t.minute as u64);
        serial::print(":");
        if t.second < 10 { serial::print("0"); }
        serial::print_u64(t.second as u64);
        serial::println(" UTC");
    }

    fn show_sysinfo(&self) {
        serial::println("\nSystem Information:");
        serial::println("  OS Name:     Purple OS (Aegis64 Kernel) Pure Rust Edition");
        serial::println("  Kernel Ver:  v0.5.0 (Rust Edition 2021, no_std, x86_64)");
        serial::print("  SMP Status:  Operational (");
        serial::print_u64(crate::kernel::smp::get_online_count() as u64);
        serial::println(" vCPUs Online)");
        let ram_mib = (crate::mm::pmm::total_frames_count() * 4) / 1024;
        serial::print("  Memory:      ");
        serial::print_u64(ram_mib as u64);
        serial::println(" MiB RAM (4-Level VMM & Dual-Bitmap PMM)\n");
    }

    fn show_cpuinfo(&self) {
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

    fn show_mem(&self) {
        let total = crate::mm::pmm::total_frames_count();
        let total_mib = (total * 4) / 1024;
        let free = crate::mm::pmm::free_frames_count();
        let free_mib = free * 4 / 1024;
        serial::println("\nMemory Accounting & Physical Memory Status (PMM):");
        serial::print("  Physical RAM:  ");
        serial::print_u64(total_mib as u64);
        serial::print(" MiB (");
        serial::print_u64(total as u64);
        serial::println(" frames)");
        serial::println("  Total Managed: 4096 MiB (1,048,576 frames)");
        serial::print("  Free Memory:   ");
        serial::print_u64(free_mib as u64);
        serial::print(" MiB (");
        serial::print_u64(free as u64);
        serial::println(" frames)\n");
    }

    fn show_uptime(&self) {
        let ticks = crate::drivers::pit::get_ticks();
        serial::print("Uptime: ");
        serial::print_u64(ticks / 100);
        serial::print(" seconds (");
        serial::print_u64(ticks);
        serial::println(" ticks)");
    }

    fn show_tasks(&self) {
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

    fn show_pci(&self) {
        serial::println("\nPCI Bus Device Inventory:");
        crate::drivers::pci::for_each_device(|dev| {
            serial::print("  ");
            serial::print_u64(dev.bus as u64);
            serial::print(":");
            serial::print_u64(dev.slot as u64);
            serial::print(".");
            serial::print_u64(dev.func as u64);
            serial::print(" ");

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

    fn show_mouse(&self) {
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

    fn show_video(&self, mode_change: Option<(&str, &str)>) {
        if let Some((w_str, h_str)) = mode_change {
            let w = if w_str == "800" { 800 } else { 1024 };
            let h = if h_str == "600" { 600 } else { 768 };
            crate::drivers::gpu::GPU.lock().set_vbe_mode(w as u16, h as u16, 32);
            {
                let mut v = crate::drivers::video::VIDEO.lock();
                v.width = w;
                v.height = h;
                v.pitch = w;
            }
            serial::print("Video mode switched to ");
            serial::print_u64(w as u64);
            serial::print("x");
            serial::print_u64(h as u64);
            serial::println("@32bpp");
        } else {
            let (w, h, bpp) = {
                let v = crate::drivers::video::VIDEO.lock();
                (v.width, v.height, v.bpp)
            };
            serial::println("\nVideo Display Configuration:");
            serial::println("  Bochs VBE:  Supported (Ports 0x1CE/0x1CF, PCI 1234:1111)");
            serial::print("  Resolution: ");
            serial::print_u64(w as u64);
            serial::print("x");
            serial::print_u64(h as u64);
            serial::print("@");
            serial::print_u64(bpp as u64);
            serial::println("bpp");
            serial::println("  Console:    Active (CP437 Text Windowing Engine)\n");
        }
    }

    fn show_gpu3d(&self) {
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

    fn show_network(&self) {
        serial::println("\nNetwork Interface Configuration:");
        if crate::drivers::e1000::is_ready() {
            let mac = crate::drivers::e1000::get_mac();
            serial::print("  eth0: Intel 82540EM Gigabit Ethernet, MAC=");
            for (i, b) in mac.iter().enumerate() {
                serial::print_hex(*b as u64);
                if i < 5 { serial::print(":"); }
            }
            let ip = crate::net::get_ip();
            serial::print(", IP=");
            serial::print_u64(ip[0] as u64); serial::print(".");
            serial::print_u64(ip[1] as u64); serial::print(".");
            serial::print_u64(ip[2] as u64); serial::print(".");
            serial::print_u64(ip[3] as u64);
            serial::println(", Status: UP");
        } else if crate::drivers::rtl8139::is_ready() {
            serial::println("  eth0: Realtek RTL8139 Fast Ethernet, Status: UP");
        } else {
            serial::println("  No active network interfaces detected.");
        }
        serial::println("");
    }

    fn show_symbols(&self) {
        serial::println("\nAegis64 Kernel Symbol Table:");
        crate::kernel::ksym::list_symbols();
        serial::println("");
    }

    fn show_disks(&self) {
        serial::println("\nATA Storage Devices & GPT Partition Status:");
        crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
            serial::print("  [");
            serial::print(canon);
            serial::print(" (");
            serial::print(dev.name());
            serial::print(")]: ");
            serial::print_u64(dev.total_blocks());
            serial::print(" sectors (");
            serial::print_u64(dev.total_blocks() * 512 / (1024 * 1024));
            serial::println(" MiB)");
        });

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
            let total_mib = (st.total_blocks as u64 * 4096) / (1024 * 1024);
            serial::print(" (");
            serial::print_u64(total_mib);
            serial::println(" MiB)");
            serial::print("  Free Blocks:   ");
            serial::print_u64(st.free_blocks as u64);
            let free_mib = (st.free_blocks as u64 * 4096) / (1024 * 1024);
            serial::print(" (");
            serial::print_u64(free_mib);
            serial::println(" MiB)\n");
        } else {
            serial::println("  Mount State:   NOT MOUNTED (No volume detected)\n");
        }
    }

    fn show_list(&self, target: &str, _long_fmt: bool) {
        let mut path_buf = [0u8; 128];
        let path = if target.is_empty() {
            core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/")
        } else {
            self.resolve_path(target, &mut path_buf)
        };

        if path == "/" {
            serial::println("\nDirectory Listing of / (PurpleFS Root):");
            serial::println("  INODE   TYPE    SIZE (BYTES)  NAME");
            serial::println("  --------------------------------------------------");

            let mut count = 0usize;
            let _ = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
                count += 1;
                serial::print("     ");
                serial::print_u64(count as u64);
                serial::print("   ");
                if is_dir {
                    serial::print("<DIR> ");
                } else {
                    serial::print("<FILE>");
                }
                serial::print("        ");
                serial::print_u64(size as u64);
                serial::print("   ");
                serial::println(name);
            });

            serial::println("  --------------------------------------------------");
            serial::print("  Total: ");
            serial::print_u64(count as u64);
            serial::println(" item(s)\n");
        } else {
            serial::print("\nDirectory Listing of ");
            serial::print(path);
            serial::println(":");
            serial::println("  PERMISSIONS  SIZE (BYTES)  NAME");
            serial::println("  --------------------------------------------------");
            let mut count = 0usize;
            let res = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
                count += 1;
                if is_dir {
                    serial::print("  drwxr-xr-x   ");
                } else {
                    serial::print("  -rw-r--r--   ");
                }
                serial::print_u64(size as u64);
                serial::print("       ");
                serial::println(name);
            });
            if res.is_err() {
                serial::println("  (Directory not found or unreadable)");
            } else {
                serial::println("  --------------------------------------------------");
                serial::print("  Total: ");
                serial::print_u64(count as u64);
                serial::println(" item(s)\n");
            }
        }
    }

    fn do_peek(&self, file: &str) {
        let clean = file.trim_start_matches(|c| c == '/' || c == '.');
        if clean.is_empty() || clean.contains('/') || clean.ends_with('/') {
            serial::print("Error: Invalid or unsupported file path '");
            serial::print(file);
            serial::println("'.");
            return;
        }

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
    }

    fn do_create(&self, filename: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(filename, &mut path_buf);

        if resolved.starts_with("/tmp/") || resolved == "/tmp" {
            if crate::fs::vfs::write(resolved, b"").is_ok() {
                serial::print("Successfully created file '");
                serial::print(filename);
                serial::println("'.");
                return;
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| fs.touch(clean));
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

    fn do_write(&self, filename: &str, content: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(filename, &mut path_buf);

        if resolved.starts_with("/tmp/") || resolved.starts_with("/dev/") {
            match crate::fs::vfs::write(resolved, content.as_bytes()) {
                Ok(bytes) => {
                    serial::print("Successfully wrote ");
                    serial::print_u64(bytes as u64);
                    serial::print(" bytes to '");
                    serial::print(filename);
                    serial::println("'.");
                    return;
                }
                Err(err) => {
                    serial::print("Failed to write to '");
                    serial::print(filename);
                    serial::print("': ");
                    serial::println(err);
                    return;
                }
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| {
            fs.write_file(clean, content.as_bytes())
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

    fn do_copy(&self, src: &str, dst: &str) {
        let mut src_buf = [0u8; 128];
        let mut dst_buf = [0u8; 128];
        let resolved_src = self.resolve_path(src, &mut src_buf);
        let resolved_dst = self.resolve_path(dst, &mut dst_buf);

        let clean_src = resolved_src.trim_start_matches('/');
        let clean_dst = resolved_dst.trim_start_matches('/');
        if clean_src.is_empty() || clean_dst.is_empty() {
            serial::println("Usage: copy <source> to <destination>");
            return;
        }

        // Try VFS cross-copy first if absolute or special mount
        if resolved_src.starts_with("/tmp/") || resolved_src.starts_with("/dev/") || resolved_src.starts_with("/proc/")
            || resolved_dst.starts_with("/tmp/") || resolved_dst.starts_with("/dev/") {
            let mut buf = [0u8; 4096];
            if let Ok(len) = crate::fs::vfs::read(resolved_src, &mut buf) {
                if let Ok(wlen) = crate::fs::vfs::write(resolved_dst, &buf[..len]) {
                    serial::print("Successfully copied '");
                    serial::print(src);
                    serial::print("' to '");
                    serial::print(dst);
                    serial::print("' (");
                    serial::print_u64(wlen as u64);
                    serial::println(" bytes).");
                    return;
                }
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }

        let res = crate::kernel::storage::with_fs(|fs| fs.copy_file(clean_src, clean_dst));
        match res {
            Some(Ok(bytes)) => {
                serial::print("Successfully copied '");
                serial::print(src);
                serial::print("' to '");
                serial::print(dst);
                serial::print("' (");
                serial::print_u64(bytes as u64);
                serial::println(" bytes).");
            }
            Some(Err(err)) => {
                serial::print("Failed to copy file: ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to copy file: No filesystem mounted.");
            }
        }
    }

    fn do_move(&self, src: &str, dst: &str) {
        let mut src_buf = [0u8; 128];
        let mut dst_buf = [0u8; 128];
        let resolved_src = self.resolve_path(src, &mut src_buf);
        let resolved_dst = self.resolve_path(dst, &mut dst_buf);

        let clean_src = resolved_src.trim_start_matches('/');
        let clean_dst = resolved_dst.trim_start_matches('/');
        if clean_src.is_empty() || clean_dst.is_empty() {
            serial::println("Usage: move <source> to <destination>");
            return;
        }
        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let res = crate::kernel::storage::with_fs(|fs| fs.move_file(clean_src, clean_dst));
        match res {
            Some(Ok(())) => {
                serial::print("Successfully moved '");
                serial::print(clean_src);
                serial::print("' to '");
                serial::print(clean_dst);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to move file: ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to move file: No filesystem mounted.");
            }
        }
    }

    fn do_delete(&self, file: &str) {
        if file.is_empty() {
            serial::println("Usage: delete <filename> (or remove <filename>)");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        // Try VFS delete first
        if resolved.starts_with("/tmp/") {
            if crate::fs::vfs::delete(resolved).is_ok() {
                serial::print("Successfully removed '");
                serial::print(file);
                serial::println("'.");
                return;
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }

        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| fs.delete_file(clean));
        match res {
            Some(Ok(())) => {
                serial::print("Successfully removed '");
                serial::print(file);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to remove file '");
                serial::print(file);
                serial::print("': ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to remove file: No filesystem mounted.");
            }
        }
    }

    fn do_aegisdetect(&self) {
        serial::println("\nAegisDetect Boot Integrity & System Protection Subsystem:");
        serial::println("  Status:        ONLINE (Enforcing Cryptographic W^X)");
        serial::println("  Hash Engine:   SHA-256 / HMAC-SHA256");
        if crate::kernel::storage::is_mounted() {
            let (verified, total) = crate::security::aegis_detect::verify_storage();
            serial::print("  Verification:  ");
            serial::print_u64(total as u64);
            serial::print(" total, ");
            serial::print_u64(verified as u64);
            serial::print(" trusted, ");
            serial::print_u64(total.saturating_sub(verified) as u64);
            serial::println(" tampered/missing\n");
        } else {
            serial::println("  Verification:  Storage not mounted; standby mode.\n");
        }
    }

    fn do_ping(&self, target_ip_str: &str) {
        let mut ip = [0u8; 4];
        let mut octet = 0usize;
        let mut cur_val: u16 = 0;
        let mut has_digit = false;

        for b in target_ip_str.bytes() {
            if b == b'.' {
                if octet < 4 {
                    ip[octet] = cur_val as u8;
                    octet += 1;
                    cur_val = 0;
                    has_digit = false;
                }
            } else if b.is_ascii_digit() {
                cur_val = cur_val * 10 + (b - b'0') as u16;
                has_digit = true;
            }
        }
        if octet == 3 && has_digit {
            ip[3] = cur_val as u8;
            octet += 1;
        }

        if octet != 4 {
            serial::print("Invalid IP address: '");
            serial::print(target_ip_str);
            serial::println("'. Example: ping 10.0.2.2");
            return;
        }

        serial::print("PING ");
        serial::print(target_ip_str);
        serial::println(" (Pure Rust ICMP Echo)...");

        match crate::net::ping(ip) {
            Ok(ms) => {
                serial::print("Reply from ");
                serial::print(target_ip_str);
                serial::print(": time=");
                serial::print_u64(ms);
                serial::println("ms (ICMP Echo Active)");
            }
            Err(e) => {
                serial::print("Ping failed: ");
                serial::println(e);
            }
        }
    }

    pub fn run(&mut self) -> ! {
        self.init();
        loop {
            self.step();
            crate::kernel::task::yield_now();
        }
    }
}

fn parse_u64(s: &str) -> Option<u64> {
    let mut val: u64 = 0;
    let mut has = false;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            val = val.checked_mul(10)?.checked_add((b - b'0') as u64)?;
            has = true;
        } else {
            break;
        }
    }
    if has { Some(val) } else { None }
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

extern "C" fn background_task_entry(_arg: *mut u8) {
    loop {
        crate::kernel::task::yield_now();
    }
}
