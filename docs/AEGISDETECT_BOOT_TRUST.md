# AegisDetect and boot trust

AegisDetect initializes as soon as kernel serial logging is available. It verifies disk-backed command records, shell scripts, and APDRV packages before those inputs are applied or executed. Its application-bundle verifier is available for future loader integration.

Boot halts before external driver loading, desktop startup, and shell startup if the system volume is unavailable, a required component fails verification, or the required command database fails signature verification. At runtime an unsuccessful database reload clears the old database. `path.dtbase` is unsigned by design. Executables resolved through its search paths still require their own signatures.

Scripts use a sibling `<filename>.sig` containing exactly 512 ASCII hex characters with no newline. Sign the ASCII tag `PurpleOS/SHELL-SCRIPT/v1`, one NUL byte, then the complete script bytes using the Creator RSA-2048 PKCS#1 v1.5 SHA-256 key.

`build/mkpfs2_disk --signing-key <key>` creates a signed manifest of the seeded system files, excluding `path.dtbase`. The manifest covers compositor configuration and descriptors, fonts, icons, drivers, and the command database. Build the kernel with Cargo feature `signed-components` to require this manifest. Compiled-in components, including the compositor implementation and desktop artwork, are part of the kernel ELF.

## Native Secure Boot chain

UEFI firmware verifies the Purple OS EFI application using its Secure Boot `db`. The EFI application embeds the kernel ELF, so Authenticode covers the loader and kernel as one image. Before loading the kernel segments or transferring control, AegisX64 also verifies a detached RSA-2048 SHA-256 signature against the same Creator public key used by AegisDetect. An invalid signature stops the boot attempt with an EFI security violation. The protected loader also requires firmware to report Secure Boot enabled and Setup Mode disabled. Firmware Secure Boot protects the verifier itself against replacement. The kernel built with `signed-components` requires the protected UEFI handoff before initializing kernel services. It passes the firmware memory map, ACPI root pointer, and compatible GOP framebuffer information to the kernel after exiting UEFI boot services.

The EFI System Partition contains two copies of the same AegisX64 executable. A named firmware boot option points to `\EFI\BOOT\AEGISX64.EFI`. The standard x86-64 fallback is `\EFI\BOOT\BOOTX64.EFI`. Firmware can use the fallback on a new system before a named boot option is registered. BGRT is ACPI boot-logo metadata and is independent of boot path selection.

Build the kernel with Cargo feature `signed-components` and use a Rust toolchain with the `x86_64-unknown-uefi` target. Build and sign AegisX64 offline:

```text
cargo build --locked --features signed-components --target x86_64-unknown-none
python scripts/build_aegis_efi.py --signing-key <creator-key>
osslsigncode sign -h sha256 -certs efi.crt -key efi.key -in target/Aegisx64.efi -out target/Aegisx64.signed.efi
python scripts/build_uefi_disk.py --efi-signing-cert efi.crt
```

For offline signing, pass `--kernel-signature` with a 256-byte binary signature over the ASCII tag `PurpleOS/UEFI-KERNEL/v1`, one NUL byte, the 8-byte little-endian address of `aegis_uefi_entry`, and the complete kernel ELF. The builder checks that the signature matches the configured Creator public key.

The disk builder checks the AegisX64 signature before packaging it. It uses the signed PurpleFS system image from `--system-disk`, defaulting to `target/aegis-signed-system.img`. Enroll the AegisX64 signing certificate in the target firmware's Secure Boot `db`. Configure the firmware boot option to use the primary path. The builder leaves firmware variables alone.

Use `run-uefi.ps1` with the Secure Boot OVMF code, a matching variable-store template carrying the required certificates, and the protected image. The launcher makes a disposable copy of the variable store and uses a disk snapshot. The launcher presents a legacy IDE controller. For UEFI development testing without enrolled firmware keys, build the loader with `build_aegis_efi.py --development` and a Creator-signed kernel built without `signed-components`. Use `build_uefi_disk.py --development` to package that loader. Direct Multiboot is also available for kernels built without `signed-components`.
