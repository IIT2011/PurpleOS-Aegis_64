# Purple OS / Aegis64 Build Artifacts

This directory contains the compiled system artifacts, bootloader, kernels, drivers, applications, and disk images.

## Disk Images and GitHub 100 MB Limit

Due to GitHub's 100 MB file size limit, the 128 MiB raw UEFI disk image (`aegis64-uefi.img`) is preserved using standard GitHub-compatible formats:

1. **Gzip Compressed (Recommended)**:
   ```bash
   gunzip -k aegis64-uefi.img.gz
   ```

2. **Split Parts Recombination**:
   ```bash
   cat aegis64-uefi.img.part_* > aegis64-uefi.img
   ```

3. **Direct Regeneration**:
   ```bash
   python3 scripts/build_uefi_disk.py
   ```
   or
   ```bash
   make disk
   ```

All other artifacts, including `BOOTX64.EFI`, `kernel64.elf`, driver packages (`*.apdrv`), application bundles (`*.apapplication`), executables (`*.apexec`), and partition images are included directly.
