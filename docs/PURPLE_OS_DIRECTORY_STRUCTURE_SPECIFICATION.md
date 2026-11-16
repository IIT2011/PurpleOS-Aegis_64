# PURPLE OS / AEGIS64 — CANONICAL FILESYSTEM DIRECTORY STRUCTURE SPECIFICATION

**Document Version:** 1.0.0  
**Status:** Approved Architectural Standard  
**Kernel:** Aegis64 Native 64-bit Kernel (Pure Rust)  
**Filesystem:** PurpleFS v3 / GPT Partition Hierarchy  

---

## 1. Executive Summary

Purple OS establishes a secure, human-readable, single-root hierarchical directory architecture. It strictly eliminates Unix-style legacy paths (`/usr/bin`, `/etc`, `/var`, `/home`) and enforces clear privilege boundaries between **Core System Components**, **System Applications**, **Dynamic Drivers**, and **User Accounts**.

### Fundamental Rules:
1. **Core Applications:** All built-in system applications reside strictly in `/System/Applications/`.
2. **Prohibited Path:** `/System/Kernel/Applications/` is **strictly prohibited and must not exist**. The kernel core is isolated in `/System/Kernel/` and does not house application packages.
3. **User-Installed Applications:** Third-party and user-installed applications reside strictly in `/System/Users/<user>/Applications/` (e.g. `/System/Users/aegis/Applications/`).
4. **Subsystem Drivers:** All `.apdrv` driver modules reside in `/System/Drivers/`.

---

## 2. On-Disk Directory Hierarchy

```text
/ (PurpleRootFS)
├── Boot/                                      # Bootloader & EFI Stage Files
│   └── EFI/
│       └── Boot/
│           └── BOOTX64.EFI                    # UEFI 64-bit bootloader
│
├── System/                                    # Operating System Core Hierarchy
│   ├── Kernel/                                # Monolithic Kernel Core (NO Applications)
│   │   ├── aegis64.elf                        # Native 64-bit kernel image
│   │   └── kernel.sym                         # Dynamic export symbol table
│   │
│   ├── Applications/                          # Core System Applications
│   │   ├── PurpleTerminal.apapplication       # Interactive command terminal
│   │   ├── PurpleFiles.apapplication          # Graphical file manager
│   │   ├── PurpleSettings.apapplication       # System preferences & hardware control
│   │   ├── PurpleViewer.apapplication         # Image and text viewer
│   │   ├── SysMon.apapplication               # Task & CPU performance monitor
│   │   └── DiskMgr.apapplication              # GPT & PurpleFS volume manager
│   │
│   ├── Drivers/                               # Relocatable W^X APDRV Kernel Modules
│   │   ├── Storage.apdrv                      # SATA AHCI & NVMe block storage
│   │   ├── Graphics.apdrv                     # GPU & Bochs VBE display engine
│   │   ├── Network.apdrv                      # Gigabit Ethernet (E1000, RTL8139)
│   │   ├── Audio.apdrv                        # Intel HDA & AC97 audio engine
│   │   ├── USB.apdrv                          # xHCI 3.0 & EHCI 2.0 host controller
│   │   ├── Input.apdrv                        # PS/2 & USB HID input driver
│   │   ├── WiFi.apdrv                         # 802.11 wireless network adapter
│   │   ├── Bluetooth.apdrv                    # HCI Bluetooth controller
│   │   └── Camera.apdrv                       # UVC video capture
│   │
│   ├── Frameworks/                            # Compositor & Graphical UI Toolkits
│   │   ├── Compositor.framework               # Window compositor & blit pipeline
│   │   └── PurpleUI.framework                 # Widget layout & rendering primitives
│   │
│   ├── Services/                              # Background Kernel Services & Daemons
│   │   ├── AegisDetect                        # Cryptographic integrity daemon
│   │   └── PurpleDetect                       # Peripheral discovery & inventory daemon
│   │
│   ├── Configuration/                         # System-Wide Configuration
│   │   ├── system.conf                        # OS release & scheduler policy
│   │   └── desktop.cfg                        # Screen resolution, theme, dock config
│   │
│   ├── Resources/                             # Shared Visual & Font Assets
│   │   ├── Fonts/                             # NotoSans TrueType vector fonts
│   │   ├── Icons/                             # Application and MIME type icons
│   │   └── Wallpapers/                        # Cosmic purple desktop backgrounds
│   │
│   ├── Libraries/                             # Shared Dynamic Relocatable Libraries (.lib)
│   │   └── libc.lib                           # Pure Rust ABI compatibility runtime
│   │
│   ├── Users/                                 # User Accounts & Profiles
│   │   ├── aegis/                             # Default Interactive User
│   │   │   ├── Applications/                  # User-Installed .apapplication Bundles
│   │   │   │   ├── VLC.apapplication          # Multimedia playback player
│   │   │   │   ├── Calculator.apapplication   # Desk calculator
│   │   │   │   └── TextEditor.apapplication   # Plain-text and code editor
│   │   │   ├── Desktop/                       # User desktop shortcuts & documents
│   │   │   ├── Documents/                     # User files & project files
│   │   │   ├── Downloads/                     # Downloaded artifacts
│   │   │   ├── Pictures/                      # User photos & screenshots
│   │   │   ├── Videos/                        # Video clips & captures
│   │   │   ├── Music/                         # Audio files
│   │   │   ├── Settings/                      # Per-user application configuration
│   │   │   ├── Cache/                         # Temporary render & shader caches
│   │   │   └── Trash/                         # Soft-deleted files
│   │   │
│   │   └── Admin/                             # System Administrator Profile
│   │       ├── Applications/                  # Administrator-only utilities
│   │       ├── Desktop/
│   │       └── Documents/
│   │
│   └── manifest.aegis                         # Cryptographic SHA-256 Trusted Baseline
│
└── Recovery/                                  # Mount point for Partition 3 (RO)
    └── (Dedicated fallback rescue environment, kernel, and diagnostic tools)
```

---

## 3. Application Search & Resolution Order

When the shell or desktop launcher attempts to launch an application by ID (e.g. `launch PurpleTerminal` or `launch VLC`):

1. **System Core Search:** `/System/Applications/<name>.apapplication`
2. **User Local Search:** `/System/Users/<active_user>/Applications/<name>.apapplication`
3. **Fail-Closed Enforcement:** Any application not verified by `AegisDetect` against the cryptographic manifest is blocked from execution.
4. **Invalid Locations:** Any path referencing `/System/Kernel/Applications` is rejected immediately with an invalid path error (`PFS3_ERR_INVALID_PATH`).
