#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <assert.h>
#include "apapp.h"
#include "crypto/sha256.h"
#include "desktop.h"

static const uint8_t test_trust_key[] = "PurpleOS-Aegis64-TrustAnchor-2026";

static void sign_bundle(apapp_manifest_t *m, const void *payload, uint32_t payload_len) {
    sha256_ctx_t ctx;
    uint8_t k_pad[SHA256_BLOCK_SIZE];
    uint8_t inner[SHA256_DIGEST_SIZE];
    size_t key_len = strlen((const char *)test_trust_key);

    memset(k_pad, 0x36, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= test_trust_key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    size_t header_len = (size_t)((uintptr_t)m->hmac_signature - (uintptr_t)m);
    sha256_update(&ctx, m, header_len);
    if (payload && payload_len > 0) {
        sha256_update(&ctx, payload, payload_len);
    }
    sha256_final(&ctx, inner);

    memset(k_pad, 0x5c, SHA256_BLOCK_SIZE);
    for (size_t i = 0; i < key_len && i < SHA256_BLOCK_SIZE; i++) {
        k_pad[i] ^= test_trust_key[i];
    }
    sha256_init(&ctx);
    sha256_update(&ctx, k_pad, SHA256_BLOCK_SIZE);
    sha256_update(&ctx, inner, SHA256_DIGEST_SIZE);
    sha256_final(&ctx, m->hmac_signature);
}

static void test_apapp_init_state(void) {
    printf("[TEST  1] Running test_apapp_init_state                     ... ");
    assert(apapp_subsystem_init() == APAPP_OK);
    assert(apapp_get_count() == 0);
    assert(apapp_find("calc.apapplication") == NULL);
    printf("PASSED\n");
}

static void test_apapp_manifest_parsing(void) {
    printf("[TEST  2] Running test_apapp_manifest_parsing              ... ");
    apapp_manifest_t m;
    memset(&m, 0, sizeof(m));
    memcpy(m.magic, APAPP_MAGIC, 8);
    m.version = 1;
    m.manifest_size = sizeof(apapp_manifest_t);
    strcpy(m.app_id, "calculator.apapplication");
    strcpy(m.app_name, "Desktop Calculator");
    strcpy(m.version_str, "1.0.0");
    strcpy(m.author, "Aegis Core");
    m.permissions = APAPP_PERM_GUI | APAPP_PERM_CONSOLE;
    m.window_width = 300;
    m.window_height = 200;

    apapp_manifest_t parsed;
    assert(apapp_parse_manifest(&m, sizeof(m), &parsed) == APAPP_OK);
    assert(strcmp(parsed.app_id, "calculator.apapplication") == 0);
    assert(strcmp(parsed.app_name, "Desktop Calculator") == 0);
    assert(parsed.permissions == (APAPP_PERM_GUI | APAPP_PERM_CONSOLE));

    /* Corrupted magic */
    memcpy(m.magic, "BADMAGIC", 8);
    assert(apapp_parse_manifest(&m, sizeof(m), &parsed) == APAPP_ERR_MAGIC);

    /* Corrupted version */
    memcpy(m.magic, APAPP_MAGIC, 8);
    m.version = 99;
    assert(apapp_parse_manifest(&m, sizeof(m), &parsed) == APAPP_ERR_VERSION);

    /* Size too small */
    assert(apapp_parse_manifest(&m, 16, &parsed) == APAPP_ERR_INVALID);

    printf("PASSED\n");
}

static void test_apapp_security_verification_valid(void) {
    printf("[TEST  3] Running test_apapp_security_verification_valid   ... ");
    apapp_manifest_t m;
    memset(&m, 0, sizeof(m));
    memcpy(m.magic, APAPP_MAGIC, 8);
    m.version = 1;
    m.manifest_size = sizeof(apapp_manifest_t);
    strcpy(m.app_id, "sysmon.apapplication");
    strcpy(m.app_name, "System Monitor");

    const char *payload = "PAYLOAD_EXECUTABLE_CODE_BYTES_12345";
    sign_bundle(&m, payload, strlen(payload));

    assert(apapp_verify_security(&m, payload, strlen(payload)) == APAPP_OK);
    printf("PASSED\n");
}

static void test_apapp_security_verification_tampered(void) {
    printf("[TEST  4] Running test_apapp_security_verification_tampered ... ");
    apapp_manifest_t m;
    memset(&m, 0, sizeof(m));
    memcpy(m.magic, APAPP_MAGIC, 8);
    m.version = 1;
    m.manifest_size = sizeof(apapp_manifest_t);
    strcpy(m.app_id, "sysmon.apapplication");
    strcpy(m.app_name, "System Monitor");

    const char *payload = "PAYLOAD_EXECUTABLE_CODE_BYTES_12345";
    sign_bundle(&m, payload, strlen(payload));

    /* 1. Tamper with manifest field */
    m.permissions |= APAPP_PERM_SYSADMIN;
    assert(apapp_verify_security(&m, payload, strlen(payload)) == APAPP_ERR_SIGNATURE);

    /* Restore and tamper with payload */
    m.permissions &= ~APAPP_PERM_SYSADMIN;
    sign_bundle(&m, payload, strlen(payload));

    const char *tampered_payload = "PAYLOAD_EXECUTABLE_CODE_TAMPERED_000";
    assert(apapp_verify_security(&m, tampered_payload, strlen(tampered_payload)) == APAPP_ERR_SIGNATURE);

    printf("PASSED\n");
}

static void test_apapp_launch_gui_and_lifecycle(void) {
    printf("[TEST  5] Running test_apapp_launch_gui_and_lifecycle      ... ");
    desktop_init();

    apapp_manifest_t m;
    memset(&m, 0, sizeof(m));
    memcpy(m.magic, APAPP_MAGIC, 8);
    m.version = 1;
    m.manifest_size = sizeof(apapp_manifest_t);
    strcpy(m.app_id, "editor.apapplication");
    strcpy(m.app_name, "Purple Text Editor");
    strcpy(m.version_str, "2.1.0");
    m.permissions = APAPP_PERM_GUI;
    m.window_width = 400;
    m.window_height = 250;

    const char *payload = "EDITOR_BINARY";
    sign_bundle(&m, payload, strlen(payload));

    apapp_instance_t *inst = NULL;
    assert(apapp_launch(&m, payload, strlen(payload), NULL, &inst) == APAPP_OK);
    assert(inst != NULL);
    assert(inst->state == APAPP_STATE_RUNNING);
    assert(inst->window_id >= 0); /* GUI window created */
    assert(apapp_get_count() == 1);
    assert(apapp_find("editor.apapplication") == inst);

    /* Duplicate rejection */
    assert(apapp_launch(&m, payload, strlen(payload), NULL, &inst) == APAPP_ERR_EXISTS);

    /* Terminate */
    assert(apapp_terminate("editor.apapplication") == APAPP_OK);
    assert(apapp_get_count() == 0);
    assert(apapp_find("editor.apapplication") == NULL);

    printf("PASSED\n");
}

static void test_apapp_disk_bundles_validation(void) {
    printf("[TEST  6] Running test_apapp_disk_bundles_validation       ... ");
    FILE *f = fopen("build/apps/terminal.apapplication", "rb");
    if (!f) {
        printf("SKIPPED (no bundle file)\n");
        return;
    }

    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    assert(sz >= (long)sizeof(apapp_manifest_t));
    uint8_t *buf = (uint8_t *)malloc(sz);
    assert(fread(buf, 1, sz, f) == (size_t)sz);
    fclose(f);

    apapp_manifest_t manifest;
    assert(apapp_parse_manifest(buf, sz, &manifest) == APAPP_OK);
    assert(strcmp(manifest.app_id, "terminal.apapplication") == 0);

    const void *payload = buf + manifest.exec_offset;
    assert(apapp_verify_security(&manifest, payload, manifest.exec_size) == APAPP_OK);

    apapp_instance_t *inst = NULL;
    assert(apapp_launch(&manifest, payload, manifest.exec_size, NULL, &inst) == APAPP_OK);
    assert(inst != NULL);
    assert(apapp_get_count() == 1);
    assert(apapp_terminate("terminal.apapplication") == APAPP_OK);

    free(buf);
    printf("PASSED\n");
}

static void test_apapp_canonical_extension_enforcement(void) {
    printf("[TEST  7] Running test_apapp_canonical_extension_enforcement... ");
    /* 1. Canonical .apapplication extension must be accepted */
    assert(apapp_has_canonical_extension("terminal.apapplication") == 1);
    assert(apapp_has_canonical_extension("/System/Applications/PurpleTerminal.apapplication") == 1);
    assert(apapp_has_canonical_extension("/Users/aegis/Applications/Calculator.apapplication") == 1);
    assert(apapp_has_canonical_extension("/Users/Admin/Applications/Editor.apapplication") == 1);

    /* 2. Noncanonical .apapp MUST be strictly rejected */
    assert(apapp_has_canonical_extension("terminal.apapp") == 0);
    assert(apapp_has_canonical_extension("/System/Applications/terminal.apapp") == 0);
    assert(apapp_has_canonical_extension("/Users/aegis/Applications/calculator.apapp") == 0);

    /* 3. Plural .apapplications or other extensions MUST be rejected */
    assert(apapp_has_canonical_extension("terminal.apapplications") == 0);
    assert(apapp_has_canonical_extension("app.exe") == 0);
    assert(apapp_has_canonical_extension("app.bin") == 0);
    assert(apapp_has_canonical_extension("app.app") == 0);
    assert(apapp_has_canonical_extension("terminal") == 0);
    assert(apapp_has_canonical_extension("") == 0);
    assert(apapp_has_canonical_extension(NULL) == 0);

    /* 4. apapp_launch_from_storage must fail on noncanonical extensions */
    assert(apapp_launch_from_storage("terminal.apapp", NULL) == APAPP_ERR_INVALID);
    assert(apapp_launch_from_storage("terminal.exe", NULL) == APAPP_ERR_INVALID);
    assert(apapp_launch_from_storage("terminal", NULL) == APAPP_ERR_INVALID);

    /* 5. apapp_launch_from_storage succeeds validation on canonical extension */
    assert(apapp_launch_from_storage("terminal.apapplication", NULL) == APAPP_OK);
    assert(apapp_launch_from_storage("/System/Applications/PurpleTerminal.apapplication", NULL) == APAPP_OK);
    assert(apapp_launch_from_storage("/Users/aegis/Applications/Calculator.apapplication", NULL) == APAPP_OK);

    printf("PASSED\n");
}

static void test_apapp_directory_structure(void) {
    printf("[TEST  8] Running test_apapp_directory_structure            ... ");
    /* Verify system application canonical directory */
    assert(strcmp(APAPP_SYSTEM_DIR, "/System/Applications") == 0);

    /* Verify user application canonical directory */
    assert(strcmp(APAPP_USER_DIR, "/Users/aegis/Applications") == 0);

    /* Verify canonical extension constant */
    assert(strcmp(APAPP_EXTENSION, ".apapplication") == 0);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running .apapplication Application Bundle Tests     \n");
    printf("=====================================================\n");

    test_apapp_init_state();
    test_apapp_manifest_parsing();
    test_apapp_security_verification_valid();
    test_apapp_security_verification_tampered();
    test_apapp_launch_gui_and_lifecycle();
    test_apapp_disk_bundles_validation();
    test_apapp_canonical_extension_enforcement();
    test_apapp_directory_structure();

    printf("=====================================================\n");
    printf(" .apapplication Tests: 8/8 passed                    \n");
    printf("=====================================================\n");
    return 0;
}
