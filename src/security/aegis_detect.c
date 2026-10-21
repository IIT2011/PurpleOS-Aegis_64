#include "security/aegis_detect.h"
#include "crypto/sha256.h"
#include "string.h"
#include "serial.h"

static uint8_t active_auth_key[SHA256_BLOCK_SIZE];
static size_t active_key_len = 0;
static int manifest_loaded = 0;

static aegis_manifest_t current_manifest;
static aegis_verify_result_t verify_results[AEGIS_MAX_COMPONENTS];
static uint32_t verified_components_count = 0;

/* Default System Trust Anchor Key: "PurpleOS-Aegis64-TrustAnchor-2026" */
static const uint8_t default_trust_key[] = "PurpleOS-Aegis64-TrustAnchor-2026";

static void calculate_manifest_hmac(const aegis_manifest_t *m, const uint8_t *key, size_t key_len, uint8_t out_mac[32]) {
    sha256_ctx_t ctx;
    uint8_t k_pad[SHA256_BLOCK_SIZE];
    uint8_t inner[SHA256_DIGEST_SIZE];

    /* Inner hash: H((K ^ ipad) || header || entries) */
    memset(k_pad, 0x36, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    sha256_update(&ctx, m, sizeof(uint32_t) * 4); // magic, version, entry_count, reserved
    if (m->entry_count > 0) {
        sha256_update(&ctx, m->entries, sizeof(aegis_manifest_entry_t) * m->entry_count);
    }
    sha256_final(&ctx, inner);

    /* Outer hash: H((K ^ opad) || inner) */
    memset(k_pad, 0x5c, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    sha256_update(&ctx, inner, SHA256_DIGEST_SIZE);
    sha256_final(&ctx, out_mac);
}

void aegis_manifest_sign(aegis_manifest_t *m, const uint8_t *key, size_t key_len) {
    if (!m || !key || key_len == 0) return;
    calculate_manifest_hmac(m, key, key_len, m->auth_tag);
}

int aegis_detect_authenticate_manifest(const aegis_manifest_t *m, const uint8_t *key, size_t key_len) {
    if (!m || !key || key_len == 0) return -1;
    if (m->magic != AEGIS_MANIFEST_MAGIC || m->version != AEGIS_MANIFEST_VERSION) return -2;
    if (m->entry_count > AEGIS_MAX_COMPONENTS) return -3;

    uint8_t expected_mac[SHA256_DIGEST_SIZE];
    calculate_manifest_hmac(m, key, key_len, expected_mac);

    if (memcmp(m->auth_tag, expected_mac, SHA256_DIGEST_SIZE) != 0) {
        return -4; /* Authentication tag mismatch (tampered or untrusted manifest) */
    }
    return 0;
}

int aegis_detect_init(const uint8_t *auth_key, size_t key_len) {
    if (!auth_key || key_len == 0) {
        auth_key = default_trust_key;
        key_len = sizeof(default_trust_key) - 1;
    }

    if (key_len > sizeof(active_auth_key)) {
        key_len = sizeof(active_auth_key);
    }
    memcpy(active_auth_key, auth_key, key_len);
    active_key_len = key_len;
    manifest_loaded = 0;
    verified_components_count = 0;
    memset(verify_results, 0, sizeof(verify_results));

    kinfo("AegisDetect: Integrity Subsystem Initialized (Trust Anchor Key Active)");
    return 0;
}

int aegis_detect_load_manifest(const aegis_manifest_t *m) {
    if (!m) return -1;

    int auth_res = aegis_detect_authenticate_manifest(m, active_auth_key, active_key_len);
    if (auth_res != 0) {
        kerror("AegisDetect: Manifest Authentication FAILED (code %d)! Rejecting untrusted policy.", auth_res);
        manifest_loaded = 0;
        return auth_res;
    }

    memcpy(&current_manifest, m, sizeof(aegis_manifest_t));
    manifest_loaded = 1;
    verified_components_count = m->entry_count;

    for (uint32_t i = 0; i < m->entry_count; i++) {
        strncpy(verify_results[i].name, m->entries[i].name, AEGIS_MAX_NAME_LEN - 1);
        verify_results[i].name[AEGIS_MAX_NAME_LEN - 1] = '\0';
        verify_results[i].flags = m->entries[i].flags;
        verify_results[i].state = AEGIS_STATE_UNVERIFIABLE;
        verify_results[i].size = 0;
        memcpy(verify_results[i].expected_sha256, m->entries[i].sha256, SHA256_DIGEST_SIZE);
        memset(verify_results[i].computed_sha256, 0, SHA256_DIGEST_SIZE);
    }

    kinfo("AegisDetect: Loaded and authenticated trusted baseline (%u protected components)",
          current_manifest.entry_count);
    return 0;
}

int aegis_detect_verify_component(const char *name, const void *data, size_t size) {
    if (!manifest_loaded || !name) return -1;

    int idx = -1;
    for (uint32_t i = 0; i < current_manifest.entry_count; i++) {
        if (strcmp(current_manifest.entries[i].name, name) == 0) {
            idx = (int)i;
            break;
        }
    }

    if (idx < 0) {
        kwarn("AegisDetect: Component '%s' is UNAUTHORIZED (not in trusted manifest)", name);
        return -2;
    }

    aegis_verify_result_t *res = &verify_results[idx];
    res->size = (uint32_t)size;

    if (!data) {
        res->state = AEGIS_STATE_MISSING;
        if (res->flags & AEGIS_FLAG_CRITICAL) {
            kerror("AegisDetect: CRITICAL COMPONENT MISSING: '%s'!", name);
        } else {
            kwarn("AegisDetect: Component missing: '%s'", name);
        }
        return -3;
    }

    sha256_hash(data, size, res->computed_sha256);

    if (memcmp(res->computed_sha256, res->expected_sha256, SHA256_DIGEST_SIZE) == 0) {
        res->state = AEGIS_STATE_TRUSTED;
        kinfo("AegisDetect: Component '%s' [TRUSTED] (SHA-256 verified, %u bytes)", name, (uint32_t)size);
        return 0;
    } else {
        res->state = AEGIS_STATE_MODIFIED;
        if (res->flags & AEGIS_FLAG_CRITICAL) {
            kerror("===============================================================");
            kerror("!!! AegisDetect SECURITY ALERT: CRITICAL TAMPER DETECTED !!!");
            kerror("Component '%s' has been MODIFIED or CORRUPTED!", name);
            kerror("Expected SHA-256 does not match actual calculated hash.");
            kerror("Enforcing Fail-Closed policy response.");
            kerror("===============================================================");
        } else {
            kwarn("AegisDetect: Tamper detected in non-critical component '%s' (Quarantine enforced)", name);
        }
        return 1;
    }
}

int aegis_detect_verify_all(void) {
    if (!manifest_loaded) return -1;

    uint32_t trusted = 0;
    uint32_t tampered = 0;

    for (uint32_t i = 0; i < current_manifest.entry_count; i++) {
        if (verify_results[i].state == AEGIS_STATE_TRUSTED) {
            trusted++;
        } else if (verify_results[i].state == AEGIS_STATE_MODIFIED ||
                   verify_results[i].state == AEGIS_STATE_MISSING) {
            tampered++;
        }
    }

    kinfo("AegisDetect: Verification summary: %u/%u components verified (%u trusted, %u tampered/missing)",
          trusted + tampered, current_manifest.entry_count, trusted, tampered);
    return (tampered == 0) ? 0 : 1;
}

uint32_t aegis_detect_get_total_count(void) {
    return manifest_loaded ? current_manifest.entry_count : 0;
}

uint32_t aegis_detect_get_trusted_count(void) {
    uint32_t count = 0;
    for (uint32_t i = 0; i < verified_components_count; i++) {
        if (verify_results[i].state == AEGIS_STATE_TRUSTED) count++;
    }
    return count;
}

uint32_t aegis_detect_get_tampered_count(void) {
    uint32_t count = 0;
    for (uint32_t i = 0; i < verified_components_count; i++) {
        if (verify_results[i].state == AEGIS_STATE_MODIFIED ||
            verify_results[i].state == AEGIS_STATE_MISSING) {
            count++;
        }
    }
    return count;
}

const aegis_verify_result_t *aegis_detect_get_result(uint32_t index) {
    if (index >= verified_components_count) return NULL;
    return &verify_results[index];
}

void aegis_detect_print_status(void) {
    kprintf("\n=== AegisDetect Boot Integrity & System Protection Subsystem ===\n");
    if (!manifest_loaded) {
        kprintf("  Manifest Status: UNLOADED (No authenticated baseline active)\n\n");
        return;
    }

    kprintf("  Baseline Policy: AUTHENTICATED (HMAC-SHA256 Verified)\n");
    kprintf("  Components:      %u total, %u trusted, %u tampered/missing\n\n",
            current_manifest.entry_count,
            aegis_detect_get_trusted_count(),
            aegis_detect_get_tampered_count());

    kprintf("  %-20s %-10s %-12s %-8s %s\n", "COMPONENT", "TYPE", "STATUS", "SIZE", "HASH STATUS");
    kprintf("  --------------------------------------------------------------------------------\n");
    for (uint32_t i = 0; i < current_manifest.entry_count; i++) {
        const char *type_str = "OPTIONAL";
        if (verify_results[i].flags & AEGIS_FLAG_CRITICAL) type_str = "CRITICAL";
        else if (verify_results[i].flags & AEGIS_FLAG_SYSTEM) type_str = "SYSTEM";

        const char *status_str = "UNKNOWN";
        switch (verify_results[i].state) {
            case AEGIS_STATE_TRUSTED:      status_str = "TRUSTED"; break;
            case AEGIS_STATE_MODIFIED:     status_str = "MODIFIED"; break;
            case AEGIS_STATE_MISSING:      status_str = "MISSING"; break;
            case AEGIS_STATE_UNVERIFIABLE: status_str = "UNVERIFIED"; break;
            case AEGIS_STATE_UNAUTHORIZED: status_str = "UNAUTHORIZED"; break;
        }

        const char *hash_match = (verify_results[i].state == AEGIS_STATE_TRUSTED) ? "MATCH [OK]" :
                                 (verify_results[i].state == AEGIS_STATE_MODIFIED) ? "MISMATCH [FAIL]" : "N/A";

        kprintf("  %-20s %-10s %-12s %-8u %s\n",
                verify_results[i].name, type_str, status_str,
                verify_results[i].size, hash_match);
    }
    kprintf("  --------------------------------------------------------------------------------\n\n");
}
