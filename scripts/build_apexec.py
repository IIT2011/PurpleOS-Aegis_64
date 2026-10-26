#!/usr/bin/env python3
import struct
import sys
import os
import zlib

APEX_MAGIC = 0x4550417F # '\x7F' 'A' 'P' 'E'
APEX_VERSION = 1
APEX_ARCH_X86_64 = 0x8664
APEX_ABI_PURPLE_V1 = 1

APEX_SEG_NULL = 0
APEX_SEG_CODE = 1
APEX_SEG_RODATA = 2
APEX_SEG_DATA = 3
APEX_SEG_BSS = 4

APEX_FLAG_READ = 1
APEX_FLAG_WRITE = 2
APEX_FLAG_EXEC = 4

def create_apexec(out_path, entry_point, base_vaddr, segments, stack_size=65536):
    """
    segments: list of dicts:
      {
        'type': APEX_SEG_*,
        'flags': APEX_FLAG_*,
        'vaddr': int,
        'mem_size': int,
        'data': bytes (optional for BSS)
      }
    """
    num_segments = len(segments)
    hdr_size = 64
    seg_tbl_size = num_segments * 40

    payload_offset = hdr_size + seg_tbl_size
    curr_offset = payload_offset

    processed_segs = []
    payloads = []

    for s in segments:
        data = s.get('data', b'')
        file_size = len(data)
        mem_size = s.get('mem_size', file_size)
        if mem_size < file_size:
            mem_size = file_size

        if s['type'] == APEX_SEG_BSS:
            seg_file_offset = 0
            file_size = 0
        else:
            seg_file_offset = curr_offset
            payloads.append(data)
            curr_offset += len(data)

        processed_segs.append({
            'type': s['type'],
            'flags': s['flags'],
            'file_offset': seg_file_offset,
            'file_size': file_size,
            'vaddr': s['vaddr'],
            'mem_size': mem_size
        })

    total_file_size = curr_offset

    # Compute CRC32 over payloads
    full_payload = b"".join(payloads)
    checksum = zlib.crc32(full_payload) & 0xFFFFFFFF

    # 1. Pack Header (64 bytes)
    hdr = bytearray(64)
    struct.pack_into("<IBBHIQQQHHIIIIII", hdr, 0,
                     APEX_MAGIC,
                     APEX_VERSION,
                     APEX_ABI_PURPLE_V1,
                     APEX_ARCH_X86_64,
                     hdr_size,
                     entry_point,
                     base_vaddr,
                     total_file_size,
                     num_segments,
                     0, # reserved0
                     stack_size,
                     checksum,
                     0, 0, 0, 0 # reserved1
    )

    # 2. Pack Segment Descriptors (40 bytes each)
    seg_bytes = bytearray()
    for ps in processed_segs:
        sb = bytearray(40)
        struct.pack_into("<IIQQQQ", sb, 0,
                         ps['type'],
                         ps['flags'],
                         ps['file_offset'],
                         ps['file_size'],
                         ps['vaddr'],
                         ps['mem_size'])
        seg_bytes.extend(sb)

    # 3. Assemble and write file
    with open(out_path, "wb") as f:
        f.write(hdr)
        f.write(seg_bytes)
        for p in payloads:
            f.write(p)

    print(f"[OK] Created .apexec binary: {out_path} ({total_file_size} bytes, {num_segments} segments, entry=0x{entry_point:X})")

def build_native_apexec(out_path, title, width, height, init_msg, file_path=None):
    """
    Builds a real native x86-64 .apexec executable that runs in Ring 3,
    uses int $0x80 syscalls to interact with Aegis64 kernel, creates a GUI window,
    optionally opens a file, presents the window, and exits cleanly.
    """
    code_vaddr = 0x0000000000401000
    rodata_vaddr = 0x0000000000402000

    title_b = title.encode('utf-8') + b'\x00'
    msg_b = init_msg.encode('utf-8') + b'\x00'
    fpath_b = (file_path.encode('utf-8') + b'\x00') if file_path else b''

    off_title = 0
    off_msg = len(title_b)
    off_fpath = off_msg + len(msg_b)
    rodata = title_b + msg_b + fpath_b

    code = bytearray()

    # 1. SYS_PRINT (rax = 60, rdi = ptr to msg, rsi = len(msg))
    code.extend(b'\x48\xC7\xC0\x3C\x00\x00\x00')
    disp_msg = (rodata_vaddr + off_msg) - (code_vaddr + len(code) + 7)
    code.extend(b'\x48\x8D\x3D' + struct.pack('<i', disp_msg))
    code.extend(b'\x48\xC7\xC6' + struct.pack('<I', len(msg_b) - 1))
    code.extend(b'\xCD\x80')

    # 2. SYS_WINDOW_CREATE (rax = 40, rdi = title, rsi = 60, rdx = 60, r10 = w, r8 = h)
    code.extend(b'\x48\xC7\xC0\x28\x00\x00\x00')
    disp_title = (rodata_vaddr + off_title) - (code_vaddr + len(code) + 7)
    code.extend(b'\x48\x8D\x3D' + struct.pack('<i', disp_title))
    code.extend(b'\x48\xC7\xC6\x3C\x00\x00\x00')
    code.extend(b'\x48\xC7\xC2\x3C\x00\x00\x00')
    code.extend(b'\x49\xC7\xC2' + struct.pack('<I', width))
    code.extend(b'\x49\xC7\xC0' + struct.pack('<I', height))
    code.extend(b'\xCD\x80')
    code.extend(b'\x48\x89\xC3') # mov %rax, %rbx (save win_id)

    # 3. Optional SYS_OPEN
    if file_path:
        code.extend(b'\x48\xC7\xC0\x14\x00\x00\x00') # SYS_OPEN
        disp_fp = (rodata_vaddr + off_fpath) - (code_vaddr + len(code) + 7)
        code.extend(b'\x48\x8D\x3D' + struct.pack('<i', disp_fp))
        code.extend(b'\x48\xC7\xC6\x01\x00\x00\x00') # O_RDONLY
        code.extend(b'\xCD\x80')

    # 4. SYS_WINDOW_PRESENT (rax = 43, rdi = win_id)
    code.extend(b'\x48\xC7\xC0\x2B\x00\x00\x00')
    code.extend(b'\x48\x89\xDF')
    code.extend(b'\xCD\x80')

    # 5. SYS_EXIT (rax = 1, rdi = 0)
    code.extend(b'\x48\xC7\xC0\x01\x00\x00\x00')
    code.extend(b'\x48\x31\xFF')
    code.extend(b'\xCD\x80')
    code.extend(b'\xF4') # hlt

    segs = [
        {
            'type': APEX_SEG_CODE,
            'flags': APEX_FLAG_READ | APEX_FLAG_EXEC,
            'vaddr': code_vaddr,
            'data': bytes(code),
            'mem_size': 4096
        },
        {
            'type': APEX_SEG_RODATA,
            'flags': APEX_FLAG_READ,
            'vaddr': rodata_vaddr,
            'data': bytes(rodata),
            'mem_size': 4096
        }
    ]

    create_apexec(out_path, code_vaddr, 0x0000000000400000, segs)

def build_fault_test_apexec(out_path):
    """
    Builds a native x86-64 .apexec executable that runs in Ring 3,
    prints a message via SYS_PRINT, then intentionally attempts to dereference
    kernel supervisor memory (0xFFFFFFFF80000000) to prove Ring 3 hardware fault containment.
    """
    code_vaddr = 0x0000000000401000
    rodata_vaddr = 0x0000000000402000

    msg_b = b"Ring 3 fault containment test initiating kernel-space read...\n\x00"
    rodata = msg_b

    code = bytearray()
    # 1. SYS_PRINT (rax = 60, rdi = ptr to msg, rsi = len(msg))
    code.extend(b'\x48\xC7\xC0\x3C\x00\x00\x00')
    disp_msg = rodata_vaddr - (code_vaddr + len(code) + 7)
    code.extend(b'\x48\x8D\x3D' + struct.pack('<i', disp_msg))
    code.extend(b'\x48\xC7\xC6' + struct.pack('<I', len(msg_b) - 1))
    code.extend(b'\xCD\x80')

    # 2. Controlled illegal access: movabs $0xffffffff80000000, %rax; movq (%rax), %rax
    # This must immediately trigger Page Fault #14 in Ring 3 and be contained by Aegis64 IDT
    code.extend(b'\x48\xB8\x00\x00\x00\x80\xFF\xFF\xFF\xFF')
    code.extend(b'\x48\x8B\x00')

    # 3. Fallback SYS_EXIT (should not be reached if fault occurs)
    code.extend(b'\x48\xC7\xC0\x01\x00\x00\x00')
    code.extend(b'\x48\x31\xFF')
    code.extend(b'\xCD\x80')

    segs = [
        {
            'type': APEX_SEG_CODE,
            'flags': APEX_FLAG_READ | APEX_FLAG_EXEC,
            'vaddr': code_vaddr,
            'data': bytes(code),
            'mem_size': 4096
        },
        {
            'type': APEX_SEG_RODATA,
            'flags': APEX_FLAG_READ,
            'vaddr': rodata_vaddr,
            'data': bytes(rodata),
            'mem_size': 4096
        }
    ]

    create_apexec(out_path, code_vaddr, 0x0000000000400000, segs)

def inspect_apexec(path):
    with open(path, "rb") as f:
        data = f.read()

    if len(data) < 64:
        print("[ERR] File too small for APEX header")
        return False

    magic, ver, abi, arch, hdr_size, entry, base_va, file_sz, num_segs, _, stack_sz, crc, _, _, _, _ = struct.unpack_from("<IBBHIQQQHHIIIIII", data, 0)
    print(f"=== APEX Binary Inspection: {path} ===")
    print(f"  Magic:       0x{magic:08X} ({'VALID' if magic == APEX_MAGIC else 'INVALID'})")
    print(f"  Version:     {ver} (ABI {abi})")
    print(f"  Arch:        0x{arch:04X} ({'x86-64' if arch == APEX_ARCH_X86_64 else 'UNKNOWN'})")
    print(f"  Entry Point: 0x{entry:016X}")
    print(f"  Base VAddr:  0x{base_va:016X}")
    print(f"  File Size:   {file_sz} bytes")
    print(f"  Stack Size:  {stack_sz} bytes")
    print(f"  Segments:    {num_segs}")

    offset = hdr_size
    for i in range(num_segs):
        stype, flags, f_off, f_sz, va, mem_sz = struct.unpack_from("<IIQQQQ", data, offset)
        offset += 40
        type_names = {1: "CODE", 2: "RODATA", 3: "DATA", 4: "BSS"}
        perm = ""
        perm += "R" if (flags & 1) else "-"
        perm += "W" if (flags & 2) else "-"
        perm += "X" if (flags & 4) else "-"
        print(f"    Segment {i}: [{type_names.get(stype, 'UNK')}] Perms={perm} VAddr=0x{va:016X} MemSz={mem_sz}B FileOff=0x{f_off:X} FileSz={f_sz}B")
    return True

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "inspect":
        inspect_apexec(sys.argv[2])
    else:
        # Generate sample native ring 3 application: tests/sample_app.apexec
        # Machine code for:
        #   mov $60, %rax       # SYS_PRINT
        #   lea msg(%rip), %rdi # pointer to message in rodata
        #   int $0x80           # syscall via gate 128
        #   mov $1, %rax        # SYS_EXIT
        #   xor %rdi, %rdi      # exit code 0
        #   int $0x80

        # Assembly machine bytes:
        # 48 c7 c0 3c 00 00 00       mov $0x3c, %rax (60 = SYS_PRINT)
        # 48 8d 3d 11 00 00 00       lea 0x11(%rip), %rdi (points to msg)
        # cd 80                      int $0x80
        # 48 c7 c0 01 00 00 00       mov $0x1, %rax (1 = SYS_EXIT)
        # 48 31 ff                   xor %rdi, %rdi
        # cd 80                      int $0x80
        # hlt                        f4 (safety)

        code_bytes = bytes([
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, # mov $60, %rax
            0x48, 0x8D, 0x3D, 0x11, 0x00, 0x00, 0x00, # lea 0x11(%rip), %rdi -> 0x00402000
            0xCD, 0x80,                               # int $0x80
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, # mov $1, %rax
            0x48, 0x31, 0xFF,                         # xor %rdi, %rdi
            0xCD, 0x80,                               # int $0x80
            0xF4                                      # hlt
        ])

        msg = b"Hello from Purple OS Native Ring 3 Application!\x00"

        segs = [
            {
                'type': APEX_SEG_CODE,
                'flags': APEX_FLAG_READ | APEX_FLAG_EXEC,
                'vaddr': 0x0000000000401000,
                'data': code_bytes,
                'mem_size': 4096
            },
            {
                'type': APEX_SEG_RODATA,
                'flags': APEX_FLAG_READ,
                'vaddr': 0x0000000000402000,
                'data': msg,
                'mem_size': 4096
            }
        ]

        os.makedirs("build/apps", exist_ok=True)
        create_apexec("build/apps/sample.apexec", 0x0000000000401000, 0x0000000000400000, segs)
        inspect_apexec("build/apps/sample.apexec")
