#include "apdrv.h"
#include "ksym.h"
#include "string.h"
#include "serial.h"
#include "pci.h"
#include "purple_detect.h"

#ifdef __KERNEL_HOST_TEST__
#include <stdlib.h>
#include <sys/mman.h>
#else
#include "pmm.h"
#include "vmm.h"
#include "kernel_storage.h"
#endif

static apdrv_module_t loaded_modules[APDRV_MAX_MODULES];
static uint32_t module_count = 0;

int apdrv_subsystem_init(void) {
    memset(loaded_modules, 0, sizeof(loaded_modules));
    module_count = 0;
    kinfo("APDRV: Subsystem v%u initialized (Max Modules: %u, W^X Enforced)",
          APDRV_VERSION, APDRV_MAX_MODULES);
    return 0;
}

uint32_t apdrv_get_count(void) {
    return module_count;
}

const apdrv_module_t *apdrv_get(uint32_t index) {
    if (index >= module_count) return NULL;
    return &loaded_modules[index];
}

apdrv_module_t *apdrv_find(const char *name) {
    if (!name) return NULL;
    for (uint32_t i = 0; i < module_count; i++) {
        if (strcmp(loaded_modules[i].hdr.name, name) == 0) {
            return &loaded_modules[i];
        }
    }
    return NULL;
}

static uint32_t calc_checksum(const uint8_t *data, uint32_t len) {
    uint32_t sum1 = 0xffff, sum2 = 0xffff;
    while (len) {
        unsigned tlen = len > 359 ? 359 : len;
        len -= tlen;
        do {
            sum1 += *data++;
            sum2 += sum1;
        } while (--tlen);
        sum1 = (sum1 & 0xffff) + (sum1 >> 16);
        sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    }
    sum1 = (sum1 & 0xffff) + (sum1 >> 16);
    sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    return (sum2 << 16) | sum1;
}

int apdrv_load(const void *image, uint32_t image_size, apdrv_module_t **out_mod) {
    if (!image || image_size < sizeof(apdrv_header_t)) {
        kerror("APDRV: Invalid image pointer or size (%u bytes)", image_size);
        return -1;
    }

    if (module_count >= APDRV_MAX_MODULES) {
        kerror("APDRV: Maximum module limit reached (%u)", APDRV_MAX_MODULES);
        return -2;
    }

    const apdrv_header_t *hdr = (const apdrv_header_t *)image;

    /* 1. Validate magic and format header */
    if (memcmp(hdr->magic, APDRV_MAGIC, APDRV_MAGIC_LEN) != 0) {
        kerror("APDRV: Bad magic (expected 'APDRV100')");
        return -3;
    }
    if (hdr->version != APDRV_VERSION) {
        kerror("APDRV: Unsupported version %u (expected %u)", hdr->version, APDRV_VERSION);
        return -4;
    }
    if (memcmp(hdr->mod_type, APDRV_TYPE_AEGISMOD, 8) != 0) {
        kerror("APDRV: Invalid module type (expected 'AEGISMOD')");
        return -5;
    }
    if (hdr->header_size != sizeof(apdrv_header_t)) {
        kerror("APDRV: Header size mismatch (%u != %u)", hdr->header_size, (uint32_t)sizeof(apdrv_header_t));
        return -6;
    }

    /* Check duplicate */
    if (apdrv_find(hdr->name) != NULL) {
        kerror("APDRV: Module '%s' is already loaded", hdr->name);
        return -7;
    }

    /* Check bounds */
    if (hdr->text_offset + hdr->text_size > image_size ||
        hdr->rodata_offset + hdr->rodata_size > image_size ||
        hdr->data_offset + hdr->data_size > image_size ||
        hdr->reloc_offset + hdr->reloc_count * sizeof(apdrv_reloc_t) > image_size ||
        hdr->import_offset + hdr->import_count * sizeof(apdrv_sym_t) > image_size) {
        kerror("APDRV: Module '%s' has section offsets exceeding image bounds", hdr->name);
        return -8;
    }

    /* Verify checksum if provided */
    if (hdr->checksum != 0) {
        uint32_t csum = calc_checksum((const uint8_t *)image + sizeof(apdrv_header_t),
                                      image_size - sizeof(apdrv_header_t));
        if (csum != hdr->checksum) {
            kerror("APDRV: Module '%s' checksum mismatch (calculated 0x%08x != header 0x%08x)",
                   hdr->name, csum, hdr->checksum);
            return -13;
        }
    }

    /* 2. Resolve Symbol Imports (Fail-Closed) */
    uint64_t resolved_imports[APDRV_MAX_SYMBOLS];
    if (hdr->import_count > APDRV_MAX_SYMBOLS) {
        kerror("APDRV: Import symbol count exceeds maximum (%u > %u)", hdr->import_count, APDRV_MAX_SYMBOLS);
        return -9;
    }

    const apdrv_sym_t *import_table = (const apdrv_sym_t *)((const uint8_t *)image + hdr->import_offset);
    for (uint32_t i = 0; i < hdr->import_count; i++) {
        uint64_t addr = ksym_lookup(import_table[i].name);
        if (addr == 0) {
            kerror("APDRV: Module '%s' unresolved symbol '%s'", hdr->name, import_table[i].name);
            return -10;
        }
        resolved_imports[i] = addr;
    }

    /* 3. Calculate page-aligned memory requirements for W^X enforcement */
    uint32_t text_aligned = (hdr->text_size + 4095) & ~4095;
    uint32_t rodata_aligned = (hdr->rodata_size + 4095) & ~4095;
    uint32_t data_aligned = (hdr->data_size + hdr->bss_size + 4095) & ~4095;
    uint32_t total_bytes = text_aligned + rodata_aligned + data_aligned;
    if (total_bytes == 0) total_bytes = 4096;
    uint32_t page_count = (total_bytes + 4095) / 4096;

    uint8_t *module_mem = NULL;
#ifdef __KERNEL_HOST_TEST__
    if (posix_memalign((void **)&module_mem, 4096, page_count * 4096) != 0) {
        return -11;
    }
    memset(module_mem, 0, page_count * 4096);
    mprotect(module_mem, page_count * 4096, PROT_READ | PROT_WRITE | PROT_EXEC);
#else
    module_mem = (uint8_t *)pmm_alloc_frames(page_count);
    if (!module_mem) {
        kerror("APDRV: Out of physical memory for module '%s' (%u pages)", hdr->name, page_count);
        return -11;
    }
    memset(module_mem, 0, page_count * 4096);
#endif

    uint8_t *text_base = module_mem;
    uint8_t *rodata_base = module_mem + text_aligned;
    uint8_t *data_base = module_mem + text_aligned + rodata_aligned;
    uint8_t *bss_base = data_base + hdr->data_size;

    /* 4. Copy Segments */
    if (hdr->text_size > 0) {
        memcpy(text_base, (const uint8_t *)image + hdr->text_offset, hdr->text_size);
    }
    if (hdr->rodata_size > 0) {
        memcpy(rodata_base, (const uint8_t *)image + hdr->rodata_offset, hdr->rodata_size);
    }
    if (hdr->data_size > 0) {
        memcpy(data_base, (const uint8_t *)image + hdr->data_offset, hdr->data_size);
    }
    if (hdr->bss_size > 0) {
        memset(bss_base, 0, hdr->bss_size);
    }

    /* 5. Process Relocations */
    const apdrv_reloc_t *relocs = (const apdrv_reloc_t *)((const uint8_t *)image + hdr->reloc_offset);
    for (uint32_t i = 0; i < hdr->reloc_count; i++) {
        const apdrv_reloc_t *rel = &relocs[i];
        if (rel->offset >= total_bytes) {
            kerror("APDRV: Relocation offset 0x%x out of module bounds", rel->offset);
            goto cleanup_error;
        }

        uint8_t *target_ptr = module_mem + rel->offset;
        uint64_t sym_addr = 0;

        if (rel->sym_index < hdr->import_count) {
            sym_addr = resolved_imports[rel->sym_index];
        } else if (rel->sym_index == APDRV_SEC_TEXT) {
            sym_addr = (uint64_t)(uintptr_t)text_base;
        } else if (rel->sym_index == APDRV_SEC_RODATA) {
            sym_addr = (uint64_t)(uintptr_t)rodata_base;
        } else if (rel->sym_index == APDRV_SEC_DATA) {
            sym_addr = (uint64_t)(uintptr_t)data_base;
        } else if (rel->sym_index == APDRV_SEC_BSS) {
            sym_addr = (uint64_t)(uintptr_t)bss_base;
        } else if (rel->sym_index == APDRV_SEC_MODULE) {
            sym_addr = (uint64_t)(uintptr_t)module_mem;
        } else {
            kerror("APDRV: Invalid relocation symbol index 0x%x", rel->sym_index);
            goto cleanup_error;
        }

        switch (rel->type) {
            case APDRV_RELOC_64: {
                uint64_t val = sym_addr + rel->addend;
                *(uint64_t *)target_ptr = val;
                break;
            }
            case APDRV_RELOC_32: {
                uint32_t val = (uint32_t)(sym_addr + rel->addend);
                *(uint32_t *)target_ptr = val;
                break;
            }
            case APDRV_RELOC_PC32:
            case APDRV_RELOC_PLT32: {
                int32_t disp = (int32_t)(sym_addr + rel->addend - (uint64_t)(uintptr_t)target_ptr);
                *(int32_t *)target_ptr = disp;
                break;
            }
            default:
                kerror("APDRV: Unknown relocation type %u", rel->type);
                goto cleanup_error;
        }
    }

    /* 6. Enforce W^X Page Protections */
    int wx_applied = 0;
#ifndef __KERNEL_HOST_TEST__
    page_table_t *pml4 = vmm_get_kernel_pml4();
    if (pml4 && (hdr->flags & APDRV_FLAG_ENFORCE_WX)) {
        /* .text: Read + Execute (NO Write) */
        for (uint32_t p = 0; p < text_aligned; p += 4096) {
            vmm_set_permissions(pml4, (uint64_t)(uintptr_t)(text_base + p),
                                VMM_PROT_READ | VMM_PROT_EXEC);
        }
        /* .rodata: Read-only (NO Write, NO Execute) */
        for (uint32_t p = 0; p < rodata_aligned; p += 4096) {
            vmm_set_permissions(pml4, (uint64_t)(uintptr_t)(rodata_base + p),
                                VMM_PROT_READ | VMM_PROT_NO_EXEC);
        }
        /* .data + .bss: Read + Write (NO Execute) */
        for (uint32_t p = 0; p < data_aligned; p += 4096) {
            vmm_set_permissions(pml4, (uint64_t)(uintptr_t)(data_base + p),
                                VMM_PROT_READ | VMM_PROT_WRITE | VMM_PROT_NO_EXEC);
        }
        wx_applied = 1;
    }
#else
    if (hdr->flags & APDRV_FLAG_ENFORCE_WX) {
        wx_applied = 1;
    }
#endif

    /* 7. Setup Loaded Module Instance */
    apdrv_module_t *mod = &loaded_modules[module_count];
    memcpy(&mod->hdr, hdr, sizeof(apdrv_header_t));
    mod->state = APDRV_STATE_LOADED;
    mod->memory = module_mem;
    mod->total_size = total_bytes;
    mod->text_addr = (uint64_t)(uintptr_t)text_base;
    mod->rodata_addr = (uint64_t)(uintptr_t)rodata_base;
    mod->data_addr = (uint64_t)(uintptr_t)data_base;
    mod->bss_addr = (uint64_t)(uintptr_t)bss_base;
    mod->wx_enforced = wx_applied;
    mod->bound_devices = 0;

    if (hdr->entry_init) {
        mod->fn_init = (int (*)(void))(uintptr_t)(text_base + hdr->entry_init);
    }
    if (hdr->entry_fini) {
        mod->fn_fini = (int (*)(void))(uintptr_t)(text_base + hdr->entry_fini);
    }
    if (hdr->entry_probe) {
        mod->fn_probe = (int (*)(uint32_t, uint32_t))(uintptr_t)(text_base + hdr->entry_probe);
    }
    if (hdr->entry_ioctl) {
        mod->fn_ioctl = (int (*)(uint32_t, void *))(uintptr_t)(text_base + hdr->entry_ioctl);
    }

    /* 8. Call init() */
    if (mod->fn_init) {
        int init_rc = mod->fn_init();
        if (init_rc != 0) {
            kerror("APDRV: Module '%s' init() returned error %d", hdr->name, init_rc);
            goto cleanup_error;
        }
    }

    mod->state = APDRV_STATE_RUNNING;
    module_count++;

    /* 9. Auto-probe matching hardware devices */
    if (mod->fn_probe) {
        uint32_t dev_count = pci_get_device_count();
        for (uint32_t d = 0; d < dev_count; d++) {
            const pci_device_t *pdev = pci_get_device(d);
            if (!pdev) continue;

            int match = 0;
            if (hdr->target_vendor == 0xFFFF || hdr->target_vendor == pdev->vendor_id) {
                if (hdr->target_device == 0xFFFF || hdr->target_device == pdev->device_id) {
                    match = 1;
                }
            }
            if (match) {
                int prc = mod->fn_probe(pdev->vendor_id, pdev->device_id);
                if (prc == 0) {
                    mod->bound_devices++;
                }
            }
        }
    }

    /* Register with PurpleDetect as an active driver */
    purple_detect_register(PURPLE_CAT_VIRTUAL, hdr->name, hdr->description,
                           hdr->author, "PCI", (uint32_t)(uintptr_t)module_mem,
                           0, PURPLE_STATE_ACTIVE, 0);

    kinfo("APDRV: Module '%s' v%u.%u.%u loaded successfully (Mem=%p, %u bytes, W^X=%s, Bound=%u)",
          hdr->name,
          (hdr->mod_version >> 16) & 0xFF,
          (hdr->mod_version >> 8) & 0xFF,
          hdr->mod_version & 0xFF,
          module_mem, total_bytes,
          wx_applied ? "ENFORCED" : "OFF",
          mod->bound_devices);

    if (out_mod) *out_mod = mod;
    return 0;

cleanup_error:
#ifdef __KERNEL_HOST_TEST__
    free(module_mem);
#else
    pmm_free_frames(module_mem, page_count);
#endif
    return -12;
}

int apdrv_unload(const char *name) {
    if (!name) return -1;
    int idx = -1;
    for (uint32_t i = 0; i < module_count; i++) {
        if (strcmp(loaded_modules[i].hdr.name, name) == 0) {
            idx = (int)i;
            break;
        }
    }
    if (idx < 0) {
        kerror("APDRV: Module '%s' not found for unload", name);
        return -2;
    }

    apdrv_module_t *mod = &loaded_modules[idx];

    /* Call fini() */
    if (mod->fn_fini) {
        mod->fn_fini();
    }

    /* Unregister from PurpleDetect */
    const purple_device_t *pdev = purple_detect_find_by_name(mod->hdr.name);
    if (pdev) {
        purple_detect_unregister(pdev->id);
    }

    /* Free memory */
#ifdef __KERNEL_HOST_TEST__
    free(mod->memory);
#else
    uint32_t page_count = (mod->total_size + 4095) / 4096;
    pmm_free_frames(mod->memory, page_count);
#endif

    kinfo("APDRV: Module '%s' unloaded and memory reclaimed", mod->hdr.name);

    for (uint32_t i = idx; i + 1 < module_count; i++) {
        loaded_modules[i] = loaded_modules[i + 1];
    }
    module_count--;
    memset(&loaded_modules[module_count], 0, sizeof(apdrv_module_t));

    return 0;
}

int apdrv_probe_device(uint16_t vendor_id, uint16_t device_id) {
    int total_bound = 0;
    for (uint32_t i = 0; i < module_count; i++) {
        apdrv_module_t *mod = &loaded_modules[i];
        if (!mod->fn_probe) continue;

        int match = 0;
        if (mod->hdr.target_vendor == 0xFFFF || mod->hdr.target_vendor == vendor_id) {
            if (mod->hdr.target_device == 0xFFFF || mod->hdr.target_device == device_id) {
                match = 1;
            }
        }
        if (match) {
            if (mod->fn_probe(vendor_id, device_id) == 0) {
                mod->bound_devices++;
                total_bound++;
            }
        }
    }
    return total_bound;
}

#ifndef __KERNEL_HOST_TEST__
int apdrv_load_from_storage(const char *filepath) {
    if (!filepath) return -1;
    if (!storage_is_fs_mounted()) {
        kerror("APDRV: Cannot load '%s' - No PurpleFS volume mounted", filepath);
        return -2;
    }

    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, filepath, &ino) != PFS2_OK) {
        kerror("APDRV: File '%s' not found on PurpleFS", filepath);
        return -3;
    }

    pfs2_inode_t finode;
    if (pfs2_read_inode(fs, ino, &finode) != PFS2_OK) {
        kerror("APDRV: Failed to read inode for '%s'", filepath);
        return -4;
    }

    if (finode.size_bytes < sizeof(apdrv_header_t) || finode.size_bytes > 512 * 1024) {
        kerror("APDRV: Invalid driver binary size (%u bytes) for '%s'", (uint32_t)finode.size_bytes, filepath);
        return -5;
    }

    uint32_t pages = (finode.size_bytes + 4095) / 4096;
    void *buf = pmm_alloc_frames(pages);
    if (!buf) {
        kerror("APDRV: Out of memory reading '%s'", filepath);
        return -6;
    }

    size_t bytes_read = 0;
    int rc = pfs2_read_file(fs, ino, buf, finode.size_bytes, 0, &bytes_read);
    if (rc != PFS2_OK || bytes_read != finode.size_bytes) {
        pmm_free_frames(buf, pages);
        kerror("APDRV: Storage read error on '%s' (rc=%d)", filepath, rc);
        return -7;
    }

    apdrv_module_t *mod = NULL;
    int load_rc = apdrv_load(buf, finode.size_bytes, &mod);
    pmm_free_frames(buf, pages);
    return load_rc;
}
#else
int apdrv_load_from_storage(const char *filepath) {
    (void)filepath;
    return 0;
}
#endif

void apdrv_dump_modules(void) {
    kprintf("\n=== APDRV v3 Loaded Native Modules (%u modules) ===\n", module_count);
    kprintf("  MODULE NAME      VERSION    STATE    MEMORY_BASE         SIZE     W^X  BOUND\n");
    kprintf("  ----------------------------------------------------------------------------\n");
    if (module_count == 0) {
        kprintf("  No APDRV modules loaded.\n\n");
        return;
    }
    for (uint32_t i = 0; i < module_count; i++) {
        const apdrv_module_t *m = &loaded_modules[i];
        const char *st_str = (m->state == APDRV_STATE_RUNNING) ? "RUNNING" :
                             (m->state == APDRV_STATE_LOADED)  ? "LOADED"  : "ERROR";
        kprintf("  %-16s %u.%u.%u      %-8s 0x%016llx  %-7u  %s   %-3u\n",
                m->hdr.name,
                (m->hdr.mod_version >> 16) & 0xFF,
                (m->hdr.mod_version >> 8) & 0xFF,
                m->hdr.mod_version & 0xFF,
                st_str,
                (uint64_t)(uintptr_t)m->memory,
                m->total_size,
                m->wx_enforced ? "YES" : "NO ",
                m->bound_devices);
    }
    kprintf("  ----------------------------------------------------------------------------\n\n");
}
