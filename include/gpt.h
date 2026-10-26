#ifndef GPT_H
#define GPT_H

#include "types.h"
#include "block_dev.h"
#include "ata_pio.h"

#define GPT_SIGNATURE              0x5452415020494645ULL /* "EFI PART" */
#define GPT_REVISION_1_0           0x00010000

#define GPT_OK                      0
#define GPT_ERR_IO                 -1
#define GPT_ERR_BAD_MBR            -2
#define GPT_ERR_BAD_SIGNATURE      -3
#define GPT_ERR_BAD_CRC            -4
#define GPT_ERR_BAD_BOUNDS         -5
#define GPT_ERR_OVERLAP            -6
#define GPT_ERR_UNSUPPORTED        -7

#define GPT_MAX_PARTITIONS         16

/* EFI GUID (Mixed-endian RFC 4122 format) */
typedef struct gpt_guid {
    uint32_t data1;             /* Little-endian */
    uint16_t data2;             /* Little-endian */
    uint16_t data3;             /* Little-endian */
    uint8_t  data4[8];          /* Big-endian / Byte array */
} __attribute__((packed)) gpt_guid_t;

/* PurpleFS v2 Partition Type GUID: 4F535046-5332-0001-8000-00A0C911D504 */
extern const gpt_guid_t GPT_GUID_PURPLEFS_V2;

/* PurpleFS v3 Partition Type GUID: 4F535046-5333-0001-8000-00A0C911D504 */
extern const gpt_guid_t GPT_GUID_PURPLEFS_V3;

/* EFI System Partition (ESP) GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B */
extern const gpt_guid_t GPT_GUID_ESP;

/* Unused Partition GUID (All zeroes) */
extern const gpt_guid_t GPT_GUID_UNUSED;

/* Protective MBR Partition Record (16 bytes) */
typedef struct mbr_partition_record {
    uint8_t  boot_indicator;    /* 0x00 = non-bootable, 0x80 = bootable */
    uint8_t  start_head;
    uint8_t  start_sector;
    uint8_t  start_cylinder;
    uint8_t  os_type;           /* 0xEE for GPT Protective MBR */
    uint8_t  end_head;
    uint8_t  end_sector;
    uint8_t  end_cylinder;
    uint32_t starting_lba;      /* Typically 1 */
    uint32_t size_in_lba;
} __attribute__((packed)) mbr_partition_record_t;

/* Protective MBR (LBA 0, 512 bytes) */
typedef struct mbr_sector {
    uint8_t  boot_code[446];
    mbr_partition_record_t partitions[4];
    uint16_t signature;         /* 0xAA55 */
} __attribute__((packed)) mbr_sector_t;

/* Primary GPT Header (LBA 1, 92+ bytes) */
typedef struct gpt_header {
    uint64_t signature;                 /* "EFI PART" */
    uint32_t revision;                  /* 0x00010000 */
    uint32_t header_size;               /* Typically 92 bytes */
    uint32_t header_crc32;              /* CRC32 of header with this field zeroed */
    uint32_t reserved;                  /* Zero */
    uint64_t my_lba;                    /* 1 for primary */
    uint64_t alternate_lba;             /* Backup LBA */
    uint64_t first_usable_lba;          /* Typically >= 34 */
    uint64_t last_usable_lba;           /* End of disk minus backup tables */
    gpt_guid_t disk_guid;
    uint64_t partition_entry_lba;       /* Typically 2 */
    uint32_t num_partition_entries;     /* Typically 128 */
    uint32_t size_of_partition_entry;   /* Typically 128 */
    uint32_t partition_entry_array_crc32;
    uint8_t  reserved_padding[420];     /* Padded to 512-byte sector */
} __attribute__((packed)) gpt_header_t;

/* GPT Partition Entry (128 bytes) */
typedef struct gpt_entry {
    gpt_guid_t partition_type_guid;
    gpt_guid_t unique_partition_guid;
    uint64_t   starting_lba;
    uint64_t   ending_lba;              /* Inclusive */
    uint64_t   attributes;
    uint16_t   partition_name[36];      /* UTF-16LE */
} __attribute__((packed)) gpt_entry_t;

/* Parsed Partition Info */
typedef struct gpt_partition {
    uint32_t   index;
    gpt_guid_t type_guid;
    gpt_guid_t unique_guid;
    uint64_t   start_lba;
    uint64_t   end_lba;
    uint64_t   sector_count;
    uint64_t   attributes;
    char       name[37];                /* ASCII converted */
    int        is_purplefs;
} gpt_partition_t;

/* Parsed GPT Table State */
typedef struct gpt_info {
    int             valid_protective_mbr;
    int             valid_header;
    int             valid_entries;
    uint64_t        current_lba;
    uint64_t        backup_lba;
    uint64_t        first_usable_lba;
    uint64_t        last_usable_lba;
    uint32_t        partition_count;
    gpt_partition_t partitions[GPT_MAX_PARTITIONS];
    int             purplefs_part_index; /* -1 if none found */
} gpt_info_t;

/* Public API */
int gpt_guid_equal(const gpt_guid_t *a, const gpt_guid_t *b);
void gpt_guid_to_str(const gpt_guid_t *guid, char *out_str);
int gpt_parse_block_dev(block_device_t *bdev, gpt_info_t *out_info);
int gpt_parse(ata_device_t *dev, gpt_info_t *out_info);

#endif /* GPT_H */
