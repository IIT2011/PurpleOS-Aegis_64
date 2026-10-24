#include "gpt.h"
#include "purplefs_v2.h"
#include "string.h"

/* PurpleFS v2 Partition Type GUID: 4F535046-5332-0001-8000-00A0C911D504 */
const gpt_guid_t GPT_GUID_PURPLEFS_V2 = {
    .data1 = 0x4F535046,
    .data2 = 0x5332,
    .data3 = 0x0001,
    .data4 = { 0x80, 0x00, 0x00, 0xA0, 0xC9, 0x11, 0xD5, 0x04 }
};

/* EFI System Partition (ESP) GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B */
const gpt_guid_t GPT_GUID_ESP = {
    .data1 = 0xC12A7328,
    .data2 = 0xF81F,
    .data3 = 0x11D2,
    .data4 = { 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B }
};

/* Unused Partition GUID (All zeroes) */
const gpt_guid_t GPT_GUID_UNUSED = {
    .data1 = 0x00000000,
    .data2 = 0x0000,
    .data3 = 0x0000,
    .data4 = { 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 }
};

int gpt_guid_equal(const gpt_guid_t *a, const gpt_guid_t *b) {
    if (!a || !b) return 0;
    return memcmp(a, b, sizeof(gpt_guid_t)) == 0;
}

static void byte_to_hex(uint8_t byte, char *out) {
    static const char hex[] = "0123456789ABCDEF";
    out[0] = hex[(byte >> 4) & 0x0F];
    out[1] = hex[byte & 0x0F];
}

void gpt_guid_to_str(const gpt_guid_t *guid, char *out_str) {
    if (!guid || !out_str) return;

    // Data1 (8 hex digits, big-endian representation of 32-bit int)
    byte_to_hex((uint8_t)((guid->data1 >> 24) & 0xFF), &out_str[0]);
    byte_to_hex((uint8_t)((guid->data1 >> 16) & 0xFF), &out_str[2]);
    byte_to_hex((uint8_t)((guid->data1 >> 8) & 0xFF),  &out_str[4]);
    byte_to_hex((uint8_t)(guid->data1 & 0xFF),         &out_str[6]);
    out_str[8] = '-';

    // Data2 (4 hex digits)
    byte_to_hex((uint8_t)((guid->data2 >> 8) & 0xFF),  &out_str[9]);
    byte_to_hex((uint8_t)(guid->data2 & 0xFF),         &out_str[11]);
    out_str[13] = '-';

    // Data3 (4 hex digits)
    byte_to_hex((uint8_t)((guid->data3 >> 8) & 0xFF),  &out_str[14]);
    byte_to_hex((uint8_t)(guid->data3 & 0xFF),         &out_str[16]);
    out_str[18] = '-';

    // Data4[0..1]
    byte_to_hex(guid->data4[0], &out_str[19]);
    byte_to_hex(guid->data4[1], &out_str[21]);
    out_str[23] = '-';

    // Data4[2..7]
    for (int i = 0; i < 6; i++) {
        byte_to_hex(guid->data4[2 + i], &out_str[24 + i * 2]);
    }
    out_str[36] = '\0';
}

int gpt_parse_block_dev(block_device_t *bdev, gpt_info_t *out_info) {
    if (!bdev || !out_info) return GPT_ERR_IO;

    memset(out_info, 0, sizeof(*out_info));
    out_info->purplefs_part_index = -1;

    // Must have at least 34 sectors for Protective MBR + Primary GPT
    if (bdev->total_blocks < 34) {
        return GPT_ERR_BAD_BOUNDS;
    }

    // 1. Read Protective MBR (LBA 0)
    uint8_t mbr_buf[512];
    if (block_dev_read(bdev, 0, 1, mbr_buf) != BLOCK_DEV_OK) {
        return GPT_ERR_IO;
    }

    mbr_sector_t *mbr = (mbr_sector_t *)mbr_buf;
    if (mbr->signature != 0xAA55) {
        return GPT_ERR_BAD_MBR;
    }

    int has_protective_partition = 0;
    for (int i = 0; i < 4; i++) {
        if (mbr->partitions[i].os_type == 0xEE) {
            has_protective_partition = 1;
            break;
        }
    }

    if (!has_protective_partition) {
        return GPT_ERR_BAD_MBR;
    }
    out_info->valid_protective_mbr = 1;

    // 2. Read Primary GPT Header (LBA 1)
    uint8_t gpt_hdr_buf[512];
    if (block_dev_read(bdev, 1, 1, gpt_hdr_buf) != BLOCK_DEV_OK) {
        return GPT_ERR_IO;
    }

    gpt_header_t *hdr = (gpt_header_t *)gpt_hdr_buf;
    if (hdr->signature != GPT_SIGNATURE) {
        return GPT_ERR_BAD_SIGNATURE;
    }

    if (hdr->header_size < 92 || hdr->header_size > 512) {
        return GPT_ERR_BAD_CRC;
    }

    if (hdr->my_lba != 1) {
        return GPT_ERR_BAD_BOUNDS;
    }

    if (hdr->first_usable_lba >= hdr->last_usable_lba ||
        hdr->last_usable_lba >= bdev->total_blocks) {
        return GPT_ERR_BAD_BOUNDS;
    }

    // Verify GPT Header CRC32
    uint32_t original_crc = hdr->header_crc32;
    hdr->header_crc32 = 0;
    uint32_t calc_crc = pfs2_crc32(hdr, hdr->header_size);
    hdr->header_crc32 = original_crc;

    if (calc_crc != original_crc) {
        return GPT_ERR_BAD_CRC;
    }

    out_info->valid_header = 1;
    out_info->current_lba = hdr->my_lba;
    out_info->backup_lba = hdr->alternate_lba;
    out_info->first_usable_lba = hdr->first_usable_lba;
    out_info->last_usable_lba = hdr->last_usable_lba;

    // 3. Read and Validate Partition Entry Array
    uint32_t num_entries = hdr->num_partition_entries;
    uint32_t entry_size = hdr->size_of_partition_entry;

    if (num_entries == 0 || num_entries > 1024) {
        return GPT_ERR_BAD_BOUNDS;
    }
    if (entry_size < 128 || entry_size > 512) {
        return GPT_ERR_BAD_BOUNDS;
    }

    uint32_t total_array_bytes = num_entries * entry_size;
    uint32_t sectors_needed = (total_array_bytes + 511) / 512;

    if (hdr->partition_entry_lba + sectors_needed > hdr->first_usable_lba) {
        return GPT_ERR_BAD_BOUNDS;
    }

    // Allocate buffer on stack for reading entries in 512-byte blocks
    // To stay safe without huge stack allocations, compute running CRC32
    uint8_t sector_buf[512];
    uint32_t entries_crc = 0xFFFFFFFF;
    uint32_t bytes_remaining = total_array_bytes;
    uint64_t entry_lba = hdr->partition_entry_lba;

    // First pass: compute CRC of entire partition entry array
    for (uint32_t s = 0; s < sectors_needed; s++) {
        if (block_dev_read(bdev, entry_lba + s, 1, sector_buf) != BLOCK_DEV_OK) {
            return GPT_ERR_IO;
        }

        uint32_t chunk = (bytes_remaining > 512) ? 512 : bytes_remaining;
        for (uint32_t b = 0; b < chunk; b++) {
            entries_crc ^= sector_buf[b];
            for (int j = 0; j < 8; j++) {
                entries_crc = (entries_crc >> 1) ^ ((entries_crc & 1) ? 0xEDB88320 : 0);
            }
        }
        bytes_remaining -= chunk;
    }
    entries_crc = ~entries_crc;

    if (entries_crc != hdr->partition_entry_array_crc32) {
        return GPT_ERR_BAD_CRC;
    }
    out_info->valid_entries = 1;

    // Second pass: parse and validate individual partition entries
    uint32_t parsed_count = 0;
    uint32_t entry_index = 0;

    for (uint32_t s = 0; s < sectors_needed && parsed_count < GPT_MAX_PARTITIONS; s++) {
        if (block_dev_read(bdev, entry_lba + s, 1, sector_buf) != BLOCK_DEV_OK) {
            return GPT_ERR_IO;
        }

        uint32_t entries_in_sector = 512 / entry_size;
        for (uint32_t e = 0; e < entries_in_sector && entry_index < num_entries; e++, entry_index++) {
            gpt_entry_t *entry = (gpt_entry_t *)(sector_buf + e * entry_size);

            if (gpt_guid_equal(&entry->partition_type_guid, &GPT_GUID_UNUSED)) {
                continue; // Unused partition entry
            }

            // Validate partition boundaries
            if (entry->starting_lba > entry->ending_lba) {
                return GPT_ERR_BAD_BOUNDS;
            }
            if (entry->starting_lba < hdr->first_usable_lba ||
                entry->ending_lba > hdr->last_usable_lba ||
                entry->ending_lba >= bdev->total_blocks) {
                return GPT_ERR_BAD_BOUNDS;
            }

            uint64_t sector_count = entry->ending_lba - entry->starting_lba + 1;
            if (sector_count == 0) {
                return GPT_ERR_BAD_BOUNDS;
            }

            // Check for overlap with already parsed partitions
            for (uint32_t p = 0; p < parsed_count; p++) {
                const gpt_partition_t *prev = &out_info->partitions[p];
                if (!(entry->ending_lba < prev->start_lba || entry->starting_lba > prev->end_lba)) {
                    return GPT_ERR_OVERLAP; // Partitions overlap!
                }
            }

            // Record partition
            gpt_partition_t *part = &out_info->partitions[parsed_count];
            part->index = entry_index;
            memcpy(&part->type_guid, &entry->partition_type_guid, sizeof(gpt_guid_t));
            memcpy(&part->unique_guid, &entry->unique_partition_guid, sizeof(gpt_guid_t));
            part->start_lba = entry->starting_lba;
            part->end_lba = entry->ending_lba;
            part->sector_count = sector_count;
            part->attributes = entry->attributes;

            // Convert UTF-16LE partition name to ASCII
            for (int c = 0; c < 36; c++) {
                uint16_t u = entry->partition_name[c];
                part->name[c] = (u < 128 && u > 0) ? (char)u : (u == 0 ? '\0' : '?');
            }
            part->name[36] = '\0';

            // Check for PurpleFS v2 partition type GUID
            if (gpt_guid_equal(&part->type_guid, &GPT_GUID_PURPLEFS_V2)) {
                part->is_purplefs = 1;
                if (out_info->purplefs_part_index < 0) {
                    out_info->purplefs_part_index = (int)parsed_count;
                }
            }

            parsed_count++;
            if (parsed_count >= GPT_MAX_PARTITIONS) {
                break;
            }
        }
    }

    out_info->partition_count = parsed_count;
    return GPT_OK;
}

int gpt_parse(ata_device_t *dev, gpt_info_t *out_info) {
    if (!dev || !dev->is_present || !out_info) return GPT_ERR_IO;

    block_device_t bdev;
    if (ata_bind_block_dev(dev, &bdev, "ata_gpt") != 0) {
        return GPT_ERR_IO;
    }

    return gpt_parse_block_dev(&bdev, out_info);
}
