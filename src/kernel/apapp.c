#include "apapp.h"
#include "crypto/sha256.h"
#include "string.h"
#include "serial.h"
#include "desktop.h"
#include "task.h"

#ifdef __KERNEL_HOST_TEST__
#include <stdlib.h>
#else
#include "pmm.h"
#include "kernel_storage.h"
#endif

static apapp_instance_t registered_apps[APAPP_MAX_APPS];
static uint32_t app_count = 0;

static const uint8_t app_trust_key[] = "PurpleOS-Aegis64-TrustAnchor-2026";

int apapp_subsystem_init(void) {
    memset(registered_apps, 0, sizeof(registered_apps));
    app_count = 0;
    kinfo("APAPP: Application Bundle Subsystem initialized (Max Apps: %u, Security=AegisDetect)",
          APAPP_MAX_APPS);
    return APAPP_OK;
}

uint32_t apapp_get_count(void) {
    return app_count;
}

const apapp_instance_t *apapp_get_instance(uint32_t index) {
    if (index >= app_count) return NULL;
    return &registered_apps[index];
}

const apapp_instance_t *apapp_find(const char *app_id) {
    if (!app_id) return NULL;
    for (uint32_t i = 0; i < app_count; i++) {
        if (strcmp(registered_apps[i].manifest.app_id, app_id) == 0) {
            return &registered_apps[i];
        }
    }
    return NULL;
}

int apapp_parse_manifest(const void *data, uint32_t size, apapp_manifest_t *out_manifest) {
    if (!data || size < sizeof(apapp_manifest_t) || !out_manifest) {
        return APAPP_ERR_INVALID;
    }

    const apapp_manifest_t *m = (const apapp_manifest_t *)data;

    if (memcmp(m->magic, APAPP_MAGIC, APAPP_MAGIC_LEN) != 0) {
        kerror("APAPP: Invalid bundle magic (expected 'APAPP100')");
        return APAPP_ERR_MAGIC;
    }
    if (m->version != APAPP_VERSION) {
        kerror("APAPP: Unsupported bundle version %u", m->version);
        return APAPP_ERR_VERSION;
    }
    if (m->manifest_size != sizeof(apapp_manifest_t)) {
        kerror("APAPP: Manifest size mismatch");
        return APAPP_ERR_INVALID;
    }

    memcpy(out_manifest, m, sizeof(apapp_manifest_t));
    return APAPP_OK;
}

int apapp_verify_security(const apapp_manifest_t *manifest, const void *payload, uint32_t payload_len) {
    if (!manifest) return APAPP_ERR_INVALID;

    /* Compute HMAC-SHA256 over manifest fields + executable payload */
    uint8_t expected_mac[SHA256_DIGEST_SIZE];

    sha256_ctx_t ctx;
    uint8_t k_pad[SHA256_BLOCK_SIZE];
    uint8_t inner[SHA256_DIGEST_SIZE];
    size_t key_len = strlen((const char *)app_trust_key);

    /* Inner hash: H((K ^ ipad) || metadata || payload) */
    memset(k_pad, 0x36, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= app_trust_key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    /* Hash header up to hmac_signature field */
    size_t header_len = (size_t)((uintptr_t)manifest->hmac_signature - (uintptr_t)manifest);
    sha256_update(&ctx, manifest, header_len);
    if (payload && payload_len > 0) {
        sha256_update(&ctx, payload, payload_len);
    }
    sha256_final(&ctx, inner);

    /* Outer hash: H((K ^ opad) || inner) */
    memset(k_pad, 0x5c, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= app_trust_key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    sha256_update(&ctx, inner, SHA256_DIGEST_SIZE);
    sha256_final(&ctx, expected_mac);

    if (memcmp(expected_mac, manifest->hmac_signature, SHA256_DIGEST_SIZE) != 0) {
        kerror("AegisDetect: Security violation! Application bundle '%s' HMAC signature mismatch",
               manifest->app_id);
        return APAPP_ERR_SIGNATURE;
    }

    kinfo("AegisDetect: Application bundle '%s' AUTHENTICATED (Trust Anchor verified)",
          manifest->app_id);
    return APAPP_OK;
}

int apapp_launch(const apapp_manifest_t *manifest,
                 const void *payload,
                 uint32_t payload_len,
                 app_entry_fn entry,
                 apapp_instance_t **out_instance) {
    if (!manifest) return APAPP_ERR_INVALID;

    if (app_count >= APAPP_MAX_APPS) {
        kerror("APAPP: Maximum running applications limit reached (%u)", APAPP_MAX_APPS);
        return APAPP_ERR_CAPACITY;
    }

    if (apapp_find(manifest->app_id) != NULL) {
        kerror("APAPP: Application '%s' is already running", manifest->app_id);
        return APAPP_ERR_EXISTS;
    }

    /* 1. Security Verification (AegisDetect root-of-trust) */
    int sec_rc = apapp_verify_security(manifest, payload, payload_len);
    if (sec_rc != APAPP_OK) {
        return sec_rc; /* Fail-closed */
    }

    /* 2. Desktop GUI Window Allocation */
    int win_id = -1;
    if (manifest->permissions & APAPP_PERM_GUI) {
        uint32_t w = manifest->window_width ? manifest->window_width : 360;
        uint32_t h = manifest->window_height ? manifest->window_height : 180;
        uint32_t wx = 40 + (app_count * 30);
        uint32_t wy = 50 + (app_count * 25);
        win_id = desktop_create_window(manifest->app_name, wx, wy, w, h,
                                       COLOR_LIGHT_GRAY, COLOR_PURPLE_DARK);
    }

    /* 3. Task Creation under AegisSched */
    uint32_t tid = 0;
#ifndef __KERNEL_HOST_TEST__
    if (entry) {
        uint32_t prio = manifest->default_priority ? manifest->default_priority : 2;
        task_t *t = task_create(manifest->app_id, entry, NULL, (task_priority_t)prio);
        if (t) {
            tid = t->id;
        }
    }
#else
    (void)entry;
    tid = 100 + app_count;
#endif

    /* 4. Record Instance */
    apapp_instance_t *inst = &registered_apps[app_count];
    memcpy(&inst->manifest, manifest, sizeof(apapp_manifest_t));
    inst->state = APAPP_STATE_RUNNING;
    inst->task_id = tid;
    inst->window_id = win_id;
    inst->launch_tick = 0;
    inst->memory = NULL;
    inst->memory_size = 0;

    app_count++;

    kinfo("APAPP: Application '%s' ('%s' v%s) launched (PID=%u, WinID=%d)",
          manifest->app_id, manifest->app_name, manifest->version_str,
          tid, win_id);

    if (out_instance) *out_instance = inst;
    return APAPP_OK;
}

int apapp_terminate(const char *app_id) {
    if (!app_id) return APAPP_ERR_INVALID;
    int found_idx = -1;
    for (uint32_t i = 0; i < app_count; i++) {
        if (strcmp(registered_apps[i].manifest.app_id, app_id) == 0) {
            found_idx = (int)i;
            break;
        }
    }
    if (found_idx < 0) return APAPP_ERR_NOT_FOUND;

    apapp_instance_t *inst = &registered_apps[found_idx];

#ifndef __KERNEL_HOST_TEST__
    if (inst->task_id > 0) {
        task_kill(inst->task_id);
    }
#endif

    kinfo("APAPP: Terminated application '%s' (PID %u)", inst->manifest.app_id, inst->task_id);

    for (uint32_t i = found_idx; i + 1 < app_count; i++) {
        registered_apps[i] = registered_apps[i + 1];
    }
    app_count--;
    memset(&registered_apps[app_count], 0, sizeof(apapp_instance_t));

    return APAPP_OK;
}

#ifndef __KERNEL_HOST_TEST__
int apapp_launch_from_storage(const char *bundle_path, app_entry_fn fallback_entry) {
    if (!bundle_path) return APAPP_ERR_INVALID;
    if (!storage_is_fs_mounted()) {
        kerror("APAPP: Cannot load '%s' - No PurpleFS volume mounted", bundle_path);
        return APAPP_ERR_NOT_FOUND;
    }

    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, bundle_path, &ino) != PFS2_OK) {
        kerror("APAPP: Bundle '%s' not found on PurpleFS", bundle_path);
        return APAPP_ERR_NOT_FOUND;
    }

    pfs2_inode_t finode;
    if (pfs2_read_inode(fs, ino, &finode) != PFS2_OK) {
        kerror("APAPP: Failed to read inode for '%s'", bundle_path);
        return APAPP_ERR_INVALID;
    }

    if (finode.size_bytes < sizeof(apapp_manifest_t) || finode.size_bytes > 1024 * 1024) {
        kerror("APAPP: Invalid bundle size (%u bytes) for '%s'", (uint32_t)finode.size_bytes, bundle_path);
        return APAPP_ERR_INVALID;
    }

    uint32_t pages = (finode.size_bytes + 4095) / 4096;
    void *buf = pmm_alloc_frames(pages);
    if (!buf) {
        kerror("APAPP: Out of memory reading bundle '%s'", bundle_path);
        return APAPP_ERR_MEMORY;
    }

    size_t bytes_read = 0;
    int rc = pfs2_read_file(fs, ino, buf, finode.size_bytes, 0, &bytes_read);
    if (rc != PFS2_OK || bytes_read != finode.size_bytes) {
        pmm_free_frames(buf, pages);
        kerror("APAPP: Storage read error on '%s'", bundle_path);
        return APAPP_ERR_INVALID;
    }

    apapp_manifest_t manifest;
    int prc = apapp_parse_manifest(buf, finode.size_bytes, &manifest);
    if (prc != APAPP_OK) {
        pmm_free_frames(buf, pages);
        return prc;
    }

    const uint8_t *payload = (const uint8_t *)buf + manifest.exec_offset;
    uint32_t payload_len = manifest.exec_size;

    apapp_instance_t *inst = NULL;
    int lrc = apapp_launch(&manifest, payload, payload_len, fallback_entry, &inst);
    pmm_free_frames(buf, pages);
    return lrc;
}
#else
int apapp_launch_from_storage(const char *bundle_path, app_entry_fn fallback_entry) {
    (void)bundle_path;
    (void)fallback_entry;
    return APAPP_OK;
}
#endif

void apapp_dump(void) {
    kprintf("\n=== Active .apapplications Native Application Bundles (%u running) ===\n", app_count);
    kprintf("  APP_ID                  VERSION  PID   WIN_ID  PERMISSIONS  STATE\n");
    kprintf("  ------------------------------------------------------------------------\n");
    if (app_count == 0) {
        kprintf("  No application bundles currently running.\n\n");
        return;
    }
    for (uint32_t i = 0; i < app_count; i++) {
        const apapp_instance_t *a = &registered_apps[i];
        kprintf("  %-23s %-8s %-5u %-7d 0x%04x       %s\n",
                a->manifest.app_id,
                a->manifest.version_str,
                a->task_id,
                a->window_id,
                a->manifest.permissions,
                (a->state == APAPP_STATE_RUNNING) ? "RUNNING" : "STOPPED");
    }
    kprintf("  ------------------------------------------------------------------------\n\n");
}
