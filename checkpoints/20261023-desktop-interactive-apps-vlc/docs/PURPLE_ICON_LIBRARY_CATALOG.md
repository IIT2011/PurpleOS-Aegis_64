# Purple OS / Aegis64 — Native Icon Library Catalog & Manifest

**Status:** VERIFIED & INTEGRATED
**Total Icon Assets:** 100 distinct identifiers
**Resolutions Available:** 32x32 RGBA, 16x16 RGBA (Assets in `assets/icons/`)

## 1. Visual Design Architecture

All icons conform to the **Hollow Purple Cosmic Visual Identity**:
- **Squircle Containers:** Continuous curvature rounded corners with subtle violet edge illumination (`#7209B7`, `#9D4EDD`).
- **Alpha Coverage Blending:** Smooth anti-aliased subpixel blending over cosmic wallpaper and dark frosted glass backgrounds.
- **Palette Harmony:** Primary violet/indigo bodies (`#130A24`, `#2A1245`), magenta accents (`#F72585`), cyan-electric diagnostic badges (`#4CC9F0`), and gold highlights (`#FFD700`).

## 2. Complete Asset Catalog

| ID | Enum Identifier | Name | Category | Description |
| :---: | :--- | :--- | :--- | :--- |
| 1 | `ICON_APP_WELCOME` | **Welcome** | Core App | Purple OS Welcome Tour |
| 2 | `ICON_APP_SETTINGS` | **Settings** | Core App | System & Preferences Manager |
| 3 | `ICON_APP_TERMINAL` | **Terminal** | Core App | Aegis Interactive Shell |
| 4 | `ICON_APP_FILES` | **Files** | Core App | PurpleFS File Explorer |
| 5 | `ICON_APP_EDITOR` | **Text Editor** | Core App | Lightweight Document Editor |
| 6 | `ICON_APP_SYSMON` | **System Monitor** | Core App | Hardware & Task Monitor |
| 7 | `ICON_APP_APPSTORE` | **App Store** | Core App | Application Package Manager |
| 8 | `ICON_APP_HELP` | **Help Center** | Core App | User Guide & Documentation |
| 9 | `ICON_APP_SCREENSHOT` | **Screenshot** | Core App | Screen Capture Utility |
| 10 | `ICON_APP_CALCULATOR` | **Calculator** | Core App | Scientific Calculator |
| 11 | `ICON_APP_CALENDAR` | **Calendar** | Core App | Date & Events Calendar |
| 12 | `ICON_APP_CLOCK` | **Clock** | Core App | Alarms & World Clock |
| 13 | `ICON_APP_NOTES` | **Notes** | Core App | Quick Notes & Sticky Memos |
| 14 | `ICON_APP_ARCHIVE` | **Archive Manager** | Core App | Zip/Tar Archive Utility |
| 15 | `ICON_APP_IMAGE_VIEWER` | **Image Viewer** | Core App | Photo & Graphic Viewer |
| 16 | `ICON_APP_MUSIC` | **Music Player** | Core App | Audio & Music Player |
| 17 | `ICON_APP_VIDEO` | **Video Player** | Core App | Media & Video Player |
| 18 | `ICON_APP_CAMERA` | **Camera** | Core App | Camera & Video Capture |
| 19 | `ICON_APP_MAIL` | **Mail** | Core App | Electronic Mail Client |
| 20 | `ICON_APP_DOCUMENT` | **Document Viewer** | Core App | Formatted Document Viewer |
| 21 | `ICON_APP_PDF` | **PDF Viewer** | Core App | Portable Document Viewer |
| 22 | `ICON_APP_FONT_VIEWER` | **Font Viewer** | Core App | Noto Typography Inspector |
| 23 | `ICON_APP_DISK_MANAGER` | **Disk Manager** | Core App | GPT & Storage Partitioner |
| 24 | `ICON_APP_TASK_MANAGER` | **Task Manager** | Core App | Process Scheduler Inspector |
| 25 | `ICON_APP_LOG_VIEWER` | **Log Viewer** | Core App | Kernel Ring Buffer Viewer |
| 26 | `ICON_APP_NETWORK_MANAGER` | **Network Manager** | Core App | Interfaces & Wi-Fi Settings |
| 27 | `ICON_APP_BLUETOOTH_MANAGER` | **Bluetooth** | Core App | Bluetooth Device Manager |
| 28 | `ICON_APP_DISPLAY_SETTINGS` | **Display Settings** | Core App | Resolution & Refresh Config |
| 29 | `ICON_APP_ACCESSIBILITY` | **Accessibility** | Core App | Assistive Display Options |
| 30 | `ICON_APP_BACKUP` | **Backup & Restore** | Core App | Volume Snapshot & Restore |
| 31 | `ICON_APP_UPDATES` | **Software Updates** | Core App | Package Update Manager |
| 32 | `ICON_APP_SECURITY_AEGIS` | **AegisDetect** | Core App | Hardware Root of Trust |
| 33 | `ICON_APP_POWER_SESSION` | **Power & Session** | Core App | Shutdown & Restart Dialog |
| 34 | `ICON_UTIL_BUNDLE_INSPECTOR` | **Bundle Inspector** | Developer | Inspect .apapplication packages |
| 35 | `ICON_UTIL_APDRV_INSPECTOR` | **APDRV Inspector** | Developer | Inspect .apdrv driver packages |
| 36 | `ICON_UTIL_DRIVER_MANAGER` | **Driver Manager** | Developer | PCI Device & Driver Matcher |
| 37 | `ICON_UTIL_KERNEL_INFO` | **Kernel Info** | Developer | Aegis64 Version & Architecture |
| 38 | `ICON_UTIL_BOOT_CONFIG` | **Boot Config** | Developer | UEFI & Multiboot Options |
| 39 | `ICON_UTIL_STORAGE_DIAG` | **Storage Diag** | Developer | AHCI/NVMe SMART Diagnostics |
| 40 | `ICON_UTIL_MEMORY_DIAG` | **Memory Diag** | Developer | PMM/VMM Page Table Diagnostics |
| 41 | `ICON_UTIL_HARDWARE_INFO` | **Hardware Info** | Developer | PCI & ACPI Topology Map |
| 42 | `ICON_UTIL_PERF_MONITOR` | **Perf Monitor** | Developer | Throughput & GPU Benchmarks |
| 43 | `ICON_UTIL_DEBUG_CONSOLE` | **Debug Console** | Developer | Serial & Trace Diagnostic Port |
| 44 | `ICON_FILE_FOLDER` | **Folder** | File Type | Standard Directory Folder |
| 45 | `ICON_FILE_FOLDER_OPEN` | **Open Folder** | File Type | Currently Open Folder |
| 46 | `ICON_FILE_FOLDER_HOME` | **Home Folder** | File Type | User Home Directory |
| 47 | `ICON_FILE_FOLDER_DESKTOP` | **Desktop Folder** | File Type | Desktop Workspace Directory |
| 48 | `ICON_FILE_FOLDER_DOCS` | **Documents** | File Type | User Documents Directory |
| 49 | `ICON_FILE_FOLDER_DOWNLOADS` | **Downloads** | File Type | Downloaded Files Directory |
| 50 | `ICON_FILE_TRASH` | **Trash** | File Type | Recycle & Waste Basket |
| 51 | `ICON_FILE_EXEC` | **Executable** | File Type | Native Binary Program |
| 52 | `ICON_FILE_APAPP` | **Application Bundle** | File Type | Packaged .apapplication Bundle |
| 53 | `ICON_FILE_APDRV` | **Driver Package** | File Type | Packaged .apdrv Driver Bundle |
| 54 | `ICON_FILE_ARCHIVE` | **Archive File** | File Type | Compressed Archive (Zip/Tar) |
| 55 | `ICON_FILE_TEXT` | **Text File** | File Type | Plain Text Document (.txt) |
| 56 | `ICON_FILE_CODE` | **Source Code** | File Type | C/ASM Source File (.c/.h/.S) |
| 57 | `ICON_FILE_CONFIG` | **Config File** | File Type | Settings Configuration (.cfg/.ini) |
| 58 | `ICON_FILE_LOG` | **Log File** | File Type | System Diagnostic Log (.log) |
| 59 | `ICON_FILE_IMAGE` | **Image File** | File Type | Raster Image File (.png/.bmp) |
| 60 | `ICON_FILE_AUDIO` | **Audio File** | File Type | Sound Recording (.wav/.pcm) |
| 61 | `ICON_FILE_VIDEO` | **Video File** | File Type | Video Media Stream (.mp4/.avi) |
| 62 | `ICON_FILE_UNKNOWN` | **Unknown File** | File Type | Generic Unknown Data File |
| 63 | `ICON_DEV_DISK` | **Fixed Disk** | Hardware | Internal AHCI/NVMe Solid Drive |
| 64 | `ICON_DEV_REMOVABLE` | **USB Drive** | Hardware | Removable Mass Storage Unit |
| 65 | `ICON_DEV_NET_SHARE` | **Network Share** | Hardware | Remote Shared Network Volume |
| 66 | `ICON_DEV_EFI` | **EFI Partition** | Hardware | UEFI System Boot Partition |
| 67 | `ICON_DEV_PURPLEFS` | **PurpleFS Volume** | Hardware | PurpleFS v2/v3 Journaled Volume |
| 68 | `ICON_DEV_KEYBOARD` | **Keyboard** | Hardware | PS/2 or USB Input Keyboard |
| 69 | `ICON_DEV_MOUSE` | **Mouse Pointer** | Hardware | Pointing Device / Trackpad |
| 70 | `ICON_DEV_DISPLAY` | **Display Monitor** | Hardware | GOP Framebuffer Monitor |
| 71 | `ICON_DEV_CPU` | **CPU Processor** | Hardware | x86-64 Multi-Core SMP CPU |
| 72 | `ICON_DEV_GPU` | **GPU Accelerator** | Hardware | Intel BCS / AMD SDMA GPU |
| 73 | `ICON_DEV_MEMORY` | **System RAM** | Hardware | Physical Memory Bank |
| 74 | `ICON_DEV_NETWORK` | **Ethernet NIC** | Hardware | e1000 Gigabit Network Adapter |
| 75 | `ICON_DEV_AUDIO` | **Audio Output** | Hardware | HD Audio Sound Controller |
| 76 | `ICON_STATUS_WIFI_3` | **Wi-Fi High** | Status/UI | Wi-Fi Full Signal Strength |
| 77 | `ICON_STATUS_WIFI_2` | **Wi-Fi Medium** | Status/UI | Wi-Fi Moderate Signal |
| 78 | `ICON_STATUS_WIFI_1` | **Wi-Fi Low** | Status/UI | Wi-Fi Weak Signal |
| 79 | `ICON_STATUS_ETHERNET` | **Ethernet Connected** | Status/UI | Wired LAN Connection Active |
| 80 | `ICON_STATUS_BLUETOOTH` | **Bluetooth On** | Status/UI | Bluetooth Radio Connected |
| 81 | `ICON_STATUS_AUDIO` | **Volume Active** | Status/UI | Audio Playback Enabled |
| 82 | `ICON_STATUS_AUDIO_MUTE` | **Audio Muted** | Status/UI | Audio Output Silenced |
| 83 | `ICON_STATUS_BATTERY_100` | **Battery Full** | Status/UI | Battery Level 100% |
| 84 | `ICON_STATUS_BATTERY_CHARGING` | **Battery Charging** | Status/UI | AC Power Adapter Connected |
| 85 | `ICON_STATUS_BRIGHTNESS` | **Display Brightness** | Status/UI | Screen Luminance Level |
| 86 | `ICON_STATUS_VOLUME` | **Volume Slider** | Status/UI | Audio Output Master Level |
| 87 | `ICON_STATUS_SECURITY_OK` | **Security Verified** | Status/UI | AegisDetect Root of Trust Clean |
| 88 | `ICON_STATUS_SECURITY_WARN` | **Security Warning** | Status/UI | Integrity Check Discrepancy |
| 89 | `ICON_STATUS_POWER` | **Power State** | Status/UI | System ACPI Power Status |
| 90 | `ICON_UI_CLOSE` | **Close Button** | Status/UI | Window Close Control [X] |
| 91 | `ICON_UI_MINIMIZE` | **Minimize Button** | Status/UI | Window Minimize to Dock [-] |
| 92 | `ICON_UI_MAXIMIZE` | **Maximize Button** | Status/UI | Window Maximize [ + ] |
| 93 | `ICON_UI_RESTORE` | **Restore Button** | Status/UI | Window Restore Dimensions |
| 94 | `ICON_UI_SEARCH` | **Search Action** | Status/UI | Search Query Icon |
| 95 | `ICON_UI_REFRESH` | **Refresh Action** | Status/UI | Reload Directory / Window |
| 96 | `ICON_UI_BACK` | **Navigate Back** | Status/UI | History Back Arrow |
| 97 | `ICON_UI_FORWARD` | **Navigate Forward** | Status/UI | History Forward Arrow |
| 98 | `ICON_UI_HELP` | **Help Action** | Status/UI | Interactive Help Dialog |
| 99 | `ICON_UI_CHECK` | **Verification Check** | Status/UI | Validation Passed Checkmark |
| 100 | `ICON_UI_STAR` | **Favorite Star** | Status/UI | Favorite Item Bookmark ★ |
