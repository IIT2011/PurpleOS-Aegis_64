#!/usr/bin/env python3
"""
Generates the PurpleOS Command Database (/System/CLI/cmd.dtbase).
Computes the SHA-256 cryptographic checksum over the database records
and embeds it into the database header.
"""

import hashlib
import sys
import os

COMMANDS = [
    # Navigation
    ("cd", "Navigation", "ring0:cd", "Change current working directory", "cd <path>"),
    ("chdir", "Navigation", "ring0:cd", "Change current working directory", "chdir <path>"),
    ("pwd", "Navigation", "ring0:pwd", "Print current working directory", "pwd"),
    ("dir", "Navigation", "ring0:dir", "MS-DOS formatted directory listing", "dir [path]"),
    ("ls", "Navigation", "ring0:ls", "Linux formatted directory listing", "ls [-l] [path]"),
    ("tree", "Navigation", "ring0:tree", "Display visual directory hierarchy", "tree"),
    ("stat", "Navigation", "ring0:stat", "Display file status and metadata", "stat <file>"),

    # File Operations & Text
    ("cat", "FileOps", "ring0:cat", "Display file contents", "cat <file>"),
    ("type", "FileOps", "ring0:cat", "Display file contents (MS-DOS)", "type <file>"),
    ("read", "FileOps", "ring0:cat", "Read and display file contents", "read <file>"),
    ("peek", "FileOps", "ring0:peek", "Peek at beginning of file", "peek <file>"),
    ("head", "FileOps", "ring0:head", "Display first lines of file", "head [n] <file>"),
    ("tail", "FileOps", "ring0:tail", "Display last lines of file", "tail [n] <file>"),
    ("grep", "FileOps", "ring0:grep", "Search for pattern in file or stream", "grep <pattern> [file]"),
    ("find", "FileOps", "ring0:grep", "Find pattern in file or stream", "find <pattern> [file]"),
    ("wc", "FileOps", "ring0:wc", "Count lines words and bytes in file", "wc <file>"),
    ("dump", "FileOps", "ring0:dump", "16-byte hexadecimal memory or file dump", "dump <file|device>"),
    ("hex", "FileOps", "ring0:dump", "Hexadecimal file dump", "hex <file>"),
    ("xxd", "FileOps", "ring0:dump", "XXD compatible hex dump", "xxd <file>"),
    ("touch", "FileOps", "ring0:touch", "Create empty file or update timestamp", "touch <file>"),
    ("create", "FileOps", "ring0:touch", "Create new empty file", "create <file>"),
    ("write", "FileOps", "ring0:write", "Write text to file (overwrite)", "write <file> <text>"),
    ("append", "FileOps", "ring0:append", "Append text to file", "append <file> <text>"),
    ("echo", "FileOps", "ring0:echo", "Display line of text or variable", "echo [text]"),
    ("copy", "FileOps", "ring0:copy", "Copy file to destination", "copy <src> <dst>"),
    ("cp", "FileOps", "ring0:copy", "Copy file (POSIX cp)", "cp <src> <dst>"),
    ("move", "FileOps", "ring0:move", "Move or rename file", "move <src> <dst>"),
    ("mv", "FileOps", "ring0:move", "Move or rename file (POSIX mv)", "mv <src> <dst>"),
    ("del", "FileOps", "ring0:del", "Delete file from filesystem", "del <file>"),
    ("rm", "FileOps", "ring0:del", "Remove file (POSIX rm)", "rm <file>"),
    ("delete", "FileOps", "ring0:del", "Delete file", "delete <file>"),
    ("mkdir", "FileOps", "ring0:mkdir", "Create new directory", "mkdir <dir>"),
    ("md", "FileOps", "ring0:mkdir", "Make directory (MS-DOS md)", "md <dir>"),
    ("rmdir", "FileOps", "ring0:rmdir", "Remove empty directory", "rmdir <dir>"),
    ("rd", "FileOps", "ring0:rmdir", "Remove directory (MS-DOS rd)", "rd <dir>"),

    # Storage & Hardware
    ("disks", "Storage", "ring0:disks", "List physical storage block devices", "disks"),
    ("df", "Storage", "ring0:df", "Display filesystem disk space usage", "df"),
    ("filesystem", "Storage", "ring0:df", "Filesystem status and superblock", "filesystem"),
    ("mount", "Storage", "ring0:mount", "Display active VFS mountpoints", "mount"),
    ("sync", "Storage", "ring0:sync", "Synchronize cached disk writes to storage", "sync"),
    ("remount", "Storage", "ring0:remount", "Remount filesystem read-only or read-write", "remount <ro|rw>"),
    ("devices", "Storage", "ring0:devices", "List enumerated hardware peripherals", "devices"),
    ("hwinfo", "Storage", "ring0:devices", "Hardware peripheral inventory", "hwinfo"),
    ("pci", "Storage", "ring0:pci", "Scan and list PCI bus devices", "pci"),
    ("lspci", "Storage", "ring0:pci", "List PCI devices (POSIX lspci)", "lspci"),
    ("mouse", "Storage", "ring0:mouse", "PS/2 mouse controller status", "mouse"),
    ("video", "Storage", "ring0:video", "GPU framebuffer video mode status", "video"),
    ("gpu3d", "Storage", "ring0:gpu3d", "Hardware 3D command processor status", "gpu3d"),
    ("3d", "Storage", "ring0:gpu3d", "3D command processor info", "3d"),

    # GUI & Compositor
    ("compositor", "GUI", "ring0:compositor", "Native window server and framebuffer compositor", "compositor [status|start|stop|demo|render]"),
    ("gui", "GUI", "ring0:compositor", "Graphical user interface environment manager", "gui [start|stop|demo|status]"),
    ("window", "GUI", "ring0:window", "Manage desktop windows and Z-order stack", "window [list|close|focus|move|demo]"),

    # Network
    ("ifconfig", "Network", "ring0:ifconfig", "Network interface configuration and stats", "ifconfig"),
    ("ping", "Network", "ring0:ping", "Send ICMP ECHO_REQUEST to network hosts", "ping <ip>"),
    ("arp", "Network", "ring0:arp", "Display Address Resolution Protocol cache", "arp"),

    # Modules & Symbols
    ("modules", "System", "ring0:modules", "List loaded APDRV kernel driver modules", "modules"),
    ("lsmod", "System", "ring0:modules", "List kernel modules (POSIX lsmod)", "lsmod"),
    ("insmod", "System", "ring0:insmod", "Load APDRV kernel driver module", "insmod <module>"),
    ("ksym", "System", "ring0:ksym", "Display kernel symbol table exports", "ksym"),

    # System & Execution
    ("smp", "System", "ring0:smp", "Symmetric multiprocessing core status", "smp"),
    ("smptest", "System", "ring0:smptest", "Multi-core inter-processor interrupt test", "smptest"),
    ("tasks", "System", "ring0:tasks", "Process and thread task manager", "tasks"),
    ("ps", "System", "ring0:tasks", "List active tasks and CPU affinity", "ps"),
    ("posix", "System", "ring0:posix", "POSIX subsystem status and test suite", "posix [status|test]"),
    ("native", "System", "ring0:native", "Native PurpleOS 68-syscall catalog and test", "native [status|test|list]"),
    ("mem", "System", "ring0:mem", "Physical memory frame allocator and heap status", "mem"),
    ("memory", "System", "ring0:mem", "System memory statistics", "memory"),
    ("free", "System", "ring0:mem", "Free physical and heap memory", "free"),
    ("cpuinfo", "System", "ring0:cpuinfo", "CPU features and topology", "cpuinfo"),
    ("sysinfo", "System", "ring0:sysinfo", "System hardware and kernel architecture summary", "sysinfo"),
    ("uptime", "System", "ring0:uptime", "System running time and PIT counter", "uptime"),
    ("date", "System", "ring0:date", "Current date from CMOS RTC", "date"),
    ("time", "System", "ring0:time", "Current time from CMOS RTC", "time"),
    ("ver", "System", "ring0:ver", "Operating system kernel version", "ver"),
    ("version", "System", "ring0:ver", "Kernel version details", "version"),
    ("uname", "System", "ring0:uname", "Print operating system name and architecture", "uname [-a]"),
    ("whoami", "System", "ring0:whoami", "Print active user identity", "whoami"),
    ("id", "System", "ring0:whoami", "Print user and group identity", "id"),
    ("clear", "System", "ring0:clear", "Clear console display buffer", "clear"),
    ("cls", "System", "ring0:clear", "Clear console screen (MS-DOS cls)", "cls"),
    ("reboot", "System", "ring0:reboot", "Reboot computer via 8042 keyboard controller", "reboot"),
    ("restart", "System", "ring0:reboot", "Restart system", "restart"),
    ("exit", "System", "ring0:exit", "Exit current shell session", "exit"),
    ("panic", "System", "ring0:panic", "Trigger diagnostic kernel panic", "panic"),

    # Security
    ("aegisdetect", "Security", "ring0:aegisdetect", "Verify system components against SHA-256 baseline", "aegisdetect"),
    ("verify", "Security", "ring0:aegisdetect", "Cryptographic integrity verification", "verify security"),
    ("verify security", "Security", "ring0:aegisdetect", "Full SHA-256 baseline integrity verification", "verify security"),

    # Ring 3 (Userland)
    ("ring3", "Userland", "ring3:control", "Ring 3 user mode control and diagnostics", "ring3 [info|test|fault]"),
    ("userland", "Userland", "ring3:control", "Userland subsystem control", "userland [info|test|fault]"),

    # CLI Engine & Utilities
    ("help", "CLI", "ring0:help", "Display categorized commands and command usage manual", "help [command]"),
    ("cmd", "CLI", "ring0:cmd", "Command database manager from /System/CLI/cmd.dtbase", "cmd [reload|verify|info]"),
    ("path", "CLI", "ring0:path", "Search path manager from /System/CLI/path.dtbase", "path [add|remove|reload]"),
    ("history", "CLI", "ring0:history", "Display command execution history", "history"),
    ("alias", "CLI", "ring0:alias", "Define or display command aliases", "alias [name=command]"),
    ("unalias", "CLI", "ring0:unalias", "Remove defined command alias", "unalias <name>"),
    ("export", "CLI", "ring0:export", "Set or display shell environment variables", "export [VAR=val]"),
    ("set", "CLI", "ring0:export", "Set environment variable", "set [VAR=val]"),
    ("env", "CLI", "ring0:env", "Print all exported environment variables", "env"),
    ("calc", "CLI", "ring0:calc", "Mathematical and bitwise hexadecimal calculator", "calc <expression>"),
    ("expr", "CLI", "ring0:calc", "Evaluate arithmetic expression", "expr <expression>"),
    ("which", "CLI", "ring0:which", "Locate command in /System/CLI/cmd.dtbase", "which <command>"),
    ("edit", "CLI", "ring0:edit", "In-terminal visual multi-line text editor", "edit <file>"),
    ("nano", "CLI", "ring0:edit", "Text editor (nano alias)", "nano <file>"),
    ("source", "CLI", "ring0:source", "Execute commands from batch script file", "source <script.sh>"),
    ("run", "CLI", "ring0:source", "Run script batch file", "run <script.sh>"),
    ("batch", "CLI", "ring0:source", "Batch file runner", "batch <script.bat>"),
    ("sleep", "CLI", "ring0:sleep", "Delay execution for specified seconds", "sleep <seconds>"),
    ("seq", "CLI", "ring0:seq", "Print sequence of numbers", "seq <first> [incr] <last>"),
    ("fortune", "CLI", "ring0:fortune", "Display random operating system wisdom or tip", "fortune"),
    ("tip", "CLI", "ring0:fortune", "Display productivity tip", "tip"),
    ("theme", "CLI", "ring0:theme", "Display or switch visual console color theme", "theme [name]"),
    ("color", "CLI", "ring0:color", "Change console text color", "color <hex>"),
    ("title", "CLI", "ring0:title", "Set terminal window title", "title <text>"),
    ("jobs", "CLI", "ring0:jobs", "List active background jobs and tasks", "jobs"),
    ("start", "CLI", "ring0:start", "Launch command or task in background", "start <command>"),
    ("kill", "CLI", "ring0:kill", "Terminate running task by TID", "kill <tid>"),
    ("top", "CLI", "ring0:top", "Display real-time task manager and CPU load", "top"),
    ("monitor", "CLI", "ring0:top", "System performance monitor", "monitor"),
    ("bench", "CLI", "ring0:bench", "Run CPU and memory performance benchmark", "bench"),
    ("sysbench", "CLI", "ring0:bench", "Run full system benchmark suite", "sysbench"),
    ("view", "CLI", "ring0:view", "View file with scroll pager", "view <file>"),
    ("pager", "CLI", "ring0:view", "Terminal pager for text viewing", "pager <file>"),
    ("prompt", "CLI", "ring0:prompt", "Customize interactive shell prompt format", "prompt [format]"),
]

def build_payload():
    lines = []
    for name, cat, target, desc, usage in COMMANDS:
        lines.append(f"[cmd:{name}]")
        lines.append(f"cat={cat}")
        lines.append(f"target={target}")
        lines.append(f"desc={desc}")
        lines.append(f"usage={usage}")
        lines.append("")
    return "\n".join(lines).encode("utf-8")

import hmac

def get_creator_key():
    for kp in ["keys/creator_key.txt", "OS/keys/creator_key.txt", "../keys/creator_key.txt"]:
        if os.path.exists(kp):
            with open(kp, "r") as f:
                return hashlib.sha256(f.read().strip().encode("utf-8")).digest()
    raise RuntimeError("Creator signing key file not found. Ensure keys/creator_key.txt exists.")

def generate_database(creator_key: bytes = None):
    if creator_key is None:
        creator_key = get_creator_key()
    payload = build_payload()
    sig = hmac.new(creator_key, payload, hashlib.sha256).hexdigest()
    
    header = (
        f"# PURPLEOS COMMAND DATABASE\n"
        f"# SIGNER: Creator (Aegis Root Developer)\n"
        f"# KEY_ID: AEGIS-CREATOR-V1\n"
        f"# SIGNATURE: {sig}\n"
        f"# CANONICAL PATH: /System/CLI/cmd.dtbase\n"
        f"# COMMAND COUNT: {len(COMMANDS)}\n\n"
    ).encode("utf-8")
    
    full_content = header + payload
    return full_content, sig

if __name__ == "__main__":
    out_path = sys.argv[1] if len(sys.argv) > 1 else "OS/build/cmd.dtbase"
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    content, sig = generate_database()
    with open(out_path, "wb") as f:
        f.write(content)
    print(f"[OK] Generated {out_path} ({len(content)} bytes, {len(COMMANDS)} commands)")
    print(f"[OK] Creator Cryptographic Signature: {sig}")
