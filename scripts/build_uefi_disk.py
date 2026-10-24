#!/usr/bin/env python3
import struct
import os
import subprocess
import sys

def build_fat16_image(out_path, files):
    """
    Builds a standalone 64 MiB FAT16 image populated with given files.
    files: dict of { "PATH/IN/IMAGE": bytes_data }
    """
    total_sectors = 131072  # 64 MiB
    bytes_per_sec = 512
    sec_per_clust = 4       # 2048 bytes per cluster
    rsvd_sec_cnt = 4
    num_fats = 2
    root_ent_cnt = 512      # 32 sectors for root dir
    fat_size_sec = 256      # 256 sectors per FAT = 128 KiB

    data_start_sec = rsvd_sec_cnt + (num_fats * fat_size_sec) + (root_ent_cnt * 32 // bytes_per_sec)
    total_clusters = (total_sectors - data_start_sec) // sec_per_clust

    img = bytearray(total_sectors * bytes_per_sec)

    # 1. Boot Sector (BPB)
    bpb = bytearray(bytes_per_sec)
    bpb[0:3] = b"\xEB\x3C\x90" # JMP short
    bpb[3:11] = b"AEGIS64 "
    struct.pack_into("<HBHBHHBHHHII", bpb, 11,
                     bytes_per_sec,
                     sec_per_clust,
                     rsvd_sec_cnt,
                     num_fats,
                     root_ent_cnt,
                     0,          # TotSec16
                     0xF8,       # Media descriptor (fixed disk)
                     fat_size_sec,
                     63,         # SecPerTrk
                     255,        # NumHeads
                     0,          # HiddSec
                     total_sectors # TotSec32
    )
    bpb[36] = 0x80 # Drive number
    bpb[38] = 0x29 # Extended boot signature
    struct.pack_into("<I", bpb, 39, 0x12345678) # Volume ID
    bpb[43:54] = b"PURPLE_ESP "
    bpb[54:62] = b"FAT16   "
    bpb[510:512] = b"\x55\xAA"
    img[0:bytes_per_sec] = bpb

    # 2. Initialize FATs
    fat = bytearray(fat_size_sec * bytes_per_sec)
    struct.pack_into("<H", fat, 0, 0xFFF8) # Media descriptor
    struct.pack_into("<H", fat, 2, 0xFFFF) # End of cluster marker

    # Directory and file writing
    next_cluster = 2
    root_dir_sec = rsvd_sec_cnt + (num_fats * fat_size_sec)
    root_dir_offset = root_dir_sec * bytes_per_sec

    def write_cluster_data(cluster, data):
        c_sec = data_start_sec + (cluster - 2) * sec_per_clust
        c_off = c_sec * bytes_per_sec
        img[c_off:c_off + len(data)] = data

    def link_fat_chain(start_cluster, num_clusters):
        for i in range(num_clusters - 1):
            struct.pack_into("<H", fat, (start_cluster + i) * 2, start_cluster + i + 1)
        struct.pack_into("<H", fat, (start_cluster + num_clusters - 1) * 2, 0xFFFF)

    # Helper for 8.3 short filename
    def make_short_name(name):
        parts = name.split(".")
        base = parts[0][:8].upper().ljust(8)
        ext = parts[1][:3].upper().ljust(3) if len(parts) > 1 else "   "
        return (base + ext).encode("ascii")

    # Directory structures
    dirs = { "": root_dir_offset } # path -> offset in img

    # First create directory tree
    for file_path in sorted(files.keys()):
        parts = [p for p in file_path.strip("/").split("/") if p]
        cur_path = ""
        for i in range(len(parts) - 1):
            dir_name = parts[i]
            parent_path = cur_path
            cur_path = (cur_path + "/" + dir_name).strip("/")
            if cur_path not in dirs:
                # Allocate a cluster for new directory
                dir_cluster = next_cluster
                next_cluster += 1
                link_fat_chain(dir_cluster, 1)

                dir_off = (data_start_sec + (dir_cluster - 2) * sec_per_clust) * bytes_per_sec
                dirs[cur_path] = dir_off

                # Put entry into parent dir
                parent_off = dirs[parent_path]
                for ent_idx in range(root_ent_cnt if parent_path == "" else (sec_per_clust * bytes_per_sec // 32)):
                    entry_pos = parent_off + ent_idx * 32
                    if img[entry_pos] == 0:
                        img[entry_pos:entry_pos + 11] = make_short_name(dir_name)
                        img[entry_pos + 11] = 0x10 # Directory attribute
                        struct.pack_into("<H", img, entry_pos + 26, dir_cluster)
                        break

                # Initialize "." and ".." in new dir
                dot = bytearray(32)
                dot[0:11] = b".          "
                dot[11] = 0x10
                struct.pack_into("<H", dot, 26, dir_cluster)
                img[dir_off:dir_off + 32] = dot

                dotdot = bytearray(32)
                dotdot[0:11] = b"..         "
                dotdot[11] = 0x10
                struct.pack_into("<H", dotdot, 26, 0)
                img[dir_off + 32:dir_off + 64] = dotdot

    # Write files
    for file_path, content in files.items():
        parts = [p for p in file_path.strip("/").split("/") if p]
        dir_path = "/".join(parts[:-1])
        file_name = parts[-1]
        dir_off = dirs[dir_path]

        file_len = len(content)
        needed_clusters = max(1, (file_len + (sec_per_clust * bytes_per_sec) - 1) // (sec_per_clust * bytes_per_sec))
        file_cluster = next_cluster
        next_cluster += needed_clusters
        link_fat_chain(file_cluster, needed_clusters)

        # Write data
        for c_idx in range(needed_clusters):
            chunk = content[c_idx * sec_per_clust * bytes_per_sec : (c_idx + 1) * sec_per_clust * bytes_per_sec]
            write_cluster_data(file_cluster + c_idx, chunk)

        # Write directory entry
        max_ents = root_ent_cnt if dir_path == "" else (sec_per_clust * bytes_per_sec // 32)
        for ent_idx in range(max_ents):
            entry_pos = dir_off + ent_idx * 32
            if img[entry_pos] == 0:
                img[entry_pos:entry_pos + 11] = make_short_name(file_name)
                img[entry_pos + 11] = 0x20 # Archive / file
                struct.pack_into("<H", img, entry_pos + 26, file_cluster)
                struct.pack_into("<I", img, entry_pos + 28, file_len)
                break

    # Copy FAT 1 and FAT 2 into image
    fat1_offset = rsvd_sec_cnt * bytes_per_sec
    fat2_offset = (rsvd_sec_cnt + fat_size_sec) * bytes_per_sec
    img[fat1_offset:fat1_offset + len(fat)] = fat
    img[fat2_offset:fat2_offset + len(fat)] = fat

    with open(out_path, "wb") as f:
        f.write(img)

def crc32_gpt(data):
    import zlib
    return zlib.crc32(data) & 0xFFFFFFFF

def format_purplefs_v2(total_blocks, label, files):
    import struct, zlib
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

def build_gpt_disk(disk_path, esp_img_path, total_disk_sectors=262144): # 128 MiB total disk
    """
    Builds a GPT partitioned disk with:
    Partition 1: ESP (FAT16/FAT32)
    Partition 2: PurpleFS Partition
    """
    esp_data = open(esp_img_path, "rb").read()
    esp_sectors = len(esp_data) // 512

    disk = bytearray(total_disk_sectors * 512)

    # Protective MBR at LBA 0
    mbr = bytearray(512)
    # Partition 1: type 0xEE (GPT Protective)
    mbr[446 + 4] = 0xEE # Type
    struct.pack_into("<I", mbr, 446 + 8, 1) # Start LBA
    struct.pack_into("<I", mbr, 446 + 12, total_disk_sectors - 1) # Size
    mbr[510:512] = b"\x55\xAA"
    disk[0:512] = mbr

    # GPT Partitions
    # Partition 1: ESP (LBA 2048 to 2048 + esp_sectors - 1)
    p1_start = 2048
    p1_end = p1_start + esp_sectors - 1

    # Partition 2: PurpleFS (remaining disk space)
    p2_start = p1_end + 1
    p2_end = total_disk_sectors - 34
    p2_sectors = p2_end - p2_start + 1
    p2_blocks = min(p2_sectors // 8, 32768)

    # Copy ESP filesystem into disk at p1_start
    disk[p1_start * 512 : (p1_start + esp_sectors) * 512] = esp_data

    # Format and copy PurpleFS v2 filesystem into Partition 2
    pfs2_files = {
        "welcome.txt": (
            "Welcome to Purple OS / Aegis64 Native Filesystem!\n"
            "Architecture: x86-64 | Kernel: Aegis64\n"
            "Filesystem: PurpleFS v2 Persistent Storage\n"
        ),
        "version.txt": "Aegis64 Kernel v0.4.0 (Stage 4B Safe Kernel Storage Integration)\n",
        "motd": "Purple OS: Correctness, Memory Safety, and Recoverability.\n",
        "desktop.cfg": "resolution=1024x768\nwallpaper=cosmic_purple\ntheme=violet_dark\n",
    }
    # If built .apapplication packages exist, copy them into the root filesystem
    for app_name in ["terminal", "sysmon", "files", "calculator"]:
        app_path = f"build/{app_name}.apapplication"
        if os.path.exists(app_path):
            with open(app_path, "rb") as af:
                pfs2_files[f"{app_name}.apapplication"] = af.read()

    pfs2_data = format_purplefs_v2(p2_blocks, "PurpleSystem", pfs2_files)
    disk[p2_start * 512 : p2_start * 512 + len(pfs2_data)] = pfs2_data

    # Generate Partition Entries
    entries = bytearray(128 * 128) # 128 entries of 128 bytes each

    # Entry 1: EFI System Partition GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B
    esp_guid = bytes([
        0x28, 0x73, 0x2A, 0xC1,
        0x1F, 0xF8,
        0xD2, 0x11,
        0xBA, 0x4B,
        0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B
    ])
    unique_guid1 = os.urandom(16)
    entries[0:16] = esp_guid
    entries[16:32] = unique_guid1
    struct.pack_into("<QQQ", entries, 32, p1_start, p1_end, 0)
    name1 = "EFI System Partition".encode("utf-16le")
    entries[56:56 + len(name1)] = name1

    # Entry 2: PurpleFS Partition GUID: 4F535046-5332-0001-8000-00A0C911D504
    pfs_guid = bytes([
        0x46, 0x50, 0x53, 0x4F,
        0x32, 0x53,
        0x01, 0x00,
        0x80, 0x00,
        0x00, 0xA0, 0xC9, 0x11, 0xD5, 0x04
    ])
    unique_guid2 = os.urandom(16)
    entries[128:144] = pfs_guid
    entries[144:160] = unique_guid2
    struct.pack_into("<QQQ", entries, 160, p2_start, p2_end, 0)
    name2 = "PurpleFS v2 Data".encode("utf-16le")
    entries[184:184 + len(name2)] = name2

    entries_crc = crc32_gpt(entries)

    # GPT Header at LBA 1
    gpt_hdr = bytearray(512)
    gpt_hdr[0:8] = b"EFI PART"
    struct.pack_into("<II", gpt_hdr, 8, 0x00010000, 92) # Revision 1.0, Header size 92
    struct.pack_into("<Q", gpt_hdr, 24, 1) # MyLBA = 1
    struct.pack_into("<Q", gpt_hdr, 32, total_disk_sectors - 1) # AlternateLBA
    struct.pack_into("<Q", gpt_hdr, 40, 34) # FirstUsableLBA
    struct.pack_into("<Q", gpt_hdr, 48, total_disk_sectors - 34) # LastUsableLBA
    gpt_hdr[56:72] = os.urandom(16) # Disk GUID
    struct.pack_into("<QII", gpt_hdr, 72, 2, 128, 128) # PartitionEntryLBA=2, NumEntries=128, EntrySize=128
    struct.pack_into("<I", gpt_hdr, 88, entries_crc)

    hdr_crc = crc32_gpt(gpt_hdr[:92])
    struct.pack_into("<I", gpt_hdr, 16, hdr_crc)

    # Place Primary Header and Entries into disk
    disk[512:1024] = gpt_hdr
    disk[1024:1024 + len(entries)] = entries

    # Generate Backup GPT Header at last LBA
    backup_hdr = bytearray(gpt_hdr)
    struct.pack_into("<Q", backup_hdr, 24, total_disk_sectors - 1) # MyLBA = Last LBA
    struct.pack_into("<Q", backup_hdr, 32, 1)                      # AlternateLBA = 1
    struct.pack_into("<Q", backup_hdr, 72, total_disk_sectors - 33) # PartitionEntryLBA
    struct.pack_into("<I", backup_hdr, 16, 0)
    backup_crc = crc32_gpt(backup_hdr[:92])
    struct.pack_into("<I", backup_hdr, 16, backup_crc)

    # Place Backup Entries and Backup Header at end of disk
    backup_entries_lba = total_disk_sectors - 33
    disk[backup_entries_lba * 512 : (backup_entries_lba + 32) * 512] = entries
    disk[(total_disk_sectors - 1) * 512 : total_disk_sectors * 512] = backup_hdr

    with open(disk_path, "wb") as f:
        f.write(disk)
    print(f"[OK] Successfully built GPT disk: {disk_path} ({len(disk) // (1024*1024)} MiB)")

def main():
    os.makedirs("build", exist_ok=True)

    print("=== Generating GRUB BOOTX64.EFI ===")
    grub_cmd = [
        "grub-mkimage",
        "-O", "x86_64-efi",
        "-o", "build/BOOTX64.EFI",
        "-p", "/EFI/BOOT",
        "fat", "part_gpt", "part_msdos", "normal", "boot",
        "multiboot", "multiboot2", "configfile", "efi_gop", "video",
        "gfxterm", "serial"
    ]
    subprocess.check_call(grub_cmd)

    grub_cfg = """set timeout=1
set default=0
terminal_output console serial
terminal_input console serial

menuentry "Purple OS / Aegis64" {
    insmod efi_gop
    insmod video
    multiboot /KERNEL.ELF
    boot
}
"""

    kernel_elf_data = open("build/kernel.elf", "rb").read()
    bootx64_data = open("build/BOOTX64.EFI", "rb").read()

    files = {
        "EFI/BOOT/BOOTX64.EFI": bootx64_data,
        "EFI/BOOT/GRUB.CFG": grub_cfg.encode("utf-8"),
        "KERNEL.ELF": kernel_elf_data
    }

    out_disk = sys.argv[1] if len(sys.argv) > 1 else "build/aegis64-uefi.img"
    esp_disk = sys.argv[2] if len(sys.argv) > 2 else "build/esp.img"

    print(f"=== Building FAT ESP Image ({esp_disk}) ===")
    build_fat16_image(esp_disk, files)

    print(f"=== Building GPT Multi-Partition Disk Image ({out_disk}) ===")
    build_gpt_disk(out_disk, esp_disk)

    # Copy OVMF vars template
    ovmf_vars_src = "/usr/share/edk2/x64/OVMF_VARS.4m.fd"
    if os.path.exists(ovmf_vars_src):
        with open(ovmf_vars_src, "rb") as sf, open("build/ovmf_vars.fd", "wb") as df:
            df.write(sf.read())
        print(f"[OK] Prepared OVMF vars: build/ovmf_vars.fd")

if __name__ == "__main__":
    main()
