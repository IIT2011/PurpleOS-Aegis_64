#!/usr/bin/env python3
"""
scripts/verify_gpt_300m_layout.py
Separate test verification script for the planned GPT layout:
- ~300 MiB FAT32 EFI System Partition
- PurpleFS partition using the remaining intended space
- Kernel discovers, validates, and mounts PurpleFS and reads seed files
- Preserves the existing bootable image without modification.
"""

import os
import sys
import struct
import zlib
import tempfile
import subprocess
import time

def crc32_gpt(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFFFFFF

def build_fat32_bpb(total_sectors: int) -> bytearray:
    """Builds a minimal FAT32 boot sector for the ESP partition."""
    bpb = bytearray(512)
    bpb[0:3] = b"\xEB\x58\x90"
    bpb[3:11] = b"AEGIS64 "
    bytes_per_sec = 512
    sec_per_clust = 8 # 4 KiB clusters
    rsvd_sec_cnt = 32
    num_fats = 2
    root_ent_cnt = 0 # 0 for FAT32
    tot_sec16 = 0
    media = 0xF8
    fat_sz16 = 0
    sec_per_trk = 63
    num_heads = 255
    hidd_sec = 2048
    tot_sec32 = total_sectors

    struct.pack_into("<HBHBHHBHHII", bpb, 11,
                     bytes_per_sec, sec_per_clust, rsvd_sec_cnt,
                     num_fats, root_ent_cnt, tot_sec16, media,
                     fat_sz16, sec_per_trk, num_heads, hidd_sec)
    struct.pack_into("<I", bpb, 32, tot_sec32)

    # FAT32 specific
    # Total clusters ~ (total_sectors - 32) / 8 ~ 76796
    # Each FAT needs 76796 * 4 bytes = 307184 bytes -> 600 sectors
    fat_sz32 = 600
    ext_flags = 0
    fs_ver = 0
    root_clust = 2
    fs_info = 1
    bk_boot_sec = 6

    struct.pack_into("<IHHIHH", bpb, 36, fat_sz32, ext_flags, fs_ver, root_clust, fs_info, bk_boot_sec)
    bpb[64] = 0x80 # Drive number
    bpb[66] = 0x29 # Boot sig
    struct.pack_into("<I", bpb, 67, 0x1234ABCD) # Vol ID
    bpb[71:82] = b"PURPLE_ESP "
    bpb[82:90] = b"FAT32   "
    bpb[510:512] = b"\x55\xAA"
    return bpb

def format_purplefs_v2(total_blocks, label, files):
    data = bytearray(total_blocks * 4096)

    # 1. Superblock at block 0
    sb = bytearray(4096)
    sig = b"PURPLEFS\x00PFS2\x00\x00\x00"
    sb[0:16] = sig

    journal_start = total_blocks - 8
    struct.pack_into("<IIIIIIIIIIII", sb, 16,
        2,              # version = 2
        4096,           # block_size = 4096
        total_blocks,   # total_blocks
        total_blocks,   # free_blocks placeholder
        256,            # total_inodes
        256,            # free_inodes placeholder
        1,              # bitmap_start_block
        1,              # bitmap_block_count
        2,              # inode_start_block
        8,              # inode_block_count
        10,             # data_start_block
        journal_start   # journal_start_block
    )
    struct.pack_into("<II", sb, 16 + 48, 8, 1) # journal_block_count=8, feature_flags=1
    struct.pack_into("<QQII", sb, 16 + 56, 0, 0, 0, 1) # clean_unmount=1
    sb[0x60:0x70] = b"\x11\x22\x33\x44" * 4
    lbl_b = label.encode("ascii")[:15]
    sb[0x70:0x70+len(lbl_b)] = lbl_b

    # Allocation bitmap
    bitmap = bytearray(4096)
    def set_bit(b):
        bitmap[b // 8] |= (1 << (b % 8))

    for b in range(11): set_bit(b)
    for b in range(journal_start, total_blocks): set_bit(b)
    for b in range(total_blocks, 32768): set_bit(b)

    # Inode table
    inodes = bytearray(8 * 4096)

    # Root dir dirents block
    dir_block = bytearray(4096)
    def pack_dirent(offset, ino, name, ftype):
        name_b = name.encode("ascii")[:23]
        struct.pack_into("<IHBB", dir_block, offset, ino, 32, len(name_b), ftype)
        dir_block[offset+8:offset+8+len(name_b)] = name_b

    pack_dirent(0, 1, ".", 2)
    pack_dirent(32, 1, "..", 2)
    dirent_offset = 64

    allocated_blocks = 11
    allocated_inodes = 1 + len(files)
    cur_data_block = 11
    file_ino = 2

    for fname, fcontent in files.items():
        if isinstance(fcontent, str):
            fcontent = fcontent.encode("utf-8")
        fsize = len(fcontent)
        nblocks = (fsize + 4095) // 4096 if fsize > 0 else 0
        file_blocks = []
        for _ in range(nblocks):
            file_blocks.append(cur_data_block)
            set_bit(cur_data_block)
            cur_data_block += 1
            allocated_blocks += 1

        for i, blk in enumerate(file_blocks):
            chunk = fcontent[i*4096 : (i+1)*4096]
            data[blk*4096 : blk*4096 + len(chunk)] = chunk

        ino_offset = file_ino * 128
        struct.pack_into("<HHHHQQQ", inodes, ino_offset,
            0x81A4, # S_IFREG | 0644
            0, 0, 1, # uid, gid, links_count
            fsize, 0, 0 # size, atime, mtime
        )
        for i, blk in enumerate(file_blocks[:12]):
            struct.pack_into("<I", inodes, ino_offset + 0x2C + i*4, blk)

        icrc = zlib.crc32(inodes[ino_offset : ino_offset + 0x7C]) & 0xFFFFFFFF
        struct.pack_into("<I", inodes, ino_offset + 0x7C, icrc)

        pack_dirent(dirent_offset, file_ino, fname, 1)
        dirent_offset += 32
        file_ino += 1

    # Root dir inode (Inode 1)
    struct.pack_into("<HHHHQQQ", inodes, 128,
        0x41ED, # S_IFDIR | 0755
        0, 0, 2, # links
        4096, 0, 0
    )
    struct.pack_into("<I", inodes, 128 + 0x2C, 10)
    root_icrc = zlib.crc32(inodes[128 : 128 + 0x7C]) & 0xFFFFFFFF
    struct.pack_into("<I", inodes, 128 + 0x7C, root_icrc)

    # Free blocks/inodes in superblock
    struct.pack_into("<II", sb, 16 + 8, total_blocks - allocated_blocks - 8, 256 - allocated_inodes)
    sbcrc = zlib.crc32(sb[:0x80]) & 0xFFFFFFFF
    struct.pack_into("<I", sb, 0x80, sbcrc)

    # Journal header
    jhdr = bytearray(4096)
    struct.pack_into("<IIQI", jhdr, 0, 0x5046534A, 0, 1, 0)
    jcrc = zlib.crc32(jhdr[:4+4+8+4+24+24]) & 0xFFFFFFFF
    struct.pack_into("<I", jhdr, 4+4+8+4+24+24, jcrc)

    data[0:4096] = sb
    data[4096:8192] = bitmap
    data[2*4096 : 10*4096] = inodes
    data[10*4096 : 11*4096] = dir_block
    data[journal_start*4096 : (journal_start+1)*4096] = jhdr

    return data

def build_300m_gpt_image(image_path: str):
    """
    Creates a 600 MiB disk image:
    - Partition 1: 300 MiB FAT32 EFI System Partition (614400 sectors)
    - Partition 2: ~299 MiB PurpleFS v2 Partition (612319 sectors)
    """
    total_disk_sectors = 1228800 # 600 MiB
    disk = bytearray(total_disk_sectors * 512)

    # Protective MBR
    mbr = bytearray(512)
    mbr[446 + 4] = 0xEE
    struct.pack_into("<I", mbr, 446 + 8, 1)
    struct.pack_into("<I", mbr, 446 + 12, total_disk_sectors - 1)
    mbr[510:512] = b"\x55\xAA"
    disk[0:512] = mbr

    p1_start = 2048
    p1_sectors = 614400 # 300 MiB exactly
    p1_end = p1_start + p1_sectors - 1

    p2_start = p1_end + 1
    p2_end = total_disk_sectors - 34
    p2_sectors = p2_end - p2_start + 1
    p2_blocks = min(p2_sectors // 8, 32768)

    # Format Partition 1 with FAT32 BPB
    fat32_bpb = build_fat32_bpb(p1_sectors)
    disk[p1_start * 512 : p1_start * 512 + 512] = fat32_bpb

    # Format Partition 2 with PurpleFS v2
    files = {
        "welcome.txt": (
            "Welcome to Purple OS / Aegis64 Native Filesystem!\n"
            "Architecture: x86-64 | Kernel: Aegis64\n"
            "Filesystem: PurpleFS v2 Persistent Storage\n"
        ),
        "version.txt": "Aegis64 Kernel v0.4.0 (Stage 4B Safe Kernel Storage Integration)\n",
        "motd": "Purple OS: Correctness, Memory Safety, and Recoverability.\n"
    }
    pfs2_data = format_purplefs_v2(p2_blocks, "PurpleSystem", files)
    disk[p2_start * 512 : p2_start * 512 + len(pfs2_data)] = pfs2_data

    # GPT Partition Entries
    entries = bytearray(128 * 128)
    esp_guid = bytes([
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11,
        0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B
    ])
    entries[0:16] = esp_guid
    entries[16:32] = os.urandom(16)
    struct.pack_into("<QQQ", entries, 32, p1_start, p1_end, 0)
    esp_name = "EFI System Partition".encode("utf-16le")
    entries[56:56+len(esp_name)] = esp_name

    pfs_guid = bytes([
        0x46, 0x50, 0x53, 0x4F,
        0x32, 0x53,
        0x01, 0x00,
        0x80, 0x00,
        0x00, 0xA0, 0xC9, 0x11, 0xD5, 0x04
    ])
    entries[128:144] = pfs_guid
    entries[144:160] = os.urandom(16)
    struct.pack_into("<QQQ", entries, 160, p2_start, p2_end, 0)
    pfs_name = "PurpleFS v2 Data".encode("utf-16le")
    entries[184:184+len(pfs_name)] = pfs_name

    entries_crc = crc32_gpt(entries)

    # Primary GPT Header
    gpt_hdr = bytearray(512)
    gpt_hdr[0:8] = b"EFI PART"
    struct.pack_into("<II", gpt_hdr, 8, 0x00010000, 92)
    struct.pack_into("<QQQQ", gpt_hdr, 24, 1, total_disk_sectors - 1, 34, total_disk_sectors - 34)
    gpt_hdr[56:72] = os.urandom(16)
    struct.pack_into("<QII", gpt_hdr, 72, 2, 128, 128)
    struct.pack_into("<I", gpt_hdr, 88, entries_crc)
    hdr_crc = crc32_gpt(gpt_hdr[:92])
    struct.pack_into("<I", gpt_hdr, 16, hdr_crc)

    disk[512:1024] = gpt_hdr
    disk[1024:1024 + len(entries)] = entries

    # Backup Header
    backup_hdr = bytearray(gpt_hdr)
    struct.pack_into("<Q", backup_hdr, 24, total_disk_sectors - 1)
    struct.pack_into("<Q", backup_hdr, 32, 1)
    struct.pack_into("<Q", backup_hdr, 72, total_disk_sectors - 33)
    struct.pack_into("<I", backup_hdr, 16, 0)
    backup_crc = crc32_gpt(backup_hdr[:92])
    struct.pack_into("<I", backup_hdr, 16, backup_crc)

    backup_entries_lba = total_disk_sectors - 33
    disk[backup_entries_lba * 512 : (backup_entries_lba + 32) * 512] = entries
    disk[(total_disk_sectors - 1) * 512 : total_disk_sectors * 512] = backup_hdr

    with open(image_path, "wb") as f:
        f.write(disk)
    print(f"[OK] Generated separate test disk image: {image_path} (600 MiB)")
    print(f"     Partition 1: LBA {p1_start}..{p1_end} ({p1_sectors * 512 / (1024*1024):.2f} MiB FAT32 ESP)")
    print(f"     Partition 2: LBA {p2_start}..{p2_end} ({p2_sectors * 512 / (1024*1024):.2f} MiB PurpleFS Data)")

def verify_with_qemu(image_path: str):
    """Boots Aegis64 kernel with the test disk attached and verifies detection."""
    print("=== Launching QEMU to verify Aegis64 GPT & PurpleFS detection on 300 MiB layout ===")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "512M",
        "-no-reboot",
        "-drive", f"file={image_path},format=raw,index=1,media=disk"
    ]
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    time.sleep(1.5)
    commands = [
        "disk status\n",
        "filesystem status\n",
        "list files\n",
        "peek welcome.txt\n",
        "aegisdetect\n",
        "help\n"
    ]
    for c in commands:
        try:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.2)
        except Exception:
            break

    time.sleep(0.5)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n=== Kernel Serial Log ===")
    print(stdout)

    assertions = [
        ("Valid GPT with 2 Partitions", "Valid GPT detected with 2 partition(s)"),
        ("EFI System Partition Found", "Part 0: 'EFI System Partition' [2048..616447, 614400 sectors] PurpleFS=0"),
        ("PurpleFS v2 Partition Found", "Part 1: 'PurpleFS v2 Data' [616448..1228766, 612319 sectors] PurpleFS=1"),
        ("PurpleFS Mounted", "PurpleFS: Mounted volume 'PurpleSystem'"),
        ("Seed welcome.txt Verified", "AegisDetect: Component 'welcome.txt' [TRUSTED] (SHA-256 verified, 132 bytes)"),
        ("Seed version.txt Verified", "AegisDetect: Component 'version.txt' [TRUSTED] (SHA-256 verified, 65 bytes)"),
        ("Seed motd Verified", "AegisDetect: Component 'motd' [TRUSTED] (SHA-256 verified, 59 bytes)")
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name}")
        else:
            print(f"[FAIL] Missing: {name}")
            failed += 1

    return failed

def main():
    test_img = "/tmp/test_300m_gpt_layout.img"
    try:
        build_300m_gpt_image(test_img)
        failures = verify_with_qemu(test_img)
        if failures == 0:
            print("\n>>> ALL 300 MiB ESP + PURPLEFS GPT LAYOUT TESTS PASSED! <<<\n")
            return 0
        else:
            print(f"\n>>> 300 MiB GPT LAYOUT VERIFICATION FAILED ({failures} errors) <<<\n")
            return 1
    finally:
        if os.path.exists(test_img):
            os.remove(test_img)

if __name__ == "__main__":
    sys.exit(main())
