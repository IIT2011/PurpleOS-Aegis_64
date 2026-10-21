#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "crypto/sha256.h"
#include "security/aegis_detect.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %2d] Running %-45s ... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        printf("PASSED\n"); \
        tests_passed++; \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while (0)

static void bytes_to_hex(const uint8_t *bytes, size_t len, char *hex_out) {
    static const char hex[] = "0123456789abcdef";
    for (size_t i = 0; i < len; i++) {
        hex_out[i * 2 + 0] = hex[(bytes[i] >> 4) & 0x0F];
        hex_out[i * 2 + 1] = hex[bytes[i] & 0x0F];
    }
    hex_out[len * 2] = '\0';
}

static int test_sha256_nist_empty(void) {
    uint8_t hash[32];
    sha256_hash("", 0, hash);
    char hex[65];
    bytes_to_hex(hash, 32, hex);
    return strcmp(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855") == 0;
}

static int test_sha256_nist_abc(void) {
    uint8_t hash[32];
    sha256_hash("abc", 3, hash);
    char hex[65];
    bytes_to_hex(hash, 32, hex);
    return strcmp(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad") == 0;
}

static int test_sha256_nist_multiblock(void) {
    const char *msg = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    uint8_t hash[32];
    sha256_hash(msg, strlen(msg), hash);
    char hex[65];
    bytes_to_hex(hash, 32, hex);
    return strcmp(hex, "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1") == 0;
}

static int test_hmac_sha256_rfc2104(void) {
    const char *key = "PurpleOS-Key-2026";
    const char *msg = "AegisDetect-Manifest-Payload";
    uint8_t mac[32];
    hmac_sha256(key, strlen(key), msg, strlen(msg), mac);
    char hex[65];
    bytes_to_hex(mac, 32, hex);
    return strcmp(hex, "09e43a296e526f99a928eaf9d0bffa8ef4a3e786e42282bf822506d957ae323f") == 0;
}

static int test_manifest_authentication_and_load(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 2;

    strcpy(m.entries[0].name, "kernel64.elf");
    m.entries[0].flags = AEGIS_FLAG_CRITICAL;
    m.entries[0].expected_size = 1000;
    sha256_hash("KERNEL_MOCK_BYTES", 17, m.entries[0].sha256);

    strcpy(m.entries[1].name, "welcome.txt");
    m.entries[1].flags = AEGIS_FLAG_SYSTEM;
    m.entries[1].expected_size = 20;
    sha256_hash("WELCOME_MOCK_BYTES", 18, m.entries[1].sha256);

    /* Sign manifest */
    aegis_manifest_sign(&m, key, strlen((const char *)key));

    /* Loading valid manifest must succeed */
    if (aegis_detect_load_manifest(&m) != 0) return 0;
    if (aegis_detect_get_total_count() != 2) return 0;

    return 1;
}

static int test_tampered_manifest_rejection(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 1;
    strcpy(m.entries[0].name, "boot.cfg");
    m.entries[0].flags = AEGIS_FLAG_CRITICAL;
    sha256_hash("CONFIG", 6, m.entries[0].sha256);

    /* Sign manifest */
    aegis_manifest_sign(&m, key, strlen((const char *)key));

    /* Tamper with entry */
    m.entries[0].flags = AEGIS_FLAG_OPTIONAL;

    /* Loading tampered manifest MUST fail */
    if (aegis_detect_load_manifest(&m) == 0) return 0;

    return 1;
}

static int test_component_verification_trusted(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 1;
    strcpy(m.entries[0].name, "kernel.sys");
    m.entries[0].flags = AEGIS_FLAG_CRITICAL;
    m.entries[0].expected_size = 12;
    sha256_hash("Aegis64Code!", 12, m.entries[0].sha256);

    aegis_manifest_sign(&m, key, strlen((const char *)key));
    if (aegis_detect_load_manifest(&m) != 0) return 0;

    /* Verify with genuine content */
    int res = aegis_detect_verify_component("kernel.sys", "Aegis64Code!", 12);
    if (res != 0) return 0;

    const aegis_verify_result_t *vr = aegis_detect_get_result(0);
    if (!vr || vr->state != AEGIS_STATE_TRUSTED) return 0;
    if (aegis_detect_get_trusted_count() != 1) return 0;
    if (aegis_detect_get_tampered_count() != 0) return 0;

    return 1;
}

static int test_component_verification_tampered(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 1;
    strcpy(m.entries[0].name, "kernel.sys");
    m.entries[0].flags = AEGIS_FLAG_CRITICAL;
    m.entries[0].expected_size = 12;
    sha256_hash("Aegis64Code!", 12, m.entries[0].sha256);

    aegis_manifest_sign(&m, key, strlen((const char *)key));
    if (aegis_detect_load_manifest(&m) != 0) return 0;

    /* Verify with modified/corrupted content */
    int res = aegis_detect_verify_component("kernel.sys", "MaliciousMod!", 13);
    if (res != 1) return 0; // Returns 1 on tamper

    const aegis_verify_result_t *vr = aegis_detect_get_result(0);
    if (!vr || vr->state != AEGIS_STATE_MODIFIED) return 0;
    if (aegis_detect_get_trusted_count() != 0) return 0;
    if (aegis_detect_get_tampered_count() != 1) return 0;

    return 1;
}

static int test_component_verification_missing(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 1;
    strcpy(m.entries[0].name, "missing.sys");
    m.entries[0].flags = AEGIS_FLAG_CRITICAL;
    sha256_hash("DATA", 4, m.entries[0].sha256);

    aegis_manifest_sign(&m, key, strlen((const char *)key));
    if (aegis_detect_load_manifest(&m) != 0) return 0;

    /* NULL data signifies missing component */
    int res = aegis_detect_verify_component("missing.sys", NULL, 0);
    if (res != -3) return 0;

    const aegis_verify_result_t *vr = aegis_detect_get_result(0);
    if (!vr || vr->state != AEGIS_STATE_MISSING) return 0;
    if (aegis_detect_get_tampered_count() != 1) return 0;

    return 1;
}

static int test_component_unauthorized_rejection(void) {
    const uint8_t key[] = "SecretTestKey1234";
    aegis_detect_init(key, strlen((const char *)key));

    aegis_manifest_t m;
    memset(&m, 0, sizeof(m));
    m.magic = AEGIS_MANIFEST_MAGIC;
    m.version = AEGIS_MANIFEST_VERSION;
    m.entry_count = 1;
    strcpy(m.entries[0].name, "known.sys");
    sha256_hash("DATA", 4, m.entries[0].sha256);

    aegis_manifest_sign(&m, key, strlen((const char *)key));
    if (aegis_detect_load_manifest(&m) != 0) return 0;

    /* Unknown component must be rejected as unauthorized (-2) */
    int res = aegis_detect_verify_component("rootkit.sys", "EVIL", 4);
    if (res != -2) return 0;

    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running AegisDetect Integrity & Security Test Suite \n");
    printf("=====================================================\n");

    RUN_TEST(test_sha256_nist_empty);
    RUN_TEST(test_sha256_nist_abc);
    RUN_TEST(test_sha256_nist_multiblock);
    RUN_TEST(test_hmac_sha256_rfc2104);
    RUN_TEST(test_manifest_authentication_and_load);
    RUN_TEST(test_tampered_manifest_rejection);
    RUN_TEST(test_component_verification_trusted);
    RUN_TEST(test_component_verification_tampered);
    RUN_TEST(test_component_verification_missing);
    RUN_TEST(test_component_unauthorized_rejection);

    printf("=====================================================\n");
    printf(" AegisDetect Tests: %d/%d passed                     \n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
