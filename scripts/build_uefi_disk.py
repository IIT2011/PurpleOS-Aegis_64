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

    # Copy ESP filesystem into disk at p1_start
    disk[p1_start * 512 : (p1_start + esp_sectors) * 512] = esp_data

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

    # Entry 2: PurpleFS Partition GUID: 70757270-6C65-6673-3200-000000000000
    pfs_guid = bytes([
        0x70, 0x72, 0x75, 0x70,
        0x65, 0x6C,
        0x73, 0x66,
        0x32, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00
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

    print("=== Building FAT ESP Image ===")
    build_fat16_image("build/esp.img", files)

    print("=== Building GPT Multi-Partition Disk Image ===")
    build_gpt_disk("build/aegis64-uefi.img", "build/esp.img")

    # Copy OVMF vars template
    ovmf_vars_src = "/usr/share/edk2/x64/OVMF_VARS.4m.fd"
    if os.path.exists(ovmf_vars_src):
        with open(ovmf_vars_src, "rb") as sf, open("build/ovmf_vars.fd", "wb") as df:
            df.write(sf.read())
        print(f"[OK] Prepared OVMF vars: build/ovmf_vars.fd")

if __name__ == "__main__":
    main()
