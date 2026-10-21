#ifndef AEGIS_DETECT_H
#define AEGIS_DETECT_H

#include "types.h"
#include "crypto/sha256.h"

#define AEGIS_MANIFEST_MAGIC    0x41454749  /* "AEGI" */
#define AEGIS_MANIFEST_VERSION  1

#define AEGIS_MAX_COMPONENTS    32
#define AEGIS_MAX_NAME_LEN      64

/* Component Flags */
#define AEGIS_FLAG_CRITICAL     (1 << 0)    /* Fail-closed on tamper (Kernel, Policy) */
#define AEGIS_FLAG_SYSTEM       (1 << 1)    /* Quarantine on tamper (Drivers, Libraries) */
#define AEGIS_FLAG_OPTIONAL     (1 << 2)    /* Warn only on tamper (Docs, utilities) */
#define AEGIS_FLAG_IN_MEMORY    (1 << 3)    /* Memory-resident verification */

/* Component Verification States */
typedef enum {
    AEGIS_STATE_TRUSTED       = 0,  /* Component perfectly matches baseline */
    AEGIS_STATE_MODIFIED      = 1,  /* Content differs from baseline hash */
    AEGIS_STATE_MISSING       = 2,  /* Required component not found */
    AEGIS_STATE_UNVERIFIABLE  = 3,  /* Error reading component or bad manifest */
    AEGIS_STATE_UNAUTHORIZED  = 4   /* Failed authentication or untrusted policy */
} aegis_state_t;

typedef struct {
    char     name[AEGIS_MAX_NAME_LEN];
    uint32_t flags;
    uint32_t expected_size;
    uint8_t  sha256[SHA256_DIGEST_SIZE];
} __attribute__((packed)) aegis_manifest_entry_t;

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint32_t entry_count;
    uint32_t reserved;
    uint8_t  auth_tag[SHA256_DIGEST_SIZE];  /* HMAC-SHA256 authentication */
    aegis_manifest_entry_t entries[AEGIS_MAX_COMPONENTS];
} __attribute__((packed)) aegis_manifest_t;

typedef struct {
    char          name[AEGIS_MAX_NAME_LEN];
    aegis_state_t state;
    uint32_t      flags;
    uint32_t      size;
    uint8_t       computed_sha256[SHA256_DIGEST_SIZE];
    uint8_t       expected_sha256[SHA256_DIGEST_SIZE];
} aegis_verify_result_t;

int aegis_detect_init(const uint8_t *auth_key, size_t key_len);
int aegis_detect_authenticate_manifest(const aegis_manifest_t *m, const uint8_t *key, size_t key_len);
int aegis_detect_load_manifest(const aegis_manifest_t *m);
int aegis_detect_verify_component(const char *name, const void *data, size_t size);
int aegis_detect_verify_all(void);

uint32_t aegis_detect_get_total_count(void);
uint32_t aegis_detect_get_trusted_count(void);
uint32_t aegis_detect_get_tampered_count(void);
const aegis_verify_result_t *aegis_detect_get_result(uint32_t index);
void aegis_detect_print_status(void);

#endif /* AEGIS_DETECT_H */
