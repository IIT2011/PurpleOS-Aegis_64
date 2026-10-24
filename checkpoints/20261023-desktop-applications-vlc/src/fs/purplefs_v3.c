#include "purplefs_v3.h"
#include "string.h"
#include "serial.h"

/* ========================================================================= */
/* CRC Utilities                                                             */
/* ========================================================================= */

uint32_t pfs3_crc32(const void *data, size_t length) {
    const uint8_t *p = (const uint8_t *)data;
    uint32_t crc = 0xFFFFFFFFU;

    for (size_t i = 0; i < length; i++) {
        crc ^= p[i];
        for (int j = 0; j < 8; j++) {
            if (crc & 1) {
                crc = (crc >> 1) ^ 0xEDB88320U;
            } else {
                crc >>= 1;
            }
        }
    }
    return crc ^ 0xFFFFFFFFU;
}

uint16_t pfs3_crc16(const void *data, size_t length) {
    const uint8_t *p = (const uint8_t *)data;
    uint16_t crc = 0xFFFFU;

    for (size_t i = 0; i < length; i++) {
        crc ^= (uint16_t)(p[i] << 8);
        for (int j = 0; j < 8; j++) {
            if (crc & 0x8000U) {
                crc = (crc << 1) ^ 0x1021U;
            } else {
                crc <<= 1;
            }
        }
    }
    return crc;
}

/* ========================================================================= */
/* Internal Block I/O Layer with Subtraction Bounds Checks                   */
/* ========================================================================= */

/* Raw read of any block within partition (including blocks 0, 1, 2) */
static int raw_read_fs_block(const pfs3_fs_t *fs, uint64_t fs_block, void *buf) {
    if (!fs || !fs->bdev || !buf) return PFS3_ERR_INVALID;
    if (fs_block >= fs->total_fs_blocks) return PFS3_ERR_OUT_OF_BOUNDS;
    if (fs_block > UINT64_MAX / 8) return PFS3_ERR_OVERFLOW;

    uint64_t p_rel_sector = fs_block * 8;
    if (p_rel_sector > fs->part_sector_count - 8) return PFS3_ERR_OUT_OF_BOUNDS;

    uint64_t device_lba = fs->part_start_lba + p_rel_sector;
    if (device_lba > fs->bdev->total_blocks - 8) return PFS3_ERR_IO;

    int rc = block_dev_read(fs->bdev, device_lba, 8, buf);
    return (rc == BLOCK_DEV_OK) ? PFS3_OK : PFS3_ERR_IO;
}

/* Raw write of any block within partition (including blocks 0, 1, 2) */
static int raw_write_fs_block(const pfs3_fs_t *fs, uint64_t fs_block, const void *buf) {
    if (!fs || !fs->bdev || !buf) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (fs_block >= fs->total_fs_blocks) return PFS3_ERR_OUT_OF_BOUNDS;
    if (fs_block > UINT64_MAX / 8) return PFS3_ERR_OVERFLOW;

    uint64_t p_rel_sector = fs_block * 8;
    if (p_rel_sector > fs->part_sector_count - 8) return PFS3_ERR_OUT_OF_BOUNDS;

    uint64_t device_lba = fs->part_start_lba + p_rel_sector;
    if (device_lba > fs->bdev->total_blocks - 8) return PFS3_ERR_IO;

    int rc = block_dev_write(fs->bdev, device_lba, 8, buf);
    return (rc == BLOCK_DEV_OK) ? PFS3_OK : PFS3_ERR_IO;
}

/* Public read of data block: enforces block 0 null/sparse sentinel */
int pfs3_read_block(pfs3_fs_t *fs, uint64_t fs_block, void *buf) {
    if (!fs || !fs->mounted || !buf) return PFS3_ERR_INVALID;
    if (fs_block == 0) {
        /* Block 0 is the null/sparse sentinel: reads return all zeros */
        memset(buf, 0, PFS3_BLOCK_SIZE);
        return PFS3_OK;
    }
    return raw_read_fs_block(fs, fs_block, buf);
}

/* Public write of data block: rejects block 0 modification */
int pfs3_write_block(pfs3_fs_t *fs, uint64_t fs_block, const void *buf) {
    if (!fs || !fs->mounted || !buf) return PFS3_ERR_INVALID;
    if (fs_block == 0) return PFS3_ERR_OUT_OF_BOUNDS; /* Cannot write to block 0 sentinel */
    return raw_write_fs_block(fs, fs_block, buf);
}

/* ========================================================================= */
/* Bitmap Management Helpers                                                 */
/* ========================================================================= */

static int bitmap_test_bit(const uint8_t *bitmap, uint64_t bit_index) {
    return (bitmap[bit_index / 8] & (1U << (bit_index % 8))) != 0;
}

static void bitmap_set_bit(uint8_t *bitmap, uint64_t bit_index) {
    bitmap[bit_index / 8] |= (1U << (bit_index % 8));
}

static void bitmap_clear_bit(uint8_t *bitmap, uint64_t bit_index) {
    bitmap[bit_index / 8] &= ~(1U << (bit_index % 8));
}

/* ========================================================================= */
/* Formatting Implementation                                                 */
/* ========================================================================= */

int pfs3_format(block_device_t *bdev,
                uint64_t part_start_lba,
                uint64_t part_sector_count,
                const char *label,
                const uint8_t uuid[16]) {
    if (!bdev) return PFS3_ERR_INVALID;
    if (bdev->total_blocks == 0 || part_sector_count == 0) return PFS3_ERR_INVALID_PARTITION;
    if (part_start_lba >= bdev->total_blocks) return PFS3_ERR_INVALID_PARTITION;
    if (part_sector_count > bdev->total_blocks - part_start_lba) return PFS3_ERR_INVALID_PARTITION;
    if (part_sector_count < 8 * 32) return PFS3_ERR_INVALID_PARTITION; /* Minimum 32 blocks */

    uint64_t total_blocks = part_sector_count / 8;
    if (total_blocks < 32) return PFS3_ERR_INVALID_PARTITION;

    /* Scalable layout computation */
    uint64_t bb_blocks = (total_blocks + 32767) / 32768;
    uint64_t total_inodes = (total_blocks > 1000000ULL) ? 32768ULL :
                            ((total_blocks > 100000ULL) ? 8192ULL : 1024ULL);
    uint64_t ib_blocks = (total_inodes + 32767) / 32768;
    uint64_t it_blocks = (total_inodes * PFS3_INODE_SIZE) / PFS3_BLOCK_SIZE;
    uint64_t j_blocks = (total_blocks > 1000000ULL) ? 8192ULL :
                        ((total_blocks > 100000ULL) ? 2048ULL : 32ULL);

    uint64_t block_bitmap_start = 3;
    uint64_t inode_bitmap_start = block_bitmap_start + bb_blocks;
    uint64_t inode_table_start = inode_bitmap_start + ib_blocks;
    uint64_t journal_start = inode_table_start + it_blocks;
    uint64_t data_start = journal_start + j_blocks;

    if (data_start >= total_blocks) return PFS3_ERR_NO_SPACE;

    uint64_t reserved_metadata_blocks = data_start;
    /* One data block is allocated to the root directory */
    uint64_t free_blocks = total_blocks - reserved_metadata_blocks - 1;

    /* Temporary fs object to perform raw block writes */
    pfs3_fs_t tmp_fs;
    memset(&tmp_fs, 0, sizeof(tmp_fs));
    tmp_fs.bdev = bdev;
    tmp_fs.part_start_lba = part_start_lba;
    tmp_fs.part_sector_count = part_sector_count;
    tmp_fs.total_fs_blocks = total_blocks;
    tmp_fs.read_only = 0;

    /* 1. Build Superblock */
    static pfs3_superblock_t sb;
    memset(&sb, 0, sizeof(sb));
    sb.magic = PFS3_MAGIC;
    sb.version = PFS3_VERSION;
    sb.block_size = PFS3_BLOCK_SIZE;
    sb.sector_size = PFS3_SECTOR_SIZE;
    sb.total_blocks = total_blocks;
    sb.free_blocks = free_blocks;
    sb.total_inodes = total_inodes;
    sb.free_inodes = total_inodes - 2; /* Inodes 1 (reserved) and 2 (root dir) used */
    sb.root_inode = PFS3_ROOT_INODE;
    sb.block_bitmap_start = block_bitmap_start;
    sb.block_bitmap_blocks = bb_blocks;
    sb.inode_bitmap_start = inode_bitmap_start;
    sb.inode_bitmap_blocks = ib_blocks;
    sb.inode_table_start = inode_table_start;
    sb.inode_table_blocks = it_blocks;
    sb.journal_start = journal_start;
    sb.journal_blocks = j_blocks;
    sb.data_start = data_start;
    sb.state_flags = PFS3_STATE_CLEAN;

    if (uuid) {
        memcpy(sb.volume_uuid, uuid, 16);
    } else {
        memset(sb.volume_uuid, 0x42, 16);
    }

    if (label) {
        strncpy(sb.volume_label, label, sizeof(sb.volume_label) - 1);
    } else {
        strncpy(sb.volume_label, "PurpleSystem", sizeof(sb.volume_label) - 1);
    }

    sb.checksum = pfs3_crc32(&sb, sizeof(pfs3_superblock_t) - 4);

    /* Write Block 0 (Partition Boot / Reserved zero block) */
    static uint8_t zero_block[PFS3_BLOCK_SIZE];
    memset(zero_block, 0, sizeof(zero_block));
    raw_write_fs_block(&tmp_fs, 0, zero_block);

    /* Write Primary Superblock (Block 1) and Backup Superblock (Block 2) */
    if (raw_write_fs_block(&tmp_fs, 1, &sb) != PFS3_OK) return PFS3_ERR_IO;
    if (raw_write_fs_block(&tmp_fs, 2, &sb) != PFS3_OK) return PFS3_ERR_IO;

    /* 2. Write Block Allocation Bitmaps */
    for (uint64_t b = 0; b < bb_blocks; b++) {
        memset(zero_block, 0, sizeof(zero_block));
        uint64_t blk_start = b * 32768ULL;
        for (uint64_t bit = 0; bit < 32768ULL; bit++) {
            uint64_t current_blk = blk_start + bit;
            if (current_blk < reserved_metadata_blocks) {
                bitmap_set_bit(zero_block, bit); /* Metadata is reserved/allocated */
            } else if (current_blk == data_start) {
                bitmap_set_bit(zero_block, bit); /* Root directory block is allocated */
            } else if (current_blk >= total_blocks) {
                bitmap_set_bit(zero_block, bit); /* Out-of-range trailing bits marked used */
            }
        }
        if (raw_write_fs_block(&tmp_fs, block_bitmap_start + b, zero_block) != PFS3_OK) {
            return PFS3_ERR_IO;
        }
    }

    /* 3. Write Inode Allocation Bitmaps */
    for (uint64_t b = 0; b < ib_blocks; b++) {
        memset(zero_block, 0, sizeof(zero_block));
        if (b == 0) {
            bitmap_set_bit(zero_block, 0); /* Inode 0 is null sentinel */
            bitmap_set_bit(zero_block, 1); /* Inode 1 is bad/journal reserved */
            bitmap_set_bit(zero_block, 2); /* Inode 2 is root directory */
        }
        if (raw_write_fs_block(&tmp_fs, inode_bitmap_start + b, zero_block) != PFS3_OK) {
            return PFS3_ERR_IO;
        }
    }

    /* 4. Initialize Inode Table */
    memset(zero_block, 0, sizeof(zero_block));
    for (uint64_t b = 0; b < it_blocks; b++) {
        if (b == 0) {
            /* Create Root Directory Inode at index 2 */
            pfs3_inode_t *inodes = (pfs3_inode_t *)zero_block;
            pfs3_inode_t *root_ino = &inodes[PFS3_ROOT_INODE];
            memset(root_ino, 0, sizeof(pfs3_inode_t));
            root_ino->mode = PFS3_MODE_DIR | 0755U;
            root_ino->links_count = 2; /* Self + '.' */
            root_ino->size_bytes = PFS3_BLOCK_SIZE;
            root_ino->direct[0] = data_start;
            root_ino->checksum = pfs3_crc32(root_ino, 0xA8);

            if (raw_write_fs_block(&tmp_fs, inode_table_start + b, zero_block) != PFS3_OK) {
                return PFS3_ERR_IO;
            }
            memset(zero_block, 0, sizeof(zero_block));
        } else {
            if (raw_write_fs_block(&tmp_fs, inode_table_start + b, zero_block) != PFS3_OK) {
                return PFS3_ERR_IO;
            }
        }
    }

    /* 5. Initialize Journal */
    memset(zero_block, 0, sizeof(zero_block));
    pfs3_journal_header_t *j_hdr = (pfs3_journal_header_t *)zero_block;
    j_hdr->magic = PFS3_JOURNAL_MAGIC;
    j_hdr->state = PFS3_JOURNAL_STATE_EMPTY;
    j_hdr->header_crc = pfs3_crc32(j_hdr, sizeof(pfs3_journal_header_t) - 3840);
    for (uint64_t b = 0; b < j_blocks; b++) {
        if (raw_write_fs_block(&tmp_fs, journal_start + b, zero_block) != PFS3_OK) {
            return PFS3_ERR_IO;
        }
        if (b == 0) {
            memset(zero_block, 0, sizeof(zero_block));
        }
    }

    /* 6. Initialize Root Directory Data Block (at data_start) */
    memset(zero_block, 0, sizeof(zero_block));
    pfs3_dirent_t *dirents = (pfs3_dirent_t *)zero_block;

    /* Slot 0: '.' pointing to root inode 2 */
    dirents[0].inode_num = PFS3_ROOT_INODE;
    dirents[0].file_type = PFS3_FT_DIR;
    dirents[0].name_len = 1;
    dirents[0].name[0] = '.';
    dirents[0].checksum = pfs3_crc16(&dirents[0], 0x0C);

    /* Slot 1: '..' pointing to parent directory (root inode 2) */
    dirents[1].inode_num = PFS3_ROOT_INODE;
    dirents[1].file_type = PFS3_FT_DIR;
    dirents[1].name_len = 2;
    dirents[1].name[0] = '.';
    dirents[1].name[1] = '.';
    dirents[1].checksum = pfs3_crc16(&dirents[1], 0x0C);

    if (raw_write_fs_block(&tmp_fs, data_start, zero_block) != PFS3_OK) {
        return PFS3_ERR_IO;
    }

    block_dev_flush(bdev);
    return PFS3_OK;
}

/* ========================================================================= */
/* Mount & Unmount Implementation                                            */
/* ========================================================================= */

int pfs3_mount(pfs3_fs_t *fs,
               block_device_t *bdev,
               uint64_t part_start_lba,
               uint64_t part_sector_count) {
    if (!fs || !bdev) return PFS3_ERR_INVALID;
    if (bdev->total_blocks == 0 || part_sector_count == 0) return PFS3_ERR_INVALID_PARTITION;
    if (part_start_lba >= bdev->total_blocks) return PFS3_ERR_INVALID_PARTITION;
    if (part_sector_count > bdev->total_blocks - part_start_lba) return PFS3_ERR_INVALID_PARTITION;
    if (part_sector_count < 8 * 32) return PFS3_ERR_INVALID_PARTITION;

    memset(fs, 0, sizeof(*fs));
    fs->bdev = bdev;
    fs->part_start_lba = part_start_lba;
    fs->part_sector_count = part_sector_count;
    fs->total_fs_blocks = part_sector_count / 8;
    fs->read_only = (bdev->flags & BLOCK_DEV_FLAG_READONLY) ? 1 : 0;

    /* Read Primary Superblock (Block 1) */
    int primary_valid = 0;
    if (raw_read_fs_block(fs, 1, &fs->sb) == PFS3_OK) {
        uint32_t calc = pfs3_crc32(&fs->sb, sizeof(pfs3_superblock_t) - 4);
        if (fs->sb.magic == PFS3_MAGIC && fs->sb.version == PFS3_VERSION && calc == fs->sb.checksum) {
            primary_valid = 1;
        }
    }

    /* If primary corrupted, fallback to Backup Superblock (Block 2) */
    if (!primary_valid) {
        if (raw_read_fs_block(fs, 2, &fs->sb) != PFS3_OK) {
            return PFS3_ERR_CORRUPT;
        }
        uint32_t calc = pfs3_crc32(&fs->sb, sizeof(pfs3_superblock_t) - 4);
        if (fs->sb.magic != PFS3_MAGIC || fs->sb.version != PFS3_VERSION || calc != fs->sb.checksum) {
            return PFS3_ERR_CORRUPT;
        }
    }

    if (fs->sb.block_size != PFS3_BLOCK_SIZE || fs->sb.sector_size != PFS3_SECTOR_SIZE) {
        return PFS3_ERR_CORRUPT;
    }

    fs->total_fs_blocks = fs->sb.total_blocks;
    fs->mounted = 1;

    /* Increment mount count if writable */
    if (!fs->read_only) {
        fs->sb.mount_count++;
        fs->sb.state_flags &= ~PFS3_STATE_CLEAN;
        fs->sb.checksum = pfs3_crc32(&fs->sb, sizeof(pfs3_superblock_t) - 4);
        raw_write_fs_block(fs, 1, &fs->sb);
        raw_write_fs_block(fs, 2, &fs->sb);
        block_dev_flush(fs->bdev);
    }

    return PFS3_OK;
}

int pfs3_unmount(pfs3_fs_t *fs) {
    if (!fs || !fs->mounted) return PFS3_ERR_INVALID;

    if (!fs->read_only) {
        fs->sb.state_flags |= PFS3_STATE_CLEAN;
        fs->sb.checksum = pfs3_crc32(&fs->sb, sizeof(pfs3_superblock_t) - 4);
        raw_write_fs_block(fs, 1, &fs->sb);
        raw_write_fs_block(fs, 2, &fs->sb);
        block_dev_flush(fs->bdev);
    }

    fs->mounted = 0;
    return PFS3_OK;
}

/* ========================================================================= */
/* Inode Table Operations                                                    */
/* ========================================================================= */

int pfs3_read_inode(pfs3_fs_t *fs, uint64_t inode_num, pfs3_inode_t *out_inode) {
    if (!fs || !fs->mounted || !out_inode) return PFS3_ERR_INVALID;
    if (inode_num == 0 || inode_num >= fs->sb.total_inodes) return PFS3_ERR_NO_INODE;

    uint64_t blk_index = fs->sb.inode_table_start + (inode_num / PFS3_INODES_PER_BLOCK);
    uint32_t slot_index = (uint32_t)(inode_num % PFS3_INODES_PER_BLOCK);

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    if (raw_read_fs_block(fs, blk_index, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    const pfs3_inode_t *table = (const pfs3_inode_t *)blk_buf;
    memcpy(out_inode, &table[slot_index], sizeof(pfs3_inode_t));

    uint32_t calc = pfs3_crc32(out_inode, 0xA8);
    if (calc != out_inode->checksum) return PFS3_ERR_CORRUPT;

    return PFS3_OK;
}

int pfs3_write_inode(pfs3_fs_t *fs, uint64_t inode_num, const pfs3_inode_t *inode) {
    if (!fs || !fs->mounted || !inode) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (inode_num == 0 || inode_num >= fs->sb.total_inodes) return PFS3_ERR_NO_INODE;

    uint64_t blk_index = fs->sb.inode_table_start + (inode_num / PFS3_INODES_PER_BLOCK);
    uint32_t slot_index = (uint32_t)(inode_num % PFS3_INODES_PER_BLOCK);

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    if (raw_read_fs_block(fs, blk_index, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    pfs3_inode_t *table = (pfs3_inode_t *)blk_buf;
    memcpy(&table[slot_index], inode, sizeof(pfs3_inode_t));
    table[slot_index].checksum = pfs3_crc32(&table[slot_index], 0xA8);

    if (raw_write_fs_block(fs, blk_index, blk_buf) != PFS3_OK) return PFS3_ERR_IO;
    return PFS3_OK;
}

int pfs3_alloc_inode(pfs3_fs_t *fs, uint16_t mode, uint64_t *out_inode_num) {
    if (!fs || !fs->mounted || !out_inode_num) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (fs->sb.free_inodes == 0) return PFS3_ERR_NO_INODE;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];

    for (uint64_t b = 0; b < fs->sb.inode_bitmap_blocks; b++) {
        uint64_t blk = fs->sb.inode_bitmap_start + b;
        if (raw_read_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

        for (uint64_t bit = 0; bit < 32768ULL; bit++) {
            uint64_t candidate = b * 32768ULL + bit;
            if (candidate >= fs->sb.total_inodes) break;
            if (candidate == 0) continue; /* Inode 0 reserved */

            if (!bitmap_test_bit(blk_buf, bit)) {
                bitmap_set_bit(blk_buf, bit);
                if (raw_write_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

                fs->sb.free_inodes--;
                *out_inode_num = candidate;

                /* Initialize and write the new inode */
                pfs3_inode_t new_ino;
                memset(&new_ino, 0, sizeof(new_ino));
                new_ino.mode = mode;
                new_ino.links_count = 1;
                new_ino.size_bytes = 0;
                new_ino.checksum = pfs3_crc32(&new_ino, 0xA8);
                pfs3_write_inode(fs, candidate, &new_ino);

                return PFS3_OK;
            }
        }
    }

    return PFS3_ERR_NO_INODE;
}

int pfs3_free_inode(pfs3_fs_t *fs, uint64_t inode_num) {
    if (!fs || !fs->mounted) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (inode_num == 0 || inode_num >= fs->sb.total_inodes) return PFS3_ERR_NO_INODE;

    uint64_t b = inode_num / 32768ULL;
    uint64_t bit = inode_num % 32768ULL;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    uint64_t blk = fs->sb.inode_bitmap_start + b;
    if (raw_read_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    if (!bitmap_test_bit(blk_buf, bit)) {
        return PFS3_OK; /* Already free */
    }

    bitmap_clear_bit(blk_buf, bit);
    if (raw_write_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    fs->sb.free_inodes++;

    /* Clear on-disk inode record */
    pfs3_inode_t blank;
    memset(&blank, 0, sizeof(blank));
    pfs3_write_inode(fs, inode_num, &blank);

    return PFS3_OK;
}

/* ========================================================================= */
/* Block Allocation Implementation                                           */
/* ========================================================================= */

int pfs3_alloc_block(pfs3_fs_t *fs, uint64_t *out_fs_block) {
    if (!fs || !fs->mounted || !out_fs_block) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (fs->sb.free_blocks == 0) return PFS3_ERR_NO_SPACE;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];

    for (uint64_t b = 0; b < fs->sb.block_bitmap_blocks; b++) {
        uint64_t blk = fs->sb.block_bitmap_start + b;
        if (raw_read_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

        for (uint64_t bit = 0; bit < 32768ULL; bit++) {
            uint64_t candidate = b * 32768ULL + bit;
            if (candidate >= fs->sb.total_blocks) break;
            if (candidate < fs->sb.data_start) continue;

            if (!bitmap_test_bit(blk_buf, bit)) {
                bitmap_set_bit(blk_buf, bit);
                if (raw_write_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

                fs->sb.free_blocks--;
                *out_fs_block = candidate;

                /* Zero out allocated data block */
                static uint8_t zero[PFS3_BLOCK_SIZE];
                memset(zero, 0, sizeof(zero));
                raw_write_fs_block(fs, candidate, zero);

                return PFS3_OK;
            }
        }
    }

    return PFS3_ERR_NO_SPACE;
}

int pfs3_free_block(pfs3_fs_t *fs, uint64_t fs_block) {
    if (!fs || !fs->mounted) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (fs_block < fs->sb.data_start || fs_block >= fs->sb.total_blocks) return PFS3_ERR_OUT_OF_BOUNDS;

    uint64_t b = fs_block / 32768ULL;
    uint64_t bit = fs_block % 32768ULL;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    uint64_t blk = fs->sb.block_bitmap_start + b;
    if (raw_read_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    if (!bitmap_test_bit(blk_buf, bit)) {
        return PFS3_OK; /* Already free */
    }

    bitmap_clear_bit(blk_buf, bit);
    if (raw_write_fs_block(fs, blk, blk_buf) != PFS3_OK) return PFS3_ERR_IO;

    fs->sb.free_blocks++;
    return PFS3_OK;
}

/* ========================================================================= */
/* File Block Mapping & I/O                                                  */
/* ========================================================================= */

/* Resolve or allocate file-relative block index */
static int inode_get_or_alloc_block(pfs3_fs_t *fs,
                                   pfs3_inode_t *ino,
                                   uint64_t file_block_idx,
                                   int alloc,
                                   uint64_t *out_phys_block) {
    *out_phys_block = 0;

    /* 1. Direct blocks: 0 .. 11 */
    if (file_block_idx < PFS3_DIRECT_BLOCKS) {
        if (ino->direct[file_block_idx] == 0) {
            if (!alloc) return PFS3_OK; /* Sparse hole */
            uint64_t nb = 0;
            int rc = pfs3_alloc_block(fs, &nb);
            if (rc != PFS3_OK) return rc;
            ino->direct[file_block_idx] = nb;
        }
        *out_phys_block = ino->direct[file_block_idx];
        return PFS3_OK;
    }

    file_block_idx -= PFS3_DIRECT_BLOCKS;

    /* 2. Single Indirect: 512 blocks (48 KiB .. 2 MiB + 48 KiB) */
    if (file_block_idx < PFS3_PTRS_PER_INDIRECT_BLOCK) {
        if (ino->indirect == 0) {
            if (!alloc) return PFS3_OK;
            uint64_t nb = 0;
            int rc = pfs3_alloc_block(fs, &nb);
            if (rc != PFS3_OK) return rc;
            ino->indirect = nb;
        }

        static pfs3_indirect_block_t ib;
        if (pfs3_read_block(fs, ino->indirect, &ib) != PFS3_OK) return PFS3_ERR_IO;

        if (ib.pointers[file_block_idx] == 0) {
            if (!alloc) return PFS3_OK;
            uint64_t nb = 0;
            int rc = pfs3_alloc_block(fs, &nb);
            if (rc != PFS3_OK) return rc;
            ib.pointers[file_block_idx] = nb;
            if (pfs3_write_block(fs, ino->indirect, &ib) != PFS3_OK) return PFS3_ERR_IO;
        }
        *out_phys_block = ib.pointers[file_block_idx];
        return PFS3_OK;
    }

    return PFS3_ERR_TOO_BIG;
}

int pfs3_read_file(pfs3_fs_t *fs,
                   uint64_t inode_num,
                   void *buf,
                   uint64_t count,
                   uint64_t offset,
                   uint64_t *out_read) {
    if (!fs || !fs->mounted || !buf || !out_read) return PFS3_ERR_INVALID;
    *out_read = 0;

    pfs3_inode_t ino;
    int rc = pfs3_read_inode(fs, inode_num, &ino);
    if (rc != PFS3_OK) return rc;

    if (offset >= ino.size_bytes) {
        return PFS3_OK; /* EOF */
    }

    uint64_t to_read = count;
    if (to_read > ino.size_bytes - offset) {
        to_read = ino.size_bytes - offset;
    }

    uint8_t *dst = (uint8_t *)buf;
    uint64_t bytes_left = to_read;
    uint64_t cur_offset = offset;
    static uint8_t blk_buf[PFS3_BLOCK_SIZE];

    while (bytes_left > 0) {
        uint64_t blk_idx = cur_offset / PFS3_BLOCK_SIZE;
        uint32_t blk_off = (uint32_t)(cur_offset % PFS3_BLOCK_SIZE);
        uint32_t chunk = PFS3_BLOCK_SIZE - blk_off;
        if (chunk > bytes_left) chunk = (uint32_t)bytes_left;

        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &ino, blk_idx, 0, &phys_blk);
        if (rc != PFS3_OK) return rc;

        if (phys_blk == 0) {
            /* Sparse hole */
            memset(dst, 0, chunk);
        } else {
            rc = pfs3_read_block(fs, phys_blk, blk_buf);
            if (rc != PFS3_OK) return rc;
            memcpy(dst, blk_buf + blk_off, chunk);
        }

        dst += chunk;
        cur_offset += chunk;
        bytes_left -= chunk;
    }

    *out_read = to_read;
    return PFS3_OK;
}

int pfs3_write_file(pfs3_fs_t *fs,
                    uint64_t inode_num,
                    const void *buf,
                    uint64_t count,
                    uint64_t offset) {
    if (!fs || !fs->mounted || !buf) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;
    if (count == 0) return PFS3_OK;

    pfs3_inode_t ino;
    int rc = pfs3_read_inode(fs, inode_num, &ino);
    if (rc != PFS3_OK) return rc;

    const uint8_t *src = (const uint8_t *)buf;
    uint64_t bytes_left = count;
    uint64_t cur_offset = offset;
    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    int ino_dirty = 0;

    while (bytes_left > 0) {
        uint64_t blk_idx = cur_offset / PFS3_BLOCK_SIZE;
        uint32_t blk_off = (uint32_t)(cur_offset % PFS3_BLOCK_SIZE);
        uint32_t chunk = PFS3_BLOCK_SIZE - blk_off;
        if (chunk > bytes_left) chunk = (uint32_t)bytes_left;

        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &ino, blk_idx, 1, &phys_blk);
        if (rc != PFS3_OK) return rc;
        ino_dirty = 1;

        if (chunk == PFS3_BLOCK_SIZE) {
            memcpy(blk_buf, src, PFS3_BLOCK_SIZE);
        } else {
            pfs3_read_block(fs, phys_blk, blk_buf);
            memcpy(blk_buf + blk_off, src, chunk);
        }

        rc = pfs3_write_block(fs, phys_blk, blk_buf);
        if (rc != PFS3_OK) return rc;

        src += chunk;
        cur_offset += chunk;
        bytes_left -= chunk;
    }

    if (cur_offset > ino.size_bytes) {
        ino.size_bytes = cur_offset;
        ino_dirty = 1;
    }

    if (ino_dirty) {
        rc = pfs3_write_inode(fs, inode_num, &ino);
        if (rc != PFS3_OK) return rc;
    }

    return PFS3_OK;
}

int pfs3_truncate_file(pfs3_fs_t *fs, uint64_t inode_num, uint64_t new_size) {
    if (!fs || !fs->mounted) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;

    pfs3_inode_t ino;
    int rc = pfs3_read_inode(fs, inode_num, &ino);
    if (rc != PFS3_OK) return rc;

    if (new_size >= ino.size_bytes) {
        ino.size_bytes = new_size;
        return pfs3_write_inode(fs, inode_num, &ino);
    }

    uint64_t keep_blocks = (new_size + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;
    uint64_t old_blocks = (ino.size_bytes + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;

    for (uint64_t b = keep_blocks; b < old_blocks; b++) {
        uint64_t phys = 0;
        inode_get_or_alloc_block(fs, &ino, b, 0, &phys);
        if (phys != 0) {
            pfs3_free_block(fs, phys);
            if (b < PFS3_DIRECT_BLOCKS) {
                ino.direct[b] = 0;
            }
        }
    }

    ino.size_bytes = new_size;
    return pfs3_write_inode(fs, inode_num, &ino);
}

/* ========================================================================= */
/* Directory Operations                                                      */
/* ========================================================================= */

static int validate_name(const char *name, size_t *out_len) {
    if (!name) return PFS3_ERR_INVALID_NAME;
    size_t len = strlen(name);
    if (len == 0 || len > PFS3_MAX_NAME_LEN) return PFS3_ERR_INVALID_NAME;

    for (size_t i = 0; i < len; i++) {
        unsigned char c = (unsigned char)name[i];
        if (c == '/' || (c < 0x20 && c != 0)) {
            return PFS3_ERR_INVALID_NAME;
        }
    }
    *out_len = len;
    return PFS3_OK;
}

int pfs3_lookup_entry(pfs3_fs_t *fs,
                      uint64_t dir_inode,
                      const char *name,
                      uint64_t *out_inode,
                      uint8_t *out_file_type) {
    if (!fs || !fs->mounted || !name || !out_inode) return PFS3_ERR_INVALID;

    size_t nlen = 0;
    if (strcmp(name, ".") == 0 || strcmp(name, "..") == 0) {
        nlen = strlen(name);
    } else {
        int rc = validate_name(name, &nlen);
        if (rc != PFS3_OK) return rc;
    }

    pfs3_inode_t dino;
    int rc = pfs3_read_inode(fs, dir_inode, &dino);
    if (rc != PFS3_OK) return rc;
    if ((dino.mode & 0xF000U) != PFS3_MODE_DIR) return PFS3_ERR_NOT_DIR;

    uint64_t dir_blocks = (dino.size_bytes + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;
    if (dir_blocks == 0) dir_blocks = 1;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];

    for (uint64_t b = 0; b < dir_blocks; b++) {
        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &dino, b, 0, &phys_blk);
        if (rc != PFS3_OK || phys_blk == 0) continue;

        rc = pfs3_read_block(fs, phys_blk, blk_buf);
        if (rc != PFS3_OK) return rc;

        pfs3_dirent_t *dirents = (pfs3_dirent_t *)blk_buf;
        for (uint32_t i = 0; i < PFS3_DIRENTS_PER_BLOCK; i++) {
            if (dirents[i].inode_num == 0) continue;

            uint16_t calc = pfs3_crc16(&dirents[i], 0x0C);
            if (calc != dirents[i].checksum) {
                return PFS3_ERR_DIR_CORRUPT;
            }

            if (dirents[i].name_len == nlen && memcmp(dirents[i].name, name, nlen) == 0) {
                *out_inode = dirents[i].inode_num;
                if (out_file_type) *out_file_type = dirents[i].file_type;
                return PFS3_OK;
            }
        }
    }

    return PFS3_ERR_NOT_FOUND;
}

int pfs3_create_entry(pfs3_fs_t *fs,
                      uint64_t dir_inode,
                      const char *name,
                      uint8_t file_type,
                      uint64_t *out_inode) {
    if (!fs || !fs->mounted || !name || !out_inode) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;

    size_t nlen = 0;
    int rc = validate_name(name, &nlen);
    if (rc != PFS3_OK) return rc;

    /* Check if already exists */
    uint64_t existing_ino = 0;
    if (pfs3_lookup_entry(fs, dir_inode, name, &existing_ino, NULL) == PFS3_OK) {
        return PFS3_ERR_EXISTS;
    }

    pfs3_inode_t dino;
    rc = pfs3_read_inode(fs, dir_inode, &dino);
    if (rc != PFS3_OK) return rc;
    if ((dino.mode & 0xF000U) != PFS3_MODE_DIR) return PFS3_ERR_NOT_DIR;

    uint64_t new_ino_num = 0;
    uint16_t mode = (file_type == PFS3_FT_DIR) ? (PFS3_MODE_DIR | 0755U) : (PFS3_MODE_REG | 0644U);
    rc = pfs3_alloc_inode(fs, mode, &new_ino_num);
    if (rc != PFS3_OK) return rc;

    /* If it is a directory, initialize . and .. in its data block */
    if (file_type == PFS3_FT_DIR) {
        uint64_t dblk = 0;
        rc = pfs3_alloc_block(fs, &dblk);
        if (rc != PFS3_OK) {
            pfs3_free_inode(fs, new_ino_num);
            return rc;
        }

        pfs3_inode_t child_ino;
        pfs3_read_inode(fs, new_ino_num, &child_ino);
        child_ino.direct[0] = dblk;
        child_ino.size_bytes = PFS3_BLOCK_SIZE;
        child_ino.links_count = 2; /* Self + '.' */
        pfs3_write_inode(fs, new_ino_num, &child_ino);

        static uint8_t child_blk[PFS3_BLOCK_SIZE];
        memset(child_blk, 0, sizeof(child_blk));
        pfs3_dirent_t *cdirents = (pfs3_dirent_t *)child_blk;

        cdirents[0].inode_num = new_ino_num;
        cdirents[0].file_type = PFS3_FT_DIR;
        cdirents[0].name_len = 1;
        cdirents[0].name[0] = '.';
        cdirents[0].checksum = pfs3_crc16(&cdirents[0], 0x0C);

        cdirents[1].inode_num = dir_inode;
        cdirents[1].file_type = PFS3_FT_DIR;
        cdirents[1].name_len = 2;
        cdirents[1].name[0] = '.';
        cdirents[1].name[1] = '.';
        cdirents[1].checksum = pfs3_crc16(&cdirents[1], 0x0C);

        pfs3_write_block(fs, dblk, child_blk);

        /* Parent gains a link from child's .. */
        dino.links_count++;
    }

    /* Find free slot in parent directory */
    uint64_t dir_blocks = (dino.size_bytes + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;
    if (dir_blocks == 0) dir_blocks = 1;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    int slot_found = 0;

    for (uint64_t b = 0; b < dir_blocks; b++) {
        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &dino, b, 1, &phys_blk);
        if (rc != PFS3_OK) break;

        rc = pfs3_read_block(fs, phys_blk, blk_buf);
        if (rc != PFS3_OK) continue;

        pfs3_dirent_t *dirents = (pfs3_dirent_t *)blk_buf;
        for (uint32_t i = 0; i < PFS3_DIRENTS_PER_BLOCK; i++) {
            if (dirents[i].inode_num == 0) {
                memset(&dirents[i], 0, sizeof(pfs3_dirent_t));
                dirents[i].inode_num = new_ino_num;
                dirents[i].file_type = file_type;
                dirents[i].name_len = (uint8_t)nlen;
                memcpy(dirents[i].name, name, nlen);
                dirents[i].checksum = pfs3_crc16(&dirents[i], 0x0C);

                pfs3_write_block(fs, phys_blk, blk_buf);
                slot_found = 1;
                break;
            }
        }
        if (slot_found) break;
    }

    /* If no slot found in existing blocks, allocate a new block for parent dir */
    if (!slot_found) {
        uint64_t new_blk_idx = dir_blocks;
        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &dino, new_blk_idx, 1, &phys_blk);
        if (rc != PFS3_OK) {
            pfs3_free_inode(fs, new_ino_num);
            return rc;
        }

        memset(blk_buf, 0, sizeof(blk_buf));
        pfs3_dirent_t *dirents = (pfs3_dirent_t *)blk_buf;
        dirents[0].inode_num = new_ino_num;
        dirents[0].file_type = file_type;
        dirents[0].name_len = (uint8_t)nlen;
        memcpy(dirents[0].name, name, nlen);
        dirents[0].checksum = pfs3_crc16(&dirents[0], 0x0C);

        pfs3_write_block(fs, phys_blk, blk_buf);
        dino.size_bytes = (new_blk_idx + 1) * PFS3_BLOCK_SIZE;
    }

    pfs3_write_inode(fs, dir_inode, &dino);
    *out_inode = new_ino_num;
    return PFS3_OK;
}

int pfs3_unlink_entry(pfs3_fs_t *fs, uint64_t dir_inode, const char *name) {
    if (!fs || !fs->mounted || !name) return PFS3_ERR_INVALID;
    if (fs->read_only) return PFS3_ERR_ROFS;

    if (strcmp(name, ".") == 0 || strcmp(name, "..") == 0) {
        return PFS3_ERR_INVALID;
    }

    size_t nlen = 0;
    int rc = validate_name(name, &nlen);
    if (rc != PFS3_OK) return rc;

    pfs3_inode_t dino;
    rc = pfs3_read_inode(fs, dir_inode, &dino);
    if (rc != PFS3_OK) return rc;
    if ((dino.mode & 0xF000U) != PFS3_MODE_DIR) return PFS3_ERR_NOT_DIR;

    uint64_t dir_blocks = (dino.size_bytes + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;
    if (dir_blocks == 0) dir_blocks = 1;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];

    for (uint64_t b = 0; b < dir_blocks; b++) {
        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &dino, b, 0, &phys_blk);
        if (rc != PFS3_OK || phys_blk == 0) continue;

        rc = pfs3_read_block(fs, phys_blk, blk_buf);
        if (rc != PFS3_OK) return rc;

        pfs3_dirent_t *dirents = (pfs3_dirent_t *)blk_buf;
        for (uint32_t i = 0; i < PFS3_DIRENTS_PER_BLOCK; i++) {
            if (dirents[i].inode_num == 0) continue;

            if (dirents[i].name_len == nlen && memcmp(dirents[i].name, name, nlen) == 0) {
                uint64_t target_ino = dirents[i].inode_num;
                uint8_t ftype = dirents[i].file_type;

                /* If directory, ensure it is empty */
                if (ftype == PFS3_FT_DIR) {
                    pfs3_dirent_t d_ent;
                    int active_count = 0;
                    for (uint32_t k = 0; pfs3_readdir(fs, target_ino, k, &d_ent) == PFS3_OK; k++) {
                        if (strcmp(d_ent.name, ".") != 0 && strcmp(d_ent.name, "..") != 0) {
                            active_count++;
                        }
                    }
                    if (active_count > 0) return PFS3_ERR_DIR_NOT_EMPTY;
                }

                /* Clear dirent */
                memset(&dirents[i], 0, sizeof(pfs3_dirent_t));
                pfs3_write_block(fs, phys_blk, blk_buf);

                /* Release target inode resources */
                pfs3_truncate_file(fs, target_ino, 0);
                pfs3_free_inode(fs, target_ino);

                if (ftype == PFS3_FT_DIR && dino.links_count > 2) {
                    dino.links_count--;
                    pfs3_write_inode(fs, dir_inode, &dino);
                }

                return PFS3_OK;
            }
        }
    }

    return PFS3_ERR_NOT_FOUND;
}

int pfs3_mkdir(pfs3_fs_t *fs, uint64_t parent_inode, const char *name, uint64_t *out_inode) {
    return pfs3_create_entry(fs, parent_inode, name, PFS3_FT_DIR, out_inode);
}

int pfs3_rmdir(pfs3_fs_t *fs, uint64_t parent_inode, const char *name) {
    return pfs3_unlink_entry(fs, parent_inode, name);
}

int pfs3_readdir(pfs3_fs_t *fs,
                 uint64_t dir_inode,
                 uint32_t entry_index,
                 pfs3_dirent_t *out_dirent) {
    if (!fs || !fs->mounted || !out_dirent) return PFS3_ERR_INVALID;

    pfs3_inode_t dino;
    int rc = pfs3_read_inode(fs, dir_inode, &dino);
    if (rc != PFS3_OK) return rc;
    if ((dino.mode & 0xF000U) != PFS3_MODE_DIR) return PFS3_ERR_NOT_DIR;

    uint64_t dir_blocks = (dino.size_bytes + PFS3_BLOCK_SIZE - 1) / PFS3_BLOCK_SIZE;
    if (dir_blocks == 0) dir_blocks = 1;

    static uint8_t blk_buf[PFS3_BLOCK_SIZE];
    uint32_t current_valid = 0;

    for (uint64_t b = 0; b < dir_blocks; b++) {
        uint64_t phys_blk = 0;
        rc = inode_get_or_alloc_block(fs, &dino, b, 0, &phys_blk);
        if (rc != PFS3_OK || phys_blk == 0) continue;

        rc = pfs3_read_block(fs, phys_blk, blk_buf);
        if (rc != PFS3_OK) return rc;

        pfs3_dirent_t *dirents = (pfs3_dirent_t *)blk_buf;
        for (uint32_t i = 0; i < PFS3_DIRENTS_PER_BLOCK; i++) {
            if (dirents[i].inode_num != 0) {
                if (current_valid == entry_index) {
                    memcpy(out_dirent, &dirents[i], sizeof(pfs3_dirent_t));
                    return PFS3_OK;
                }
                current_valid++;
            }
        }
    }

    return PFS3_ERR_NOT_FOUND;
}

/* ========================================================================= */
/* Path Resolution (Hierarchical with . and ..)                              */
/* ========================================================================= */

int pfs3_lookup_path(pfs3_fs_t *fs, const char *path, uint64_t *out_inode) {
    if (!fs || !fs->mounted || !path || !out_inode) return PFS3_ERR_INVALID;

    uint64_t cur_inode = fs->sb.root_inode;

    /* Skip leading slashes */
    const char *p = path;
    while (*p == '/') p++;

    if (*p == '\0') {
        *out_inode = cur_inode;
        return PFS3_OK;
    }

    char comp[PFS3_MAX_NAME_LEN + 1];

    while (*p != '\0') {
        size_t c_len = 0;
        while (*p != '\0' && *p != '/') {
            if (c_len < PFS3_MAX_NAME_LEN) {
                comp[c_len++] = *p;
            } else {
                return PFS3_ERR_INVALID_NAME;
            }
            p++;
        }
        comp[c_len] = '\0';

        while (*p == '/') p++;

        uint64_t next_ino = 0;
        int rc = pfs3_lookup_entry(fs, cur_inode, comp, &next_ino, NULL);
        if (rc != PFS3_OK) return rc;

        cur_inode = next_ino;
    }

    *out_inode = cur_inode;
    return PFS3_OK;
}
