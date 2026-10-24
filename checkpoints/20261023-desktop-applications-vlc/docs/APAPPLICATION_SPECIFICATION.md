# PURPLE OS / AEGIS64 — .APAPPLICATION BUNDLE SPECIFICATION

**Classification:** Native Userland Application Package Specification  
**Architecture:** Aegis64 / Purple OS  
**File Extension:** `.apapplication` (or `.apapp`)  
**Package Magic:** `APAPP100` (8 bytes)  
**Format Version:** 1  
**Security Model:** AegisDetect HMAC-SHA256 Integrity Verification with Trust Anchor  

---

## 1. Executive Overview

The `.apapplication` format is the official self-contained application package format for Purple OS. Designed for the Aegis64 kernel and Purple Desktop environment, an `.apapplication` bundle encapsulates:

1. **Application Manifest & Metadata:** Machine-readable identifier, name, version, author, description, requested window geometry, and execution priorities.
2. **Capability-Based Permissions:** Granular security flags controlling access to the console, graphical window manager, filesystem storage, network stack, and privileged system administration routines.
3. **Multi-Section File Layout:** Explicit offsets and byte counts for executable code, graphical and font assets, shared libraries, and application metadata.
4. **Hardware-Anchored Cryptographic Verification:** End-to-end authentication via HMAC-SHA256 keyed to the AegisDetect system trust anchor, preventing tampered, corrupted, or unauthorized execution.

---

## 2. On-Disk Binary Layout

An `.apapplication` file is structured as a contiguous binary package composed of a fixed manifest header followed by optional payload sections:

```
+-------------------------------------------------------------+
| Section 0: Manifest Header (apapp_manifest_t, 284 bytes)    |
|   - Magic: 'APAPP100'                                       |
|   - Version: 1                                              |
|   - Metadata: App ID, Name, Version, Author, Description    |
|   - Runtime Config: Permissions, Priority, Window (W x H)   |
|   - Section Offsets & Sizes (Exec, Res, Lib, Meta)          |
|   - AegisDetect HMAC-SHA256 Signature (32 bytes)            |
|   - Payload Checksum (CRC32, 4 bytes)                       |
+-------------------------------------------------------------+
| Section 1: Executable Payload (.text / code)                |
|   - Native x86-64 executable instructions / entry point     |
+-------------------------------------------------------------+
| Section 2: Application Resources (.res / assets)            |
|   - Bitmaps, icons, fonts, localized strings                |
+-------------------------------------------------------------+
| Section 3: Dynamic Shared Libraries (.lib)                  |
|   - Relocatable modules or bundled helper libraries         |
+-------------------------------------------------------------+
| Section 4: Extended Metadata (.meta)                        |
|   - Key-value configuration, desktop entry, mime associations|
+-------------------------------------------------------------+
```

---

## 3. Manifest Data Structure

The manifest is defined in C as `apapp_manifest_t` (packed, 284 bytes):

```c
typedef struct {
    uint8_t  magic[8];          /* 0x00..0x07: "APAPP100" */
    uint32_t version;           /* 0x08..0x0B: 1 */
    uint32_t manifest_size;     /* 0x0C..0x0F: sizeof(apapp_manifest_t) */
    char     app_id[32];        /* 0x10..0x2F: e.g. "terminal.apapplication" */
    char     app_name[32];      /* 0x30..0x4F: e.g. "Purple Terminal" */
    char     version_str[16];   /* 0x50..0x5F: e.g. "1.0.0" */
    char     author[32];        /* 0x60..0x7F: e.g. "Purple OS Team" */
    char     description[64];   /* 0x80..0xBF: Application summary */
    uint32_t permissions;       /* 0xC0..0xC3: APAPP_PERM_* bitmask */
    uint32_t default_priority;  /* 0xC4..0xC7: Task priority (1=Low, 2=Normal, 3=High) */
    uint32_t window_width;      /* 0xC8..0xCB: Default window width (pixels) */
    uint32_t window_height;     /* 0xCC..0xCF: Default window height (pixels) */

    /* Section Offsets in bundle file */
    uint32_t exec_offset;       /* 0xD0..0xD3: Byte offset of executable payload */
    uint32_t exec_size;         /* 0xD4..0xD7: Byte length of executable payload */
    uint32_t res_offset;        /* 0xD8..0xDB: Byte offset of resource section */
    uint32_t res_size;          /* 0xDC..0xDF: Byte length of resource section */
    uint32_t lib_offset;        /* 0xE0..0xE3: Byte offset of library section */
    uint32_t lib_size;          /* 0xE4..0xE7: Byte length of library section */
    uint32_t meta_offset;       /* 0xE8..0xEB: Byte offset of metadata section */
    uint32_t meta_size;         /* 0xEC..0xEF: Byte length of metadata section */

    /* Security: HMAC-SHA256 authenticated by AegisDetect */
    uint8_t  hmac_signature[32];/* 0xF0..0x10F: HMAC-SHA256 digest */
    uint32_t payload_checksum;  /* 0x110..0x113: CRC32 of payload */
    uint32_t reserved[2];       /* 0x114..0x11B: Alignment padding */
} __attribute__((packed)) apapp_manifest_t;
```

---

## 4. Capability Permissions Model

Purple OS implements strict principle-of-least-privilege capability bits:

| Flag | Value | Description |
| :--- | :---: | :--- |
| `APAPP_PERM_CONSOLE` | `0x00000001` | Allows standard console I/O (`printf`, `kinfo`, interactive stdin/stdout). |
| `APAPP_PERM_GUI` | `0x00000002` | Permits creation and management of graphical desktop windows via `desktop_create_window()`. |
| `APAPP_PERM_STORAGE` | `0x00000004` | Grants read/write access to user files on PurpleFS (`/Users/`, `/System/Data/`). |
| `APAPP_PERM_NETWORK` | `0x00000008` | Grants network access (raw packets, socket communication via netdev). |
| `APAPP_PERM_SYSADMIN`| `0x00000010` | Privileged operations (modifying system drivers, reboot, low-level hardware access). |

---

## 5. Security & Cryptographic Verification

AegisDetect enforces tamper detection before allocating memory or executing any code from an `.apapplication` bundle.

### 5.1 Trust Anchor Keying
The system verification engine uses the kernel trust anchor:
`"PurpleOS-Aegis64-TrustAnchor-2026"`

### 5.2 HMAC-SHA256 Computation
The signature covers:
1. **Manifest Metadata:** All manifest bytes from offset `0x00` up to `offsetof(hmac_signature)` (240 bytes).
2. **Executable Payload:** The exact executable payload bytes (`exec_size` bytes starting at `exec_offset`).

Formula:
$$\text{HMAC} = \text{HMAC-SHA256}(K_{trust}, \text{Manifest}_{0 \dots 239} \parallel \text{Payload})$$

If the computed HMAC does not match `manifest.hmac_signature`, the kernel logs a security violation and aborts launch with `APAPP_ERR_SIGNATURE`.

---

## 6. Application Lifecycle

1. **Discovery:** Applications are located in `/Applications/` or `/System/Applications/`.
2. **Inspection:** `apapp_parse_manifest()` reads and validates the header, checking magic, version, and section boundaries.
3. **Security Audit:** `apapp_verify_security()` evaluates the HMAC-SHA256 signature against the trust anchor.
4. **Task Instantiation:** `apapp_launch()` spawns an AegisSched task with the specified priority and name.
5. **Window Provisioning:** If `APAPP_PERM_GUI` is set, a desktop window is initialized with title `manifest.app_name` and dimensions `window_width` $\times$ `window_height`.
6. **Execution:** The application's entrypoint runs inside its cooperative or preemptive thread.
7. **Termination:** Upon completion or `kill` command, `apapp_terminate()` destroys the GUI window, releases physical memory pages, and notifies the scheduler.
