#ifndef PURPLEFS_V3_H
#define PURPLEFS_V3_H

#include "types.h"
#include "block_dev.h"

/* Magic signature: 'PFS3' in little-endian (0x50465333) */
#define PFS3_MAGIC                      0x50465333U
#define PFS3_VERSION                    3U

#define PFS3_BLOCK_SIZE                 4096U
#define PFS3_SECTOR_SIZE                512U
#define PFS3_SECTORS_PER_BLOCK          8U

#define PFS3_INODE_SIZE                 256U
#define PFS3_INODES_PER_BLOCK           (PFS3_BLOCK_SIZE / PFS3_INODE_SIZE) /* 16 */

#define PFS3_DIRENT_SIZE                64U
#define PFS3_DIRENTS_PER_BLOCK          (PFS3_BLOCK_SIZE / PFS3_DIRENT_SIZE) /* 64 */

#define PFS3_MAX_NAME_LEN               48U
#define PFS3_DIRECT_BLOCKS              12U
#define PFS3_PTRS_PER_INDIRECT_BLOCK    (PFS3_BLOCK_SIZE / sizeof(uint64_t)) /* 512 */

/* Special Inodes */
#define PFS3_NULL_INODE                 0ULL
#define PFS3_BAD_INODE                  1ULL
#define PFS3_ROOT_INODE                 2ULL

/* File Types for dirent */
#define PFS3_FT_UNKNOWN                 0U
#define PFS3_FT_REG                     1U
#define PFS3_FT_DIR                     2U
#define PFS3_FT_SYMLINK                 3U
#define PFS3_FT_DEV                     4U

/* Inode Mode Flags */
#define PFS3_MODE_REG                   0x8000U
#define PFS3_MODE_DIR                   0x4000U
#define PFS3_MODE_SYMLINK               0xA000U
#define PFS3_MODE_PERM_MASK             0x0FFFU

/* Superblock State Flags */
#define PFS3_STATE_CLEAN                0x00000001U
#define PFS3_STATE_READONLY             0x00000002U
#define PFS3_STATE_RECOVERY             0x00000004U

/* Journal Constants */
#define PFS3_JOURNAL_MAGIC              0x5046534AU /* "PFSJ" */
#define PFS3_JOURNAL_STATE_EMPTY        0U
#define PFS3_JOURNAL_STATE_COMMITTED    1U
#define PFS3_JOURNAL_STATE_APPLIED      2U

/* Error Codes */
#define PFS3_OK                          0
#define PFS3_ERR_IO                     -1
#define PFS3_ERR_BAD_MAGIC              -2
#define PFS3_ERR_CORRUPT                -3
#define PFS3_ERR_NO_SPACE               -4
#define PFS3_ERR_NO_INODE               -5
#define PFS3_ERR_NOT_FOUND              -6
#define PFS3_ERR_EXISTS                 -7
#define PFS3_ERR_INVALID                -8
#define PFS3_ERR_JOURNAL                -9
#define PFS3_ERR_TOO_BIG                -10
#define PFS3_ERR_ROFS                   -11
#define PFS3_ERR_INVALID_NAME           -12
#define PFS3_ERR_DIR_NOT_EMPTY          -13
#define PFS3_ERR_NOT_DIR                -14
#define PFS3_ERR_IS_DIR                 -15
#define PFS3_ERR_OUT_OF_BOUNDS          -16
#define PFS3_ERR_OVERFLOW               -17
#define PFS3_ERR_INVALID_PARTITION      -18
#define PFS3_ERR_DIR_CORRUPT            -19

/* ========================================================================= */
/* On-Disk Structures (Packed)                                               */
/* ========================================================================= */

/* Superblock: Exactly 4096 bytes (Block 1 and Block 2) */
typedef struct pfs3_superblock {
    uint32_t magic;                     /* 0x00: PFS3_MAGIC (0x50465333) */
    uint32_t version;                   /* 0x04: 3 */
    uint32_t block_size;                /* 0x08: 4096 */
    uint32_t sector_size;               /* 0x0C: 512 */
    uint64_t total_blocks;              /* 0x10: Total 4 KiB filesystem blocks */
    uint64_t free_blocks;               /* 0x18: Free usable data blocks */
    uint64_t total_inodes;              /* 0x20: Total inode capacity */
    uint64_t free_inodes;               /* 0x28: Available inodes */
    uint64_t root_inode;                /* 0x30: Root directory inode (default 2) */
    uint64_t block_bitmap_start;        /* 0x38: Start block of block allocation bitmap */
    uint64_t block_bitmap_blocks;       /* 0x40: Number of blocks for block bitmap */
    uint64_t inode_bitmap_start;        /* 0x48: Start block of inode allocation bitmap */
    uint64_t inode_bitmap_blocks;       /* 0x50: Number of blocks for inode bitmap */
    uint64_t inode_table_start;         /* 0x58: Start block of inode table */
    uint64_t inode_table_blocks;        /* 0x60: Number of blocks for inode table */
    uint64_t journal_start;             /* 0x68: Start block of metadata journal */
    uint64_t journal_blocks;            /* 0x70: Number of blocks in journal */
    uint64_t data_start;                /* 0x78: Start block of user data region */
    uint8_t  volume_uuid[16];           /* 0x80: 128-bit filesystem UUID */
    char     volume_label[32];          /* 0x90: Volume name (null-terminated) */
    uint32_t mount_count;               /* 0xB0: Mount count */
    uint32_t state_flags;               /* 0xB4: Clean, Readonly, Recovery */
    uint64_t last_mount_time;           /* 0xB8: Timestamp of last mount */
    uint64_t last_write_time;           /* 0xC0: Timestamp of last write */
    uint8_t  reserved[3892];            /* 0xC8..0xFFB: Pad to 4092 bytes */
    uint32_t checksum;                  /* 0xFFC..0xFFF: CRC32 of bytes 0x000..0xFFB */
} __attribute__((packed)) pfs3_superblock_t;

/* Inode: Exactly 256 bytes */
typedef struct pfs3_inode {
    uint16_t mode;                      /* 0x00: Mode & file type (DIR, REG, SYMLINK) */
    uint16_t uid;                       /* 0x02: Owner User ID */
    uint16_t gid;                       /* 0x04: Owner Group ID */
    uint16_t links_count;               /* 0x06: Hard link counter */
    uint32_t flags;                     /* 0x08: Immutability, system, quarantine flags */
    uint32_t generation;                /* 0x0C: Inode generation/version counter */
    uint64_t size_bytes;                /* 0x10: 64-bit exact file size in bytes */
    uint64_t atime_sec;                 /* 0x18: Last access timestamp */
    uint64_t mtime_sec;                 /* 0x20: Last modification timestamp */
    uint64_t ctime_sec;                 /* 0x28: Status change timestamp */
    uint64_t direct[12];                /* 0x30..0x8F: 12 direct block pointers (48 KiB) */
    uint64_t indirect;                  /* 0x90..0x97: Single indirect pointer (2 MiB) */
    uint64_t double_indirect;           /* 0x98..0x9F: Double indirect pointer (1 GiB) */
    uint64_t triple_indirect;           /* 0xA0..0xA7: Triple indirect pointer (512 GiB) */
    uint32_t checksum;                  /* 0xA8..0xAB: CRC32 of bytes 0x00..0xA7 */
    uint8_t  reserved[84];              /* 0xAC..0xFF: Expansion pad to 256 bytes */
} __attribute__((packed)) pfs3_inode_t;

/* Directory Entry: Exactly 64 bytes */
typedef struct pfs3_dirent {
    uint64_t inode_num;                 /* 0x00..0x07: Inode number (0 = free slot) */
    uint8_t  file_type;                 /* 0x08: 1=REG, 2=DIR, 3=SYMLINK, 4=DEV */
    uint8_t  name_len;                  /* 0x09: Exact length in bytes (1..48) */
    uint16_t flags;                     /* 0x0A..0x0B: Dirent flags */
    uint16_t checksum;                  /* 0x0C..0x0D: CRC16 of bytes 0x00..0x0B */
    uint16_t reserved;                  /* 0x0E..0x0F: Expansion pad */
    char     name[PFS3_MAX_NAME_LEN];   /* 0x10..0x3F: Filename string (48 bytes) */
} __attribute__((packed)) pfs3_dirent_t;

/* Indirect Pointer Block: Exactly 4096 bytes (512 64-bit block pointers) */
typedef struct pfs3_indirect_block {
    uint64_t pointers[PFS3_PTRS_PER_INDIRECT_BLOCK];
} __attribute__((packed)) pfs3_indirect_block_t;

/* Journal Header: Exactly 4096 bytes */
#define PFS3_JOURNAL_MAX_IMAGES 14
typedef struct pfs3_journal_entry {
    uint64_t target_fs_block;
    uint32_t checksum;
    uint32_t pad;
} __attribute__((packed)) pfs3_journal_entry_t;

typedef struct pfs3_journal_header {
    uint32_t magic;                     /* PFS3_JOURNAL_MAGIC */
    uint32_t state;                     /* EMPTY, COMMITTED, APPLIED */
    uint64_t seq_num;                   /* Monotonically increasing transaction sequence */
    uint32_t entry_count;               /* Number of modified blocks in transaction */
    uint32_t pad;
    pfs3_journal_entry_t entries[PFS3_JOURNAL_MAX_IMAGES];
    uint32_t header_crc;                /* CRC32 of header fields up to header_crc */
    uint8_t  reserved[3844];            /* Pad to 4096 bytes */
} __attribute__((packed)) pfs3_journal_header_t;

/* ========================================================================= */
/* Compile-Time Size and Offset Assertions                                   */
/* ========================================================================= */

#if defined(__STDC_VERSION__) && (__STDC_VERSION__ >= 201112L)
_Static_assert(sizeof(pfs3_superblock_t) == 4096, "pfs3_superblock_t must be exactly 4096 bytes");
_Static_assert(sizeof(pfs3_inode_t) == 256,       "pfs3_inode_t must be exactly 256 bytes");
_Static_assert(sizeof(pfs3_dirent_t) == 64,       "pfs3_dirent_t must be exactly 64 bytes");
_Static_assert(sizeof(pfs3_indirect_block_t) == 4096, "pfs3_indirect_block_t must be exactly 4096 bytes");
_Static_assert(sizeof(pfs3_journal_header_t) == 4096, "pfs3_journal_header_t must be exactly 4096 bytes");
_Static_assert(__builtin_offsetof(pfs3_inode_t, checksum) == 0xA8, "pfs3_inode_t checksum offset mismatch");
_Static_assert(__builtin_offsetof(pfs3_dirent_t, name) == 0x10, "pfs3_dirent_t name offset mismatch");
#endif

/* ========================================================================= */
/* In-Memory Filesystem Mount State                                          */
/* ========================================================================= */

typedef struct pfs3_fs {
    block_device_t      *bdev;
    uint64_t            part_start_lba;     /* Partition starting sector LBA */
    uint64_t            part_sector_count;  /* Total partition sectors */
    uint64_t            total_fs_blocks;    /* Usable 4 KiB filesystem blocks */
    pfs3_superblock_t   sb;
    int                 mounted;
    int                 read_only;
} pfs3_fs_t;

/* ========================================================================= */
/* Public Filesystem API                                                     */
/* ========================================================================= */

/* Format a partition with PurpleFS v3 */
int pfs3_format(block_device_t *bdev,
                uint64_t part_start_lba,
                uint64_t part_sector_count,
                const char *label,
                const uint8_t uuid[16]);

/* Mount & Unmount */
int pfs3_mount(pfs3_fs_t *fs,
               block_device_t *bdev,
               uint64_t part_start_lba,
               uint64_t part_sector_count);

int pfs3_unmount(pfs3_fs_t *fs);

/* Inode Management */
int pfs3_read_inode(pfs3_fs_t *fs, uint64_t inode_num, pfs3_inode_t *out_inode);
int pfs3_write_inode(pfs3_fs_t *fs, uint64_t inode_num, const pfs3_inode_t *inode);
int pfs3_alloc_inode(pfs3_fs_t *fs, uint16_t mode, uint64_t *out_inode_num);
int pfs3_free_inode(pfs3_fs_t *fs, uint64_t inode_num);

/* Block Allocation & Safe I/O */
int pfs3_alloc_block(pfs3_fs_t *fs, uint64_t *out_fs_block);
int pfs3_free_block(pfs3_fs_t *fs, uint64_t fs_block);
int pfs3_read_block(pfs3_fs_t *fs, uint64_t fs_block, void *buf);
int pfs3_write_block(pfs3_fs_t *fs, uint64_t fs_block, const void *buf);

/* File Operations */
int pfs3_read_file(pfs3_fs_t *fs,
                   uint64_t inode_num,
                   void *buf,
                   uint64_t count,
                   uint64_t offset,
                   uint64_t *out_read);

int pfs3_write_file(pfs3_fs_t *fs,
                    uint64_t inode_num,
                    const void *buf,
                    uint64_t count,
                    uint64_t offset);

int pfs3_truncate_file(pfs3_fs_t *fs, uint64_t inode_num, uint64_t new_size);

/* Hierarchical Directory Operations */
int pfs3_lookup_entry(pfs3_fs_t *fs,
                      uint64_t dir_inode,
                      const char *name,
                      uint64_t *out_inode,
                      uint8_t *out_file_type);

int pfs3_create_entry(pfs3_fs_t *fs,
                      uint64_t dir_inode,
                      const char *name,
                      uint8_t file_type,
                      uint64_t *out_inode);

int pfs3_unlink_entry(pfs3_fs_t *fs, uint64_t dir_inode, const char *name);

int pfs3_lookup_path(pfs3_fs_t *fs, const char *path, uint64_t *out_inode);

int pfs3_mkdir(pfs3_fs_t *fs, uint64_t parent_inode, const char *name, uint64_t *out_inode);

int pfs3_rmdir(pfs3_fs_t *fs, uint64_t parent_inode, const char *name);

int pfs3_readdir(pfs3_fs_t *fs,
                 uint64_t dir_inode,
                 uint32_t entry_index,
                 pfs3_dirent_t *out_dirent);

/* Checksum Utilities */
uint32_t pfs3_crc32(const void *data, size_t length);
uint16_t pfs3_crc16(const void *data, size_t length);

#endif /* PURPLEFS_V3_H */
