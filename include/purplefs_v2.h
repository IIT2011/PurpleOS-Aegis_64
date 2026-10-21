#ifndef PURPLEFS_V2_H
#define PURPLEFS_V2_H

#include "types.h"

#define PFS2_BLOCK_SIZE             4096
#define PFS2_DEFAULT_BLOCKS         32768   /* 128 MiB volume limit */
#define PFS2_MIN_BLOCKS             32
#define PFS2_INODE_COUNT            256
#define PFS2_INODE_SIZE             128
#define PFS2_INODES_PER_BLOCK       (PFS2_BLOCK_SIZE / PFS2_INODE_SIZE) /* 32 */
#define PFS2_INODE_TABLE_BLOCKS     (PFS2_INODE_COUNT / PFS2_INODES_PER_BLOCK) /* 8 */

#define PFS2_BITMAP_BLOCK           1
#define PFS2_INODE_TABLE_START      2
#define PFS2_DATA_START_BLOCK       10
#define PFS2_JOURNAL_BLOCKS         8
#define PFS2_MAX_AFTER_IMAGES       6

#define PFS2_FEATURE_JOURNAL        0x00000001

#define PFS2_JOURNAL_MAGIC          0x5046534A /* "PFSJ" */
#define PFS2_JOURNAL_STATE_EMPTY    0
#define PFS2_JOURNAL_STATE_COMMITTED 1
#define PFS2_JOURNAL_STATE_APPLIED  2

#define PFS2_MODE_FILE              0x8000
#define PFS2_MODE_DIR               0x4000
#define PFS2_ROOT_INODE             1

/* Error codes */
#define PFS2_OK                      0
#define PFS2_ERR_IO                 -1
#define PFS2_ERR_BAD_MAGIC          -2
#define PFS2_ERR_CORRUPT            -3
#define PFS2_ERR_NO_SPACE           -4
#define PFS2_ERR_NO_INODE           -5
#define PFS2_ERR_NOT_FOUND          -6
#define PFS2_ERR_EXISTS             -7
#define PFS2_ERR_INVALID            -8
#define PFS2_ERR_JOURNAL            -9
#define PFS2_ERR_TOO_BIG            -10
#define PFS2_ERR_ROFS               -11

/* 16-byte magic signature */
extern const uint8_t PFS2_SIGNATURE[16];

/* On-disk Superblock (Block 0) */
typedef struct pfs2_superblock {
    uint8_t  magic[16];                 /* 0x00: "PURPLEFS\0PFS2\0\0\0" */
    uint32_t version;                   /* 0x10: 2 */
    uint32_t block_size;                /* 0x14: 4096 */
    uint32_t total_blocks;              /* 0x18: 32768 */
    uint32_t free_blocks;               /* 0x1C */
    uint32_t total_inodes;              /* 0x20: 256 */
    uint32_t free_inodes;               /* 0x24 */
    uint32_t bitmap_start_block;        /* 0x28: 1 */
    uint32_t bitmap_block_count;        /* 0x2C: 1 */
    uint32_t inode_start_block;         /* 0x30: 2 */
    uint32_t inode_block_count;         /* 0x34: 8 */
    uint32_t data_start_block;          /* 0x38: 10 */
    uint32_t journal_start_block;       /* 0x3C: total_blocks - 8 */
    uint32_t journal_block_count;       /* 0x40: 8 */
    uint32_t feature_flags;             /* 0x44: 0x1 = Journal */
    uint64_t mount_time;                /* 0x48 */
    uint64_t write_time;                /* 0x50 */
    uint32_t mount_count;               /* 0x58 */
    uint32_t clean_unmount;             /* 0x5C: 1 if clean, 0 if dirty */
    uint8_t  volume_uuid[16];           /* 0x60 */
    char     volume_label[16];          /* 0x70: null-terminated */
    uint32_t checksum;                  /* 0x80: CRC32 of bytes 0x00..0x7F */
    uint8_t  reserved[3964];            /* Padded to 4096 bytes */
} __attribute__((packed)) pfs2_superblock_t;

/* On-disk Inode (128 bytes) */
typedef struct pfs2_inode {
    uint16_t mode;                      /* 0x00: File type & permissions */
    uint16_t uid;                       /* 0x02 */
    uint16_t gid;                       /* 0x04 */
    uint16_t links_count;               /* 0x06 */
    uint64_t size_bytes;                /* 0x08: File length in bytes */
    uint64_t atime;                     /* 0x10 */
    uint64_t mtime;                     /* 0x18 */
    uint64_t ctime;                     /* 0x20 */
    uint32_t flags;                     /* 0x28 */
    uint32_t direct_blocks[12];         /* 0x2C: 12 * 4 KiB = 48 KiB */
    uint32_t indirect_block;            /* 0x5C: 1 * 1024 * 4 KiB = 4 MiB */
    uint32_t double_indirect;           /* 0x60 */
    uint32_t reserved[6];               /* 0x64 */
    uint32_t checksum;                  /* 0x7C: CRC32 of bytes 0x00..0x7B */
} __attribute__((packed)) pfs2_inode_t;

/* On-disk Directory Entry (32 bytes) */
typedef struct pfs2_dir_entry {
    uint32_t inode;                     /* Inode number (0 = free/unused) */
    uint16_t rec_len;                   /* 32 bytes */
    uint8_t  name_len;                  /* Filename length */
    uint8_t  file_type;                 /* 1 = File, 2 = Directory */
    char     name[24];                  /* Filename (null-terminated if <24) */
} __attribute__((packed)) pfs2_dir_entry_t;

/* On-disk Journal Transaction Header (Stored at journal_start_block) */
typedef struct pfs2_journal_header {
    uint32_t magic;                     /* "PFSJ" (0x5046534A) */
    uint32_t journal_state;             /* EMPTY(0), COMMITTED(1), APPLIED(2) */
    uint64_t sequence_number;           /* Monotonically increasing */
    uint32_t num_after_images;          /* Count of after-image blocks (1..6) */
    uint32_t target_blocks[PFS2_MAX_AFTER_IMAGES]; /* In-place target block numbers */
    uint32_t after_image_crcs[PFS2_MAX_AFTER_IMAGES]; /* CRC32 for each after-image */
    uint32_t header_crc;                /* CRC32 of header up to this field */
    uint8_t  reserved[4024];            /* Padded to 4096 bytes */
} __attribute__((packed)) pfs2_journal_header_t;

/* Block Device Abstraction */
typedef struct pfs2_block_dev {
    uint32_t block_count;
    int (*read_block)(struct pfs2_block_dev *dev, uint32_t block_num, void *buf);
    int (*write_block)(struct pfs2_block_dev *dev, uint32_t block_num, const void *buf);
    int (*flush)(struct pfs2_block_dev *dev);
    void *priv;
} pfs2_block_dev_t;

/* In-memory Filesystem Instance */
typedef struct pfs2_fs {
    pfs2_block_dev_t *dev;
    pfs2_superblock_t sb;
    uint8_t           bitmap[PFS2_BLOCK_SIZE];
    uint64_t          next_journal_seq;
    int               is_mounted;
    int               read_only;
} pfs2_fs_t;

/* Public API */
uint32_t pfs2_crc32(const void *data, size_t length);

int pfs2_format(pfs2_block_dev_t *dev, uint32_t total_blocks, const char *label);
int pfs2_mount(pfs2_fs_t *fs, pfs2_block_dev_t *dev);
int pfs2_unmount(pfs2_fs_t *fs);

int pfs2_create_file(pfs2_fs_t *fs, const char *name, uint16_t mode, uint32_t *out_inode);
int pfs2_lookup(pfs2_fs_t *fs, const char *name, uint32_t *out_inode);
int pfs2_read_file(pfs2_fs_t *fs, uint32_t inode_num, void *buf, size_t count, uint64_t offset, size_t *out_bytes_read);
int pfs2_write_file(pfs2_fs_t *fs, uint32_t inode_num, const void *buf, size_t count, uint64_t offset, size_t *out_bytes_written);
int pfs2_truncate_file(pfs2_fs_t *fs, uint32_t inode_num, uint64_t new_size);
int pfs2_read_inode(pfs2_fs_t *fs, uint32_t inode_num, pfs2_inode_t *out_inode);
int pfs2_sync(pfs2_fs_t *fs);

/* Crash Recovery & Journal Operations */
int pfs2_journal_replay_if_needed(pfs2_fs_t *fs);

#endif /* PURPLEFS_V2_H */
