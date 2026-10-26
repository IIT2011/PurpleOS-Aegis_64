#ifndef APEXEC_H
#define APEXEC_H

#include "types.h"
#include "vmm.h"

#define APEX_MAGIC              0x4550417F /* '\x7F' 'A' 'P' 'E' */
#define APEX_VERSION_CURRENT    1
#define APEX_ARCH_X86_64        0x8664
#define APEX_ABI_PURPLE_V1      1

/* Segment Types */
#define APEX_SEG_NULL           0
#define APEX_SEG_CODE           1 /* Executable code (.text) */
#define APEX_SEG_RODATA         2 /* Read-only data & strings (.rodata) */
#define APEX_SEG_DATA           3 /* Read-write initialized data (.data) */
#define APEX_SEG_BSS            4 /* Read-write zero-initialized (.bss) */

/* Segment Permission Flags */
#define APEX_FLAG_READ          (1 << 0)
#define APEX_FLAG_WRITE         (1 << 1)
#define APEX_FLAG_EXEC          (1 << 2)

/* Error Codes */
#define APEX_OK                  0
#define APEX_ERR_NULL_PTR       -1
#define APEX_ERR_TOO_SMALL      -2
#define APEX_ERR_BAD_MAGIC      -3
#define APEX_ERR_BAD_VERSION    -4
#define APEX_ERR_BAD_ARCH       -5
#define APEX_ERR_BAD_ABI        -6
#define APEX_ERR_CORRUPT_HEADER -7
#define APEX_ERR_SEGMENT_BOUNDS -8
#define APEX_ERR_SECURITY_WX    -9 /* W^X violation */
#define APEX_ERR_INVALID_VADDR  -10
#define APEX_ERR_INVALID_ENTRY  -11
#define APEX_ERR_OUT_OF_MEMORY  -12
#define APEX_ERR_CHECKSUM       -13

#define APEX_MAX_SEGMENTS       16
#define APEX_DEFAULT_STACK_SIZE (64 * 1024) /* 64 KiB */
#define APEX_MAX_STACK_SIZE     (2 * 1024 * 1024) /* 2 MiB */

/* 64-byte File Header */
typedef struct {
    uint32_t magic;             /* APEX_MAGIC ('\x7F' 'A' 'P' 'E') */
    uint8_t  version;           /* APEX_VERSION_CURRENT (1) */
    uint8_t  abi_version;       /* APEX_ABI_PURPLE_V1 (1) */
    uint16_t arch;              /* APEX_ARCH_X86_64 (0x8664) */
    uint32_t header_size;       /* sizeof(apex_header_t) = 64 */
    uint64_t entry_point;       /* Virtual address of entry point in userspace */
    uint64_t base_vaddr;        /* Recommended base virtual address */
    uint64_t file_size;         /* Total file size in bytes */
    uint16_t num_segments;      /* Number of segment descriptors */
    uint16_t reserved0;
    uint32_t stack_size;        /* Requested user stack size */
    uint32_t checksum;          /* CRC32 of binary payload */
    uint32_t reserved1[4];      /* Padding to 64 bytes */
} __attribute__((packed)) apex_header_t;

/* 40-byte Segment Descriptor */
typedef struct {
    uint32_t type;              /* APEX_SEG_* */
    uint32_t flags;             /* APEX_FLAG_* */
    uint64_t file_offset;       /* Offset in file */
    uint64_t file_size;         /* Size in file */
    uint64_t vaddr;             /* Virtual address (page-aligned) */
    uint64_t mem_size;          /* Size in memory (>= file_size) */
} __attribute__((packed)) apex_segment_t;

/* Loaded Binary Metadata */
typedef struct {
    uint64_t entry_point;
    uint64_t stack_top;
    uint64_t stack_base;
    uint32_t stack_size;
    uint64_t min_vaddr;
    uint64_t max_vaddr;
    uint16_t num_segments;
} apex_binary_info_t;

int apex_validate(const void *data, size_t size);
int apex_get_header(const void *data, size_t size, apex_header_t *out_hdr);
int apex_get_segment(const void *data, size_t size, uint16_t index, apex_segment_t *out_seg);
int apex_load(page_table_t *pml4, const void *data, size_t size, apex_binary_info_t *out_info);

#endif /* APEXEC_H */
