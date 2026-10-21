#include "purplefs_v2.h"
#include "string.h"

/* 16-byte PurpleFS v2 signature: "PURPLEFS\0PFS2\0\0\0" */
const uint8_t PFS2_SIGNATURE[16] = {
    'P', 'U', 'R', 'P', 'L', 'E', 'F', 'S',
    0x00,
    'P', 'F', 'S', '2',
    0x00, 0x00, 0x00
};

/* Standard CRC32 (polynomial 0xEDB88320) */
uint32_t pfs2_crc32(const void *data, size_t length) {
    const uint8_t *p = (const uint8_t *)data;
    uint32_t crc = 0xFFFFFFFF;
    for (size_t i = 0; i < length; i++) {
        crc ^= p[i];
        for (int j = 0; j < 8; j++) {
            crc = (crc >> 1) ^ ((crc & 1) ? 0xEDB88320 : 0);
        }
    }
    return ~crc;
}

/* Bitmap helpers */
static inline int bitmap_get(const uint8_t *bm, uint32_t bit) {
    return (bm[bit / 8] & (1 << (bit % 8))) != 0;
}

static inline void bitmap_set(uint8_t *bm, uint32_t bit) {
    bm[bit / 8] |= (1 << (bit % 8));
}

static inline void bitmap_clear(uint8_t *bm, uint32_t bit) {
    bm[bit / 8] &= ~(1 << (bit % 8));
}

static int alloc_block(pfs2_fs_t *fs, uint32_t *out_block) {
    for (uint32_t i = fs->sb.data_start_block; i < fs->sb.journal_start_block; i++) {
        if (!bitmap_get(fs->bitmap, i)) {
            bitmap_set(fs->bitmap, i);
            fs->sb.free_blocks--;
            *out_block = i;
            return PFS2_OK;
        }
    }
    return PFS2_ERR_NO_SPACE;
}

static void free_block(pfs2_fs_t *fs, uint32_t block) {
    if (block >= fs->sb.data_start_block && block < fs->sb.journal_start_block) {
        if (bitmap_get(fs->bitmap, block)) {
            bitmap_clear(fs->bitmap, block);
            fs->sb.free_blocks++;
        }
    }
}

/* Redo Journal Commit Implementation */
typedef struct {
    uint32_t target_block;
    uint8_t data[PFS2_BLOCK_SIZE];
} journal_slot_t;

static int journal_commit_transaction(pfs2_fs_t *fs, journal_slot_t *slots, uint32_t count) {
    if (count == 0 || count > PFS2_MAX_AFTER_IMAGES) return PFS2_ERR_INVALID;
    pfs2_block_dev_t *dev = fs->dev;
    uint32_t j_start = fs->sb.journal_start_block;

    // 1. Write after-images to journal data blocks (j_start + 1 .. j_start + count)
    for (uint32_t i = 0; i < count; i++) {
        if (dev->write_block(dev, j_start + 1 + i, slots[i].data) != 0) {
            return PFS2_ERR_IO;
        }
    }

    // 2. Flush device
    if (dev->flush(dev) != 0) {
        return PFS2_ERR_IO;
    }

    // 3. Prepare Journal Header
    pfs2_journal_header_t j_hdr;
    memset(&j_hdr, 0, sizeof(j_hdr));
    j_hdr.magic = PFS2_JOURNAL_MAGIC;
    j_hdr.journal_state = PFS2_JOURNAL_STATE_COMMITTED;
    j_hdr.sequence_number = ++fs->next_journal_seq;
    j_hdr.num_after_images = count;

    for (uint32_t i = 0; i < count; i++) {
        j_hdr.target_blocks[i] = slots[i].target_block;
        j_hdr.after_image_crcs[i] = pfs2_crc32(slots[i].data, PFS2_BLOCK_SIZE);
    }
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24);

    // 4. Write Journal Header and Flush (COMMIT BOUNDARY)
    uint8_t hdr_block[PFS2_BLOCK_SIZE];
    memset(hdr_block, 0, PFS2_BLOCK_SIZE);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));

    if (dev->write_block(dev, j_start, hdr_block) != 0) {
        return PFS2_ERR_IO;
    }
    if (dev->flush(dev) != 0) {
        return PFS2_ERR_IO;
    }

    // 5. Apply After-Images in-place
    for (uint32_t i = 0; i < count; i++) {
        if (dev->write_block(dev, slots[i].target_block, slots[i].data) != 0) {
            return PFS2_ERR_IO;
        }
    }
    if (dev->flush(dev) != 0) {
        return PFS2_ERR_IO;
    }

    // 6. Mark Transaction Applied
    j_hdr.journal_state = PFS2_JOURNAL_STATE_APPLIED;
    j_hdr.header_crc = pfs2_crc32(&j_hdr, 4 + 4 + 8 + 4 + 24 + 24);
    memcpy(hdr_block, &j_hdr, sizeof(j_hdr));

    if (dev->write_block(dev, j_start, hdr_block) != 0) {
        return PFS2_ERR_IO;
    }
    if (dev->flush(dev) != 0) {
        return PFS2_ERR_IO;
    }

    return PFS2_OK;
}

int pfs2_journal_replay_if_needed(pfs2_fs_t *fs) {
    pfs2_block_dev_t *dev = fs->dev;
    uint32_t j_start = fs->sb.journal_start_block;
    uint8_t hdr_block[PFS2_BLOCK_SIZE];

    if (dev->read_block(dev, j_start, hdr_block) != 0) {
        return PFS2_ERR_IO;
    }

    pfs2_journal_header_t *j_hdr = (pfs2_journal_header_t *)hdr_block;
    if (j_hdr->magic != PFS2_JOURNAL_MAGIC) {
        return PFS2_ERR_JOURNAL;
    }

    uint32_t expected_crc = pfs2_crc32(j_hdr, 4 + 4 + 8 + 4 + 24 + 24);
    if (j_hdr->header_crc != expected_crc) {
        return PFS2_ERR_CORRUPT;
    }

    if (j_hdr->journal_state == PFS2_JOURNAL_STATE_COMMITTED) {
        if (j_hdr->num_after_images == 0 || j_hdr->num_after_images > PFS2_MAX_AFTER_IMAGES) {
            return PFS2_ERR_CORRUPT;
        }

        // Validate all after-images
        uint8_t ai_buf[PFS2_MAX_AFTER_IMAGES][PFS2_BLOCK_SIZE];
        for (uint32_t i = 0; i < j_hdr->num_after_images; i++) {
            uint32_t target = j_hdr->target_blocks[i];
            if (target >= fs->sb.total_blocks) {
                return PFS2_ERR_CORRUPT;
            }
            if (dev->read_block(dev, j_start + 1 + i, ai_buf[i]) != 0) {
                return PFS2_ERR_IO;
            }
            if (pfs2_crc32(ai_buf[i], PFS2_BLOCK_SIZE) != j_hdr->after_image_crcs[i]) {
                return PFS2_ERR_CORRUPT;
            }
        }

        // Replay to target blocks
        for (uint32_t i = 0; i < j_hdr->num_after_images; i++) {
            if (dev->write_block(dev, j_hdr->target_blocks[i], ai_buf[i]) != 0) {
                return PFS2_ERR_IO;
            }
        }
        if (dev->flush(dev) != 0) return PFS2_ERR_IO;

        // Mark applied
        j_hdr->journal_state = PFS2_JOURNAL_STATE_APPLIED;
        j_hdr->header_crc = pfs2_crc32(j_hdr, 4 + 4 + 8 + 4 + 24 + 24);
        memcpy(hdr_block, j_hdr, sizeof(*j_hdr));
        dev->write_block(dev, j_start, hdr_block);
        dev->flush(dev);
    }

    return PFS2_OK;
}

int pfs2_format(pfs2_block_dev_t *dev, uint32_t total_blocks, const char *label) {
    if (!dev) return PFS2_ERR_INVALID;
    if (total_blocks < PFS2_MIN_BLOCKS || total_blocks > PFS2_DEFAULT_BLOCKS) {
        return PFS2_ERR_INVALID;
    }
    if (dev->block_count < total_blocks) {
        return PFS2_ERR_INVALID;
    }

    // 1. Superblock Setup
    pfs2_superblock_t sb;
    memset(&sb, 0, sizeof(sb));
    memcpy(sb.magic, PFS2_SIGNATURE, sizeof(PFS2_SIGNATURE));
    sb.version = 2;
    sb.block_size = PFS2_BLOCK_SIZE;
    sb.total_blocks = total_blocks;
    sb.total_inodes = PFS2_INODE_COUNT;
    sb.free_inodes = PFS2_INODE_COUNT - 1; // Inode 1 is root dir
    sb.bitmap_start_block = PFS2_BITMAP_BLOCK;
    sb.bitmap_block_count = 1;
    sb.inode_start_block = PFS2_INODE_TABLE_START;
    sb.inode_block_count = PFS2_INODE_TABLE_BLOCKS;
    sb.data_start_block = PFS2_DATA_START_BLOCK;
    sb.journal_start_block = total_blocks - PFS2_JOURNAL_BLOCKS;
    sb.journal_block_count = PFS2_JOURNAL_BLOCKS;
    sb.feature_flags = PFS2_FEATURE_JOURNAL;
    sb.free_blocks = sb.journal_start_block - sb.data_start_block - 1; // Minus root dir data block
    sb.clean_unmount = 1;
    sb.mount_count = 0;

    if (label) {
        strncpy(sb.volume_label, label, sizeof(sb.volume_label) - 1);
    } else {
        strncpy(sb.volume_label, "purpledata", sizeof(sb.volume_label) - 1);
    }
    sb.checksum = pfs2_crc32(&sb, 0x80);

    uint8_t sb_block[PFS2_BLOCK_SIZE];
    memset(sb_block, 0, PFS2_BLOCK_SIZE);
    memcpy(sb_block, &sb, sizeof(sb));
    if (dev->write_block(dev, 0, sb_block) != 0) return PFS2_ERR_IO;

    // 2. Block Bitmap Setup
    uint8_t bitmap[PFS2_BLOCK_SIZE];
    memset(bitmap, 0, PFS2_BLOCK_SIZE);
    // Mark Superblock (0), Bitmap (1), Inode Table (2..9), Root Dir Data (10)
    for (uint32_t i = 0; i <= PFS2_DATA_START_BLOCK; i++) {
        bitmap_set(bitmap, i);
    }
    // Mark Journal Blocks (journal_start .. total_blocks - 1)
    for (uint32_t i = sb.journal_start_block; i < total_blocks; i++) {
        bitmap_set(bitmap, i);
    }
    // Mark any blocks beyond total_blocks in the 32768-bit bitmap as allocated
    for (uint32_t i = total_blocks; i < PFS2_DEFAULT_BLOCKS; i++) {
        bitmap_set(bitmap, i);
    }
    if (dev->write_block(dev, PFS2_BITMAP_BLOCK, bitmap) != 0) return PFS2_ERR_IO;

    // 3. Inode Table Setup
    uint8_t inode_block[PFS2_BLOCK_SIZE];
    memset(inode_block, 0, PFS2_BLOCK_SIZE);

    // Inode 1: Root directory
    pfs2_inode_t *root_inode = (pfs2_inode_t *)(inode_block + PFS2_INODE_SIZE);
    root_inode->mode = PFS2_MODE_DIR | 0755;
    root_inode->links_count = 2; // '.' and parent
    root_inode->size_bytes = PFS2_BLOCK_SIZE;
    root_inode->direct_blocks[0] = PFS2_DATA_START_BLOCK; // Block 10
    root_inode->checksum = pfs2_crc32(root_inode, 0x7C);

    if (dev->write_block(dev, PFS2_INODE_TABLE_START, inode_block) != 0) return PFS2_ERR_IO;

    // Clear remaining inode blocks (3..9)
    memset(inode_block, 0, PFS2_BLOCK_SIZE);
    for (uint32_t b = PFS2_INODE_TABLE_START + 1; b < PFS2_DATA_START_BLOCK; b++) {
        if (dev->write_block(dev, b, inode_block) != 0) return PFS2_ERR_IO;
    }

    // 4. Root Directory Data Block (Block 10)
    uint8_t dir_block[PFS2_BLOCK_SIZE];
    memset(dir_block, 0, PFS2_BLOCK_SIZE);

    pfs2_dir_entry_t *dot = (pfs2_dir_entry_t *)dir_block;
    dot->inode = PFS2_ROOT_INODE;
    dot->rec_len = 32;
    dot->name_len = 1;
    dot->file_type = 2; // Directory
    strcpy(dot->name, ".");

    pfs2_dir_entry_t *dotdot = (pfs2_dir_entry_t *)(dir_block + 32);
    dotdot->inode = PFS2_ROOT_INODE;
    dotdot->rec_len = 32;
    dotdot->name_len = 2;
    dotdot->file_type = 2;
    strcpy(dotdot->name, "..");

    if (dev->write_block(dev, PFS2_DATA_START_BLOCK, dir_block) != 0) return PFS2_ERR_IO;

    // 5. Redo Journal Setup
    uint8_t j_block[PFS2_BLOCK_SIZE];
    memset(j_block, 0, PFS2_BLOCK_SIZE);
    pfs2_journal_header_t *j_hdr = (pfs2_journal_header_t *)j_block;
    j_hdr->magic = PFS2_JOURNAL_MAGIC;
    j_hdr->journal_state = PFS2_JOURNAL_STATE_EMPTY;
    j_hdr->header_crc = pfs2_crc32(j_hdr, 4 + 4 + 8 + 4 + 24 + 24);

    if (dev->write_block(dev, sb.journal_start_block, j_block) != 0) return PFS2_ERR_IO;

    // Clear remaining journal blocks
    memset(j_block, 0, PFS2_BLOCK_SIZE);
    for (uint32_t b = sb.journal_start_block + 1; b < total_blocks; b++) {
        if (dev->write_block(dev, b, j_block) != 0) return PFS2_ERR_IO;
    }

    if (dev->flush(dev) != 0) return PFS2_ERR_IO;
    return PFS2_OK;
}

int pfs2_mount(pfs2_fs_t *fs, pfs2_block_dev_t *dev) {
    extern uint64_t boot_pd_3[];
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped at entry!");
    if (!fs || !dev) return PFS2_ERR_INVALID;
    memset(fs, 0, sizeof(*fs));
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after memset(fs)!");
    fs->dev = dev;

    uint8_t sb_block[PFS2_BLOCK_SIZE];
    if (dev->read_block(dev, 0, sb_block) != 0) return PFS2_ERR_IO;
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after read_block(sb)!");

    memcpy(&fs->sb, sb_block, sizeof(pfs2_superblock_t));

    // Validate signature
    if (memcmp(fs->sb.magic, PFS2_SIGNATURE, sizeof(PFS2_SIGNATURE)) != 0) {
        return PFS2_ERR_BAD_MAGIC;
    }

    // Validate geometry
    if (fs->sb.block_size != PFS2_BLOCK_SIZE ||
        fs->sb.total_blocks < PFS2_MIN_BLOCKS ||
        fs->sb.total_blocks > PFS2_DEFAULT_BLOCKS ||
        fs->sb.total_blocks > dev->block_count) {
        return PFS2_ERR_INVALID;
    }

    // Validate feature flags
    if (!(fs->sb.feature_flags & PFS2_FEATURE_JOURNAL)) {
        return PFS2_ERR_INVALID;
    }

    // Validate Superblock CRC
    uint32_t expected_crc = pfs2_crc32(&fs->sb, 0x80);
    if (fs->sb.checksum != expected_crc) {
        return PFS2_ERR_CORRUPT;
    }

    // Read bitmap
    if (dev->read_block(dev, fs->sb.bitmap_start_block, fs->bitmap) != 0) {
        return PFS2_ERR_IO;
    }
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after read_block(bitmap)!");

    // Replay journal if interrupted commit exists
    int replay_res = pfs2_journal_replay_if_needed(fs);
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after replay_if_needed!");
    if (replay_res != PFS2_OK) {
        return replay_res;
    }

    // Read updated bitmap after potential journal replay
    dev->read_block(dev, fs->sb.bitmap_start_block, fs->bitmap);
    if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after 2nd read_block(bitmap)!");

    // Update superblock mount metadata if device is writable
    if (dev->write_block) {
        fs->sb.mount_count++;
        fs->sb.clean_unmount = 0;
        fs->sb.checksum = pfs2_crc32(&fs->sb, 0x80);
        memcpy(sb_block, &fs->sb, sizeof(fs->sb));
        int wrc = dev->write_block(dev, 0, sb_block);
        if (boot_pd_3[503] == 0) kerror("PFS2_MOUNT: wiped after write_block(sb)!");
        if (wrc == 0) {
            dev->flush(dev);
        } else {
            // Write failed or device is read-only
            fs->read_only = 1;
        }
    } else {
        fs->read_only = 1;
    }

    fs->is_mounted = 1;
    return PFS2_OK;
}

int pfs2_unmount(pfs2_fs_t *fs) {
    if (!fs || !fs->is_mounted) return PFS2_ERR_INVALID;

    // Only update superblock if mounted read-write
    if (!fs->read_only && fs->dev->write_block) {
        // Mark cleanly unmounted
        fs->sb.clean_unmount = 1;
        fs->sb.checksum = pfs2_crc32(&fs->sb, 0x80);

        uint8_t sb_block[PFS2_BLOCK_SIZE];
        memset(sb_block, 0, PFS2_BLOCK_SIZE);
        memcpy(sb_block, &fs->sb, sizeof(fs->sb));

        if (fs->dev->write_block(fs->dev, 0, sb_block) != 0) return PFS2_ERR_IO;
        if (fs->dev->flush(fs->dev) != 0) return PFS2_ERR_IO;
    }

    fs->is_mounted = 0;
    return PFS2_OK;
}

int pfs2_read_inode(pfs2_fs_t *fs, uint32_t inode_num, pfs2_inode_t *out_inode) {
    if (!fs || !fs->is_mounted || inode_num == 0 || inode_num >= fs->sb.total_inodes) {
        return PFS2_ERR_INVALID;
    }
    uint32_t block = fs->sb.inode_start_block + (inode_num / PFS2_INODES_PER_BLOCK);
    uint32_t offset = (inode_num % PFS2_INODES_PER_BLOCK) * PFS2_INODE_SIZE;

    uint8_t buf[PFS2_BLOCK_SIZE];
    if (fs->dev->read_block(fs->dev, block, buf) != 0) return PFS2_ERR_IO;

    pfs2_inode_t *ino = (pfs2_inode_t *)(buf + offset);
    if (ino->mode == 0) return PFS2_ERR_NOT_FOUND;

    if (ino->checksum != pfs2_crc32(ino, 0x7C)) {
        return PFS2_ERR_CORRUPT;
    }

    if (out_inode) memcpy(out_inode, ino, sizeof(pfs2_inode_t));
    return PFS2_OK;
}

static int write_inode(pfs2_fs_t *fs, uint32_t inode_num, const pfs2_inode_t *inode, uint8_t *out_block_data, uint32_t *out_block_num) {
    uint32_t block = fs->sb.inode_start_block + (inode_num / PFS2_INODES_PER_BLOCK);
    uint32_t offset = (inode_num % PFS2_INODES_PER_BLOCK) * PFS2_INODE_SIZE;

    if (fs->dev->read_block(fs->dev, block, out_block_data) != 0) return PFS2_ERR_IO;
    memcpy(out_block_data + offset, inode, sizeof(pfs2_inode_t));
    *out_block_num = block;
    return PFS2_OK;
}

int pfs2_lookup(pfs2_fs_t *fs, const char *name, uint32_t *out_inode) {
    if (!fs || !fs->is_mounted || !name) return PFS2_ERR_INVALID;

    pfs2_inode_t root_ino;
    int err = pfs2_read_inode(fs, PFS2_ROOT_INODE, &root_ino);
    if (err != PFS2_OK) return err;

    uint8_t dir_buf[PFS2_BLOCK_SIZE];
    uint32_t dir_block = root_ino.direct_blocks[0];
    if (fs->dev->read_block(fs->dev, dir_block, dir_buf) != 0) return PFS2_ERR_IO;

    size_t name_len = strlen(name);
    for (uint32_t offset = 0; offset < PFS2_BLOCK_SIZE; offset += 32) {
        pfs2_dir_entry_t *de = (pfs2_dir_entry_t *)(dir_buf + offset);
        if (de->inode != 0 && de->name_len == name_len) {
            if (strncmp(de->name, name, name_len) == 0) {
                if (out_inode) *out_inode = de->inode;
                return PFS2_OK;
            }
        }
    }
    return PFS2_ERR_NOT_FOUND;
}

int pfs2_create_file(pfs2_fs_t *fs, const char *name, uint16_t mode, uint32_t *out_inode) {
    if (!fs || !fs->is_mounted || !name) return PFS2_ERR_INVALID;
    if (fs->read_only) return PFS2_ERR_ROFS;
    size_t name_len = strlen(name);
    if (name_len == 0 || name_len >= 24) return PFS2_ERR_INVALID;

    // Check if already exists
    uint32_t existing = 0;
    if (pfs2_lookup(fs, name, &existing) == PFS2_OK) {
        return PFS2_ERR_EXISTS;
    }

    // Find free inode
    uint32_t free_ino_num = 0;
    for (uint32_t i = 2; i < fs->sb.total_inodes; i++) {
        pfs2_inode_t check_ino;
        if (pfs2_read_inode(fs, i, &check_ino) == PFS2_ERR_NOT_FOUND) {
            free_ino_num = i;
            break;
        }
    }
    if (free_ino_num == 0) return PFS2_ERR_NO_INODE;

    // Read root dir data block
    pfs2_inode_t root_ino;
    pfs2_read_inode(fs, PFS2_ROOT_INODE, &root_ino);

    uint8_t dir_buf[PFS2_BLOCK_SIZE];
    uint32_t dir_block = root_ino.direct_blocks[0];
    if (fs->dev->read_block(fs->dev, dir_block, dir_buf) != 0) return PFS2_ERR_IO;

    // Find free slot in directory
    int slot_found = -1;
    for (uint32_t offset = 0; offset < PFS2_BLOCK_SIZE; offset += 32) {
        pfs2_dir_entry_t *de = (pfs2_dir_entry_t *)(dir_buf + offset);
        if (de->inode == 0) {
            slot_found = offset;
            break;
        }
    }
    if (slot_found < 0) return PFS2_ERR_NO_SPACE;

    // Populate new directory entry
    pfs2_dir_entry_t *new_de = (pfs2_dir_entry_t *)(dir_buf + slot_found);
    new_de->inode = free_ino_num;
    new_de->rec_len = 32;
    new_de->name_len = (uint8_t)name_len;
    new_de->file_type = 1; // File
    memset(new_de->name, 0, sizeof(new_de->name));
    strncpy(new_de->name, name, sizeof(new_de->name) - 1);

    // Populate new Inode
    pfs2_inode_t new_ino;
    memset(&new_ino, 0, sizeof(new_ino));
    new_ino.mode = PFS2_MODE_FILE | (mode & 0xFFF);
    new_ino.links_count = 1;
    new_ino.size_bytes = 0;
    new_ino.checksum = pfs2_crc32(&new_ino, 0x7C);

    // Inode block
    journal_slot_t slots[3];
    uint32_t ino_block_num = 0;
    write_inode(fs, free_ino_num, &new_ino, slots[0].data, &ino_block_num);
    slots[0].target_block = ino_block_num;

    // Directory block
    slots[1].target_block = dir_block;
    memcpy(slots[1].data, dir_buf, PFS2_BLOCK_SIZE);

    // Superblock
    fs->sb.free_inodes--;
    fs->sb.checksum = pfs2_crc32(&fs->sb, 0x80);
    slots[2].target_block = 0;
    memset(slots[2].data, 0, PFS2_BLOCK_SIZE);
    memcpy(slots[2].data, &fs->sb, sizeof(fs->sb));

    // Commit transaction via redo journal!
    int res = journal_commit_transaction(fs, slots, 3);
    if (res != PFS2_OK) return res;

    if (out_inode) *out_inode = free_ino_num;
    return PFS2_OK;
}

int pfs2_read_file(pfs2_fs_t *fs, uint32_t inode_num, void *buf, size_t count, uint64_t offset, size_t *out_bytes_read) {
    if (!fs || !fs->is_mounted || !buf) return PFS2_ERR_INVALID;

    pfs2_inode_t ino;
    int err = pfs2_read_inode(fs, inode_num, &ino);
    if (err != PFS2_OK) return err;

    if (!(ino.mode & PFS2_MODE_FILE)) return PFS2_ERR_INVALID;

    if (offset >= ino.size_bytes) {
        if (out_bytes_read) *out_bytes_read = 0;
        return PFS2_OK;
    }

    size_t to_read = count;
    if (offset + to_read > ino.size_bytes) {
        to_read = (size_t)(ino.size_bytes - offset);
    }

    size_t bytes_done = 0;
    uint8_t block_buf[PFS2_BLOCK_SIZE];

    while (bytes_done < to_read) {
        uint64_t cur_pos = offset + bytes_done;
        uint32_t block_idx = (uint32_t)(cur_pos / PFS2_BLOCK_SIZE);
        uint32_t block_offset = (uint32_t)(cur_pos % PFS2_BLOCK_SIZE);
        size_t chunk = PFS2_BLOCK_SIZE - block_offset;
        if (chunk > to_read - bytes_done) chunk = to_read - bytes_done;

        uint32_t phys_block = 0;
        if (block_idx < 12) {
            phys_block = ino.direct_blocks[block_idx];
        } else if (block_idx < 12 + 1024) {
            uint32_t ind_block = ino.indirect_block;
            if (ind_block == 0) {
                phys_block = 0;
            } else {
                uint32_t ind_ptrs[1024];
                if (fs->dev->read_block(fs->dev, ind_block, ind_ptrs) != 0) return PFS2_ERR_IO;
                phys_block = ind_ptrs[block_idx - 12];
            }
        } else {
            return PFS2_ERR_TOO_BIG;
        }

        if (phys_block == 0) {
            // Sparse block hole: zeroes
            memset((uint8_t *)buf + bytes_done, 0, chunk);
        } else {
            if (fs->dev->read_block(fs->dev, phys_block, block_buf) != 0) {
                return PFS2_ERR_IO;
            }
            memcpy((uint8_t *)buf + bytes_done, block_buf + block_offset, chunk);
        }

        bytes_done += chunk;
    }

    if (out_bytes_read) *out_bytes_read = bytes_done;
    return PFS2_OK;
}

int pfs2_write_file(pfs2_fs_t *fs, uint32_t inode_num, const void *buf, size_t count, uint64_t offset, size_t *out_bytes_written) {
    if (!fs || !fs->is_mounted || !buf) return PFS2_ERR_INVALID;
    if (fs->read_only) return PFS2_ERR_ROFS;
    if (count == 0) {
        if (out_bytes_written) *out_bytes_written = 0;
        return PFS2_OK;
    }

    pfs2_inode_t ino;
    int err = pfs2_read_inode(fs, inode_num, &ino);
    if (err != PFS2_OK) return err;
    if (!(ino.mode & PFS2_MODE_FILE)) return PFS2_ERR_INVALID;

    size_t bytes_written = 0;
    uint8_t block_buf[PFS2_BLOCK_SIZE];
    uint8_t ind_buf[PFS2_BLOCK_SIZE];
    int bitmap_modified = 0;
    int ind_modified = 0;

    while (bytes_written < count) {
        uint64_t cur_pos = offset + bytes_written;
        uint32_t block_idx = (uint32_t)(cur_pos / PFS2_BLOCK_SIZE);
        uint32_t block_offset = (uint32_t)(cur_pos % PFS2_BLOCK_SIZE);
        size_t chunk = PFS2_BLOCK_SIZE - block_offset;
        if (chunk > count - bytes_written) chunk = count - bytes_written;

        uint32_t phys_block = 0;
        if (block_idx < 12) {
            phys_block = ino.direct_blocks[block_idx];
            if (phys_block == 0) {
                uint32_t new_blk = 0;
                err = alloc_block(fs, &new_blk);
                if (err != PFS2_OK) return err;
                phys_block = new_blk;
                ino.direct_blocks[block_idx] = phys_block;
                bitmap_modified = 1;
                memset(block_buf, 0, PFS2_BLOCK_SIZE);
            } else if (block_offset != 0 || chunk < PFS2_BLOCK_SIZE) {
                fs->dev->read_block(fs->dev, phys_block, block_buf);
            }
        } else if (block_idx < 12 + 1024) {
            if (ino.indirect_block == 0) {
                uint32_t new_ind_blk = 0;
                err = alloc_block(fs, &new_ind_blk);
                if (err != PFS2_OK) return err;
                ino.indirect_block = new_ind_blk;
                bitmap_modified = 1;
                memset(ind_buf, 0, PFS2_BLOCK_SIZE);
                ind_modified = 1;
            } else if (!ind_modified) {
                fs->dev->read_block(fs->dev, ino.indirect_block, ind_buf);
            }

            uint32_t *ind_ptrs = (uint32_t *)ind_buf;
            phys_block = ind_ptrs[block_idx - 12];
            if (phys_block == 0) {
                uint32_t new_data_blk = 0;
                err = alloc_block(fs, &new_data_blk);
                if (err != PFS2_OK) return err;
                phys_block = new_data_blk;
                ind_ptrs[block_idx - 12] = phys_block;
                bitmap_modified = 1;
                ind_modified = 1;
                memset(block_buf, 0, PFS2_BLOCK_SIZE);
            } else if (block_offset != 0 || chunk < PFS2_BLOCK_SIZE) {
                fs->dev->read_block(fs->dev, phys_block, block_buf);
            }
        } else {
            return PFS2_ERR_TOO_BIG;
        }

        memcpy(block_buf + block_offset, (const uint8_t *)buf + bytes_written, chunk);

        // Write data block directly
        if (fs->dev->write_block(fs->dev, phys_block, block_buf) != 0) {
            return PFS2_ERR_IO;
        }

        bytes_written += chunk;
        if (cur_pos + chunk > ino.size_bytes) {
            ino.size_bytes = cur_pos + chunk;
        }
    }

    // Update Inode checksum
    ino.checksum = pfs2_crc32(&ino, 0x7C);

    // Journal commit for metadata: Inode block, Bitmap block, Indirect block (if modified), Superblock
    journal_slot_t slots[PFS2_MAX_AFTER_IMAGES];
    uint32_t s_count = 0;

    // Slot 0: Inode block
    uint32_t ino_block = 0;
    write_inode(fs, inode_num, &ino, slots[s_count].data, &ino_block);
    slots[s_count].target_block = ino_block;
    s_count++;

    // Slot 1: Bitmap block
    if (bitmap_modified) {
        slots[s_count].target_block = fs->sb.bitmap_start_block;
        memcpy(slots[s_count].data, fs->bitmap, PFS2_BLOCK_SIZE);
        s_count++;
    }

    // Slot 2: Indirect block
    if (ind_modified) {
        slots[s_count].target_block = ino.indirect_block;
        memcpy(slots[s_count].data, ind_buf, PFS2_BLOCK_SIZE);
        s_count++;
    }

    // Slot 3: Superblock
    fs->sb.checksum = pfs2_crc32(&fs->sb, 0x80);
    slots[s_count].target_block = 0;
    memset(slots[s_count].data, 0, PFS2_BLOCK_SIZE);
    memcpy(slots[s_count].data, &fs->sb, sizeof(fs->sb));
    s_count++;

    int j_res = journal_commit_transaction(fs, slots, s_count);
    if (j_res != PFS2_OK) return j_res;

    if (out_bytes_written) *out_bytes_written = bytes_written;
    return PFS2_OK;
}

int pfs2_truncate_file(pfs2_fs_t *fs, uint32_t inode_num, uint64_t new_size) {
    if (!fs || !fs->is_mounted) return PFS2_ERR_INVALID;
    if (fs->read_only) return PFS2_ERR_ROFS;

    pfs2_inode_t ino;
    int err = pfs2_read_inode(fs, inode_num, &ino);
    if (err != PFS2_OK) return err;
    if (!(ino.mode & PFS2_MODE_FILE)) return PFS2_ERR_INVALID;

    uint32_t current_blocks = (uint32_t)((ino.size_bytes + PFS2_BLOCK_SIZE - 1) / PFS2_BLOCK_SIZE);
    uint32_t needed_blocks = (uint32_t)((new_size + PFS2_BLOCK_SIZE - 1) / PFS2_BLOCK_SIZE);

    int bitmap_modified = 0;
    int ind_modified = 0;
    uint8_t ind_buf[PFS2_BLOCK_SIZE];
    uint32_t *ind_ptrs = NULL;

    if (ino.indirect_block != 0) {
        fs->dev->read_block(fs->dev, ino.indirect_block, ind_buf);
        ind_ptrs = (uint32_t *)ind_buf;
    }

    if (needed_blocks < current_blocks) {
        for (uint32_t b = needed_blocks; b < current_blocks; b++) {
            if (b < 12) {
                if (ino.direct_blocks[b] != 0) {
                    free_block(fs, ino.direct_blocks[b]);
                    ino.direct_blocks[b] = 0;
                    bitmap_modified = 1;
                }
            } else if (b < 12 + 1024 && ind_ptrs) {
                if (ind_ptrs[b - 12] != 0) {
                    free_block(fs, ind_ptrs[b - 12]);
                    ind_ptrs[b - 12] = 0;
                    bitmap_modified = 1;
                    ind_modified = 1;
                }
            }
        }
    }

    ino.size_bytes = new_size;
    ino.checksum = pfs2_crc32(&ino, 0x7C);

    journal_slot_t slots[4];
    uint32_t s_count = 0;

    uint32_t ino_block = 0;
    write_inode(fs, inode_num, &ino, slots[s_count].data, &ino_block);
    slots[s_count].target_block = ino_block;
    s_count++;

    if (bitmap_modified) {
        slots[s_count].target_block = fs->sb.bitmap_start_block;
        memcpy(slots[s_count].data, fs->bitmap, PFS2_BLOCK_SIZE);
        s_count++;
    }

    if (ind_modified && ino.indirect_block != 0) {
        slots[s_count].target_block = ino.indirect_block;
        memcpy(slots[s_count].data, ind_buf, PFS2_BLOCK_SIZE);
        s_count++;
    }

    fs->sb.checksum = pfs2_crc32(&fs->sb, 0x80);
    slots[s_count].target_block = 0;
    memset(slots[s_count].data, 0, PFS2_BLOCK_SIZE);
    memcpy(slots[s_count].data, &fs->sb, sizeof(fs->sb));
    s_count++;

    return journal_commit_transaction(fs, slots, s_count);
}

int pfs2_sync(pfs2_fs_t *fs) {
    if (!fs || !fs->is_mounted || !fs->dev) return PFS2_ERR_INVALID;
    if (fs->dev->flush) {
        return (fs->dev->flush(fs->dev) == 0) ? PFS2_OK : PFS2_ERR_IO;
    }
    return PFS2_OK;
}
