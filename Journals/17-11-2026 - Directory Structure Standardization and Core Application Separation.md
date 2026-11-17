# Engineering Journal: Directory Structure Standardization and Application Isolation

**Date:** November 17, 2026  
**Subsystems Affected:** PurpleFS Inode Routing (`purplefs_v3.rs`), System Documentation (`APAPPLICATION_SPECIFICATION.md`, `PURPLE_OS_DIRECTORY_STRUCTURE_SPECIFICATION.md`, `PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`, `PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.md`).

---

## 1. Context & Architectural Requirements

1. **Elimination of Conflicting Paths:**
   - Any historical or proposed references to `/System/Kernel/Applications` were eliminated. The kernel core `/System/Kernel/` is strictly reserved for the low-level bare-metal monolithic binary (`aegis64.elf`) and symbol exports (`kernel.sym`).
2. **Core System Application Placement:**
   - Standard built-in applications (`PurpleTerminal.apapplication`, `PurpleFiles.apapplication`, `PurpleSettings.apapplication`, `PurpleViewer.apapplication`, `SysMon.apapplication`, `DiskMgr.apapplication`) reside in `/System/Applications/`.
3. **User-Installed Application Placement:**
   - Userland applications installed by interactive users reside in `/System/Users/<user>/Applications/` (e.g. `/System/Users/aegis/Applications/`).
4. **Filesystem Lookup Normalization:**
   - Updated `src/fs/purplefs_v3.rs` path lookup logic to transparently recognize `/System/Users/aegis/welcome.txt` alongside legacy flat aliases.

---

## 2. Updated Deliverables & Git Records

* **New Specification:** `docs/PURPLE_OS_DIRECTORY_STRUCTURE_SPECIFICATION.md` defining the complete on-disk directory tree, application discovery rules, and GPT partition bindings.
* **Updated Documentation:**
  - `docs/APAPPLICATION_SPECIFICATION.md`: Application discovery search order and permissions model.
  - `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`: Multi-file coexistence and block consumption across `/System/Applications` and `/System/Users`.
  - `docs/PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.md`: System layout consistency.
* **Commits:**
  - `15723ff`: Multi-core TSS, SMP scheduler, MMIO caching, fast video console, and AegisDetect SHA-256 verification.
  - `c2787df`: PurpleDetect hardware inventory registry, VMM lock recursion fix, and ACPI parser hardening.
  - `2ed32d0`: Formalization of canonical directory structure with `/System/Applications` and `/System/Users/<user>/Applications`.
* **Remote State:** Fully pushed and synchronized with `origin/Aegis_Purple`.
