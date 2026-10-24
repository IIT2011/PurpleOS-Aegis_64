#!/usr/bin/env python3
"""
APDRV v3 Native Module Package Builder for Purple OS / Aegis64.
Constructs valid APDRV100 v3 binary packages.
"""

import struct
import sys
import os

HEADER_FMT = "<8sII8s32s64s32s" + ("I" * 18) + "HHIII"
HEADER_SIZE = struct.calcsize(HEADER_FMT) # 240 bytes

RELOC_FMT = "<IIHq" # offset: uint32, type: uint32, sym_index: uint16, pad: uint16, addend: int64 -> packed struct
# In C:
# typedef struct {
#     uint32_t offset;
#     uint32_t type;
#     uint32_t sym_index;
#     int64_t  addend;
# } __attribute__((packed)) apdrv_reloc_t;
RELOC_FMT = "<IIQI" # wait, sym_index is uint32_t in include/apdrv.h:
# uint32_t offset (4)
# uint32_t type (4)
# uint32_t sym_index (4)
# int64_t addend (8)
# Total packed: 20 bytes!
RELOC_FMT = "<IIIq"
RELOC_SIZE = struct.calcsize(RELOC_FMT)

SYM_FMT = "<32sQ" # name: 32s, address: uint64 (40 bytes)
SYM_SIZE = struct.calcsize(SYM_FMT)

def build_module(filename, name, desc, author, vendor, device, imports):
    # Align helper
    def align4k(val):
        return (val + 4095) & ~4095

    # Text section: minimal x86-64 code:
    # init(): xor eax, eax; ret  (31 c0 c3)
    # fini(): xor eax, eax; ret  (31 c0 c3)
    # probe(vendor, device):
    #   cmp edi, target_vendor; jne fail; cmp esi, target_device; jne fail; xor eax, eax; ret; fail: mov eax, -1; ret
    # ioctl(cmd, arg): xor eax, eax; ret (31 c0 c3)
    text_code = bytearray([
        # offset 0: init() -> return 0
        0x31, 0xC0, 0xC3, 0x90,
        # offset 4: fini() -> return 0
        0x31, 0xC0, 0xC3, 0x90,
        # offset 8: probe(edi=vendor, esi=device)
        0x31, 0xC0, 0xC3, 0x90, # returns 0 (match)
        # offset 12: ioctl() -> return 0
        0x31, 0xC0, 0xC3, 0x90
    ])

    # Pad text to 64 bytes
    while len(text_code) < 64:
        text_code.append(0x90)

    # Rodata: string table
    rodata = bytearray(f"Purple OS Native Module: {name} v1.0.0 (AEGIS64)\0".encode('utf-8'))
    while len(rodata) < 64:
        rodata.append(0x00)

    # Data: 64 bytes
    data = bytearray([0x00] * 64)

    # Import symbols table
    import_bytes = bytearray()
    for imp in imports:
        import_bytes += struct.pack(SYM_FMT, imp.encode('utf-8')[:31], 0)

    # Relocations:
    # 1 relocation: in Data section, 64-bit pointer pointing to Rodata string
    # Data offset in module memory: text_aligned (4096) + rodata_aligned (4096) = 8192
    relocs = bytearray()
    # offset=8192, type=1 (APDRV_RELOC_64), sym_index=0xFFF1 (APDRV_SEC_RODATA), addend=0
    relocs += struct.pack(RELOC_FMT, 8192, 1, 0xFFF1, 0)

    # Layout in binary file:
    # Header (232 bytes)
    # Imports
    # Relocs
    # Text
    # Rodata
    # Data
    cursor = HEADER_SIZE
    import_offset = cursor
    import_count = len(imports)
    cursor += len(import_bytes)

    reloc_offset = cursor
    reloc_count = 1
    cursor += len(relocs)

    text_offset = cursor
    text_size = len(text_code)
    cursor += text_size

    rodata_offset = cursor
    rodata_size = len(rodata)
    cursor += rodata_size

    data_offset = cursor
    data_size = len(data)
    cursor += data_size

    hdr = struct.pack(
        HEADER_FMT,
        b"APDRV100",           # magic
        3,                     # version
        HEADER_SIZE,           # header_size
        b"AEGISMOD",           # mod_type
        name.encode('utf-8')[:31],
        desc.encode('utf-8')[:63],
        author.encode('utf-8')[:31],
        0x010000,              # mod_version (1.0.0)
        text_offset, text_size,
        rodata_offset, rodata_size,
        data_offset, data_size,
        128,                   # bss_size
        reloc_offset, reloc_count,
        import_offset, import_count,
        0, 0,                  # exports
        0,                     # entry_init (offset 0)
        4,                     # entry_fini (offset 4)
        8,                     # entry_probe (offset 8)
        12,                    # entry_ioctl (offset 12)
        vendor, device,
        0x0001 | 0x0002,       # flags (ENFORCE_WX | AUTO_PROBE)
        0,                     # checksum
        0                      # reserved
    )

    full_binary = hdr + import_bytes + relocs + text_code + rodata + data

    os.makedirs(os.path.dirname(os.path.abspath(filename)), exist_ok=True)
    with open(filename, "wb") as f:
        f.write(full_binary)

    print(f"[OK] Generated APDRV module: {filename} ({len(full_binary)} bytes)")

def main():
    out_dir = "build/drivers_apdrv"
    if len(sys.argv) > 1:
        out_dir = sys.argv[1]

    # 1. e1000.apdrv
    build_module(
        os.path.join(out_dir, "e1000.apdrv"),
        "e1000",
        "Intel 82540EM PCIe Gigabit Ethernet Driver",
        "Purple OS Hardware Group",
        0x8086, 0x100E,
        ["kinfo", "netdev_register", "pci_enable_bus_mastering"]
    )

    # 2. xhci.apdrv
    build_module(
        os.path.join(out_dir, "xhci.apdrv"),
        "usb_xhci",
        "Extensible Host Controller Interface (USB 3.0)",
        "Purple OS Hardware Group",
        0xFFFF, 0xFFFF,
        ["kinfo", "usb_dump_controllers"]
    )

    # 3. hdaudio.apdrv
    build_module(
        os.path.join(out_dir, "hdaudio.apdrv"),
        "intel_hda",
        "Intel High Definition Audio Controller",
        "Purple OS Hardware Group",
        0x8086, 0x2668,
        ["kinfo", "audio_dump_devices"]
    )

if __name__ == "__main__":
    main()
