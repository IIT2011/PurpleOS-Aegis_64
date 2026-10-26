#ifndef APAPP_H
#define APAPP_H

#include "types.h"
#include "task.h"

#define APAPP_MAGIC          "APAPP100"
#define APAPP_MAGIC_LEN      8
#define APAPP_VERSION        1
#define APAPP_EXTENSION      ".apapplication"
#define APAPP_SYSTEM_DIR     "/System/Applications"
#define APAPP_USER_DIR       "/Users/aegis/Applications"

#define APAPP_MAX_NAME       32
#define APAPP_MAX_AUTHOR     32
#define APAPP_MAX_VERSION    16
#define APAPP_MAX_DESC       64
#define APAPP_MAX_APPS       16

/* Error Codes */
#define APAPP_OK             0
#define APAPP_ERR_INVALID   -1
#define APAPP_ERR_MAGIC     -2
#define APAPP_ERR_VERSION   -3
#define APAPP_ERR_SIGNATURE -4
#define APAPP_ERR_CAPACITY  -5
#define APAPP_ERR_MEMORY    -6
#define APAPP_ERR_NOT_FOUND -7
#define APAPP_ERR_EXISTS    -8

/* Permissions / Capabilities */
#define APAPP_PERM_CONSOLE   (1 << 0)
#define APAPP_PERM_GUI       (1 << 1)
#define APAPP_PERM_STORAGE   (1 << 2)
#define APAPP_PERM_NETWORK   (1 << 3)
#define APAPP_PERM_SYSADMIN  (1 << 4)

/* Application Bundle Manifest Header */
typedef struct {
    uint8_t  magic[8];          /* "APAPP100" */
    uint32_t version;           /* 1 */
    uint32_t manifest_size;     /* sizeof(apapp_manifest_t) */
    char     app_id[APAPP_MAX_NAME];         /* e.g. "sysmon.apapplication" */
    char     app_name[APAPP_MAX_NAME];       /* e.g. "Aegis System Monitor" */
    char     version_str[APAPP_MAX_VERSION]; /* "1.0.0" */
    char     author[APAPP_MAX_AUTHOR];       /* "Purple OS Team" */
    char     description[APAPP_MAX_DESC];
    uint32_t permissions;       /* APAPP_PERM_* */
    uint32_t default_priority;  /* Task priority 1..3 */
    uint32_t window_width;      /* Default GUI window dimensions */
    uint32_t window_height;

    /* Section Offsets in bundle file */
    uint32_t exec_offset;
    uint32_t exec_size;
    uint32_t res_offset;
    uint32_t res_size;
    uint32_t lib_offset;
    uint32_t lib_size;
    uint32_t meta_offset;
    uint32_t meta_size;

    /* Security: HMAC-SHA256 authenticated by AegisDetect */
    uint8_t  hmac_signature[32];
    uint32_t payload_checksum;
    uint32_t reserved[2];       /* 8-byte alignment */
} __attribute__((packed)) apapp_manifest_t;

typedef enum {
    APAPP_STATE_STOPPED  = 0,
    APAPP_STATE_STARTING = 1,
    APAPP_STATE_RUNNING  = 2,
    APAPP_STATE_ERROR    = 3
} apapp_state_t;

typedef struct apapp_instance {
    apapp_manifest_t manifest;
    apapp_state_t    state;
    uint32_t         task_id;
    int              window_id;
    uint32_t         launch_tick;
    void            *memory;
    uint32_t         memory_size;
} apapp_instance_t;

typedef void (*app_entry_fn)(void *arg);

int apapp_subsystem_init(void);
int apapp_parse_manifest(const void *data, uint32_t size, apapp_manifest_t *out_manifest);
int apapp_verify_security(const apapp_manifest_t *manifest, const void *payload, uint32_t payload_len);
int apapp_launch(const apapp_manifest_t *manifest, const void *payload, uint32_t payload_len, app_entry_fn entry, apapp_instance_t **out_instance);
int apapp_launch_from_storage(const char *bundle_path, app_entry_fn fallback_entry);
int apapp_terminate(const char *app_id);
uint32_t apapp_get_count(void);
const apapp_instance_t *apapp_get_instance(uint32_t index);
const apapp_instance_t *apapp_find(const char *app_id);
int apapp_has_canonical_extension(const char *path);
void apapp_dump(void);

#endif /* APAPP_H */
