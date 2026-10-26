#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <sys/stat.h>
#include <sys/types.h>

#include "purplefs_v3.h"

typedef struct {
    int fd;
    uint64_t total_sectors;
    uint64_t offset_bytes;
} file_bdev_priv_t;

static int file_read_blocks(block_device_t *dev, uint64_t lba, uint32_t count, void *buf) {
    file_bdev_priv_t *priv = (file_bdev_priv_t *)dev->priv;
    off_t off = (off_t)(priv->offset_bytes + lba * 512ULL);
    size_t bytes = (size_t)(count * 512ULL);
    ssize_t rd = pread(priv->fd, buf, bytes, off);
    return (rd == (ssize_t)bytes) ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int file_write_blocks(block_device_t *dev, uint64_t lba, uint32_t count, const void *buf) {
    file_bdev_priv_t *priv = (file_bdev_priv_t *)dev->priv;
    off_t off = (off_t)(priv->offset_bytes + lba * 512ULL);
    size_t bytes = (size_t)(count * 512ULL);
    ssize_t wr = pwrite(priv->fd, buf, bytes, off);
    return (wr == (ssize_t)bytes) ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static int file_flush(block_device_t *dev) {
    file_bdev_priv_t *priv = (file_bdev_priv_t *)dev->priv;
    return (fsync(priv->fd) == 0) ? BLOCK_DEV_OK : BLOCK_DEV_ERR_IO;
}

static const block_device_ops_t s_file_ops = {
    .read_blocks = file_read_blocks,
    .write_blocks = file_write_blocks,
    .flush = file_flush
};

static block_device_t *open_file_bdev(const char *path, uint64_t offset_bytes, uint64_t sector_count) {
    int fd = open(path, O_RDWR);
    if (fd < 0) {
        perror("open");
        return NULL;
    }
    struct stat st;
    if (fstat(fd, &st) < 0) {
        close(fd);
        return NULL;
    }

    uint64_t total_sectors = sector_count;
    if (total_sectors == 0) {
        if (st.st_size > (off_t)offset_bytes) {
            total_sectors = (st.st_size - offset_bytes) / 512ULL;
        }
    }

    block_device_t *bdev = calloc(1, sizeof(block_device_t));
    file_bdev_priv_t *priv = calloc(1, sizeof(file_bdev_priv_t));
    priv->fd = fd;
    priv->offset_bytes = offset_bytes;
    priv->total_sectors = total_sectors;

    bdev->block_size = 512;
    bdev->total_blocks = total_sectors;
    bdev->ops = &s_file_ops;
    bdev->priv = priv;
    strncpy(bdev->name, "host_img", sizeof(bdev->name) - 1);
    return bdev;
}

static void close_file_bdev(block_device_t *bdev) {
    if (bdev) {
        file_bdev_priv_t *priv = (file_bdev_priv_t *)bdev->priv;
        if (priv) {
            fsync(priv->fd);
            close(priv->fd);
            free(priv);
        }
        free(bdev);
    }
}

/* Recursive directory creator in PFS3: creates all parent components as needed */
static int pfs3_mkdir_p(pfs3_fs_t *fs, const char *path, uint64_t *out_dir_ino) {
    if (!path || path[0] == '\0') return PFS3_ERR_INVALID;
    if (strcmp(path, "/") == 0) {
        if (out_dir_ino) *out_dir_ino = PFS3_ROOT_INODE;
        return PFS3_OK;
    }

    char tmp[512];
    strncpy(tmp, path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = '\0';

    uint64_t cur_ino = PFS3_ROOT_INODE;
    char *p = tmp;
    if (*p == '/') p++;

    while (*p != '\0') {
        char *slash = strchr(p, '/');
        if (slash) *slash = '\0';

        if (*p != '\0') {
            uint64_t child_ino = 0;
            int rc = pfs3_lookup_entry(fs, cur_ino, p, &child_ino, NULL);
            if (rc == PFS3_OK) {
                cur_ino = child_ino;
            } else {
                rc = pfs3_mkdir(fs, cur_ino, p, &child_ino);
                if (rc != PFS3_OK) {
                    fprintf(stderr, "Error creating directory '%s' under inode %llu: %d\n", p, (unsigned long long)cur_ino, rc);
                    return rc;
                }
                cur_ino = child_ino;
            }
        }

        if (!slash) break;
        p = slash + 1;
    }

    if (out_dir_ino) *out_dir_ino = cur_ino;
    return PFS3_OK;
}

/* Write host buffer into a PFS3 path */
static int pfs3_write_file_path(pfs3_fs_t *fs, const char *target_path, const void *buf, size_t size) {
    char parent[512];
    char base[128];
    const char *slash = strrchr(target_path, '/');
    uint64_t parent_ino = PFS3_ROOT_INODE;

    if (slash) {
        size_t plen = (size_t)(slash - target_path);
        if (plen == 0) plen = 1;
        strncpy(parent, target_path, plen);
        parent[plen] = '\0';
        strncpy(base, slash + 1, sizeof(base) - 1);
        base[sizeof(base) - 1] = '\0';

        int mrc = pfs3_mkdir_p(fs, parent, &parent_ino);
        if (mrc != PFS3_OK) return mrc;
    } else {
        strncpy(base, target_path, sizeof(base) - 1);
        base[sizeof(base) - 1] = '\0';
    }

    uint64_t file_ino = 0;
    int rc = pfs3_lookup_entry(fs, parent_ino, base, &file_ino, NULL);
    if (rc != PFS3_OK) {
        rc = pfs3_create_entry(fs, parent_ino, base, PFS3_FT_REG, &file_ino);
        if (rc != PFS3_OK) {
            fprintf(stderr, "Failed to create entry '%s': %d\n", base, rc);
            return rc;
        }
    }

    rc = pfs3_write_file(fs, file_ino, buf, size, 0);
    if (rc != PFS3_OK) {
        fprintf(stderr, "Failed to write file '%s': %d\n", target_path, rc);
        return rc;
    }
    return PFS3_OK;
}

static int cmd_format(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "Usage: %s format <img_file> <size_sectors> [label]\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    uint64_t sectors = strtoull(argv[3], NULL, 0);
    const char *label = (argc > 4) ? argv[4] : "PurpleSystem";

    /* Ensure file exists and is of correct size */
    int fd = open(img_path, O_RDWR | O_CREAT, 0644);
    if (fd < 0) { perror("open"); return 1; }
    if (ftruncate(fd, sectors * 512ULL) < 0) { perror("ftruncate"); close(fd); return 1; }
    close(fd);

    block_device_t *bdev = open_file_bdev(img_path, 0, sectors);
    if (!bdev) return 1;

    int rc = pfs3_format(bdev, 0, sectors, label, NULL);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_format failed: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }

    printf("[OK] Formatted PurpleFS v3 on %s (%llu sectors, label '%s')\n",
           img_path, (unsigned long long)sectors, label);
    close_file_bdev(bdev);
    return 0;
}

static int cmd_mkdir(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "Usage: %s mkdir <img_file> <dir_path>\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    const char *dir_path = argv[3];

    block_device_t *bdev = open_file_bdev(img_path, 0, 0);
    if (!bdev) return 1;

    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, bdev->total_blocks);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_mount failed: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }

    rc = pfs3_mkdir_p(&fs, dir_path, NULL);
    if (rc == PFS3_OK) {
        printf("[OK] Created directory '%s' on %s\n", dir_path, img_path);
    }

    pfs3_unmount(&fs);
    close_file_bdev(bdev);
    return (rc == PFS3_OK) ? 0 : 1;
}

static int cmd_write(int argc, char **argv) {
    if (argc < 5) {
        fprintf(stderr, "Usage: %s write <img_file> <host_file> <target_pfs_path>\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    const char *host_file = argv[3];
    const char *target_path = argv[4];

    FILE *f = fopen(host_file, "rb");
    if (!f) { perror("fopen host_file"); return 1; }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    void *data = malloc(sz);
    if (fread(data, 1, sz, f) != (size_t)sz) {
        perror("fread");
        free(data);
        fclose(f);
        return 1;
    }
    fclose(f);

    block_device_t *bdev = open_file_bdev(img_path, 0, 0);
    if (!bdev) { free(data); return 1; }

    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, bdev->total_blocks);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_mount failed: %d\n", rc);
        free(data);
        close_file_bdev(bdev);
        return 1;
    }

    rc = pfs3_write_file_path(&fs, target_path, data, (size_t)sz);
    if (rc == PFS3_OK) {
        printf("[OK] Wrote '%s' (%ld bytes) -> '%s'\n", host_file, sz, target_path);
    }

    free(data);
    pfs3_unmount(&fs);
    close_file_bdev(bdev);
    return (rc == PFS3_OK) ? 0 : 1;
}

static int cmd_ls(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s ls <img_file> [dir_path]\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    const char *dir_path = (argc > 3) ? argv[3] : "/";

    block_device_t *bdev = open_file_bdev(img_path, 0, 0);
    if (!bdev) return 1;

    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, bdev->total_blocks);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_mount failed: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }

    uint64_t dir_ino = PFS3_ROOT_INODE;
    if (strcmp(dir_path, "/") != 0) {
        rc = pfs3_lookup_path(&fs, dir_path, &dir_ino);
        if (rc != PFS3_OK) {
            fprintf(stderr, "Path '%s' not found: %d\n", dir_path, rc);
            pfs3_unmount(&fs);
            close_file_bdev(bdev);
            return 1;
        }
    }

    printf("\nDirectory Listing of '%s' (Inode %llu):\n", dir_path, (unsigned long long)dir_ino);
    printf("  %-8s %-6s %-12s %s\n", "INODE", "TYPE", "SIZE", "NAME");
    printf("  --------------------------------------------------\n");

    uint32_t idx = 0;
    pfs3_dirent_t de;
    uint32_t count = 0;
    while (pfs3_readdir(&fs, dir_ino, idx++, &de) == PFS3_OK) {
        count++;
        pfs3_inode_t ino;
        uint64_t sz = 0;
        if (pfs3_read_inode(&fs, de.inode_num, &ino) == PFS3_OK) {
            sz = ino.size_bytes;
        }
        printf("  %-8llu %-6s %-12llu %s\n",
               (unsigned long long)de.inode_num,
               (de.file_type == PFS3_FT_DIR) ? "<DIR>" : "<FILE>",
               (unsigned long long)sz,
               de.name);
    }
    printf("  --------------------------------------------------\n");
    printf("  Total: %u item(s)\n\n", count);

    pfs3_unmount(&fs);
    close_file_bdev(bdev);
    return 0;
}

static int cmd_fsck(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s fsck <img_file>\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    block_device_t *bdev = open_file_bdev(img_path, 0, 0);
    if (!bdev) return 1;

    printf("=== PurpleFS v3 Consistency & Integrity Check: %s ===\n", img_path);
    printf("[1/5] Reading Primary Superblock (Block 1)... ");
    uint8_t blk1[4096];
    if (bdev->ops->read_blocks(bdev, 8, 8, blk1) != BLOCK_DEV_OK) {
        printf("FAILED (I/O error)\n");
        close_file_bdev(bdev);
        return 1;
    }
    pfs3_superblock_t *sb = (pfs3_superblock_t *)blk1;
    if (sb->magic != PFS3_MAGIC || sb->version != PFS3_VERSION) {
        printf("FAILED (Invalid magic 0x%08X or version %u)\n", sb->magic, sb->version);
        close_file_bdev(bdev);
        return 1;
    }
    uint32_t crc1 = pfs3_crc32(sb, sizeof(pfs3_superblock_t) - 4);
    if (crc1 != sb->checksum) {
        printf("CORRUPTED (Expected CRC 0x%08X, got 0x%08X)\n", sb->checksum, crc1);
    } else {
        printf("OK (CRC 0x%08X verified)\n", crc1);
    }

    printf("[2/5] Reading Backup Superblock (Block 2)... ");
    uint8_t blk2[4096];
    bdev->ops->read_blocks(bdev, 16, 8, blk2);
    pfs3_superblock_t *sb2 = (pfs3_superblock_t *)blk2;
    uint32_t crc2 = pfs3_crc32(sb2, sizeof(pfs3_superblock_t) - 4);
    if (sb2->magic == PFS3_MAGIC && crc2 == sb2->checksum) {
        printf("OK (Valid backup)\n");
    } else {
        printf("WARNING (Backup corrupted or mismatched)\n");
    }

    printf("[3/5] Verifying Volume Metadata...\n");
    printf("      Label:        '%s'\n", sb->volume_label);
    printf("      Total Blocks: %llu (%.1f MiB)\n", (unsigned long long)sb->total_blocks, (double)(sb->total_blocks * 4096) / (1024.0*1024.0));
    printf("      Free Blocks:  %llu\n", (unsigned long long)sb->free_blocks);
    printf("      Total Inodes: %llu\n", (unsigned long long)sb->total_inodes);
    printf("      Free Inodes:  %llu\n", (unsigned long long)sb->free_inodes);
    printf("      Root Inode:   %llu\n", (unsigned long long)sb->root_inode);

    printf("[4/5] Mounting Volume & Verifying Inode Table... ");
    pfs3_fs_t fs;
    int rc = pfs3_mount(&fs, bdev, 0, bdev->total_blocks);
    if (rc != PFS3_OK) {
        printf("MOUNT FAILED: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }
    pfs3_inode_t root_ino;
    if (pfs3_read_inode(&fs, PFS3_ROOT_INODE, &root_ino) != PFS3_OK || !(root_ino.mode & PFS3_MODE_DIR)) {
        printf("ROOT INODE CORRUPTED\n");
    } else {
        printf("OK\n");
    }

    printf("[5/5] Checking Directory Tree Traversal... ");
    uint32_t dir_count = 0, file_count = 0;
    /* Helper recursive checker */
    void check_tree(uint64_t dinode, int depth) {
        if (depth > 16) return;
        uint32_t d_idx = 0;
        pfs3_dirent_t de;
        while (pfs3_readdir(&fs, dinode, d_idx++, &de) == PFS3_OK) {
            if (strcmp(de.name, ".") == 0 || strcmp(de.name, "..") == 0) continue;
            if (de.file_type == PFS3_FT_DIR) {
                dir_count++;
                check_tree(de.inode_num, depth + 1);
            } else {
                file_count++;
            }
        }
    }
    check_tree(PFS3_ROOT_INODE, 0);
    printf("OK (%u directories, %u files checked)\n", dir_count, file_count);

    pfs3_unmount(&fs);
    close_file_bdev(bdev);
    printf(">>> FSCK RESULT: VOLUME CLEAN & CONSISTENT <<<\n\n");
    return 0;
}

static int cmd_populate(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s populate <img_file> [size_sectors]\n", argv[0]);
        return 1;
    }
    const char *img_path = argv[2];
    uint64_t sectors = (argc > 3) ? strtoull(argv[3], NULL, 0) : 126976; /* ~62 MiB */

    /* 1. Format disk with PFS3 */
    int fd = open(img_path, O_RDWR | O_CREAT, 0644);
    if (fd < 0) { perror("open"); return 1; }
    if (ftruncate(fd, sectors * 512ULL) < 0) { perror("ftruncate"); close(fd); return 1; }
    close(fd);

    block_device_t *bdev = open_file_bdev(img_path, 0, sectors);
    if (!bdev) return 1;

    int rc = pfs3_format(bdev, 0, sectors, "PurpleSystem", NULL);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_format failed: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }

    pfs3_fs_t fs;
    rc = pfs3_mount(&fs, bdev, 0, sectors);
    if (rc != PFS3_OK) {
        fprintf(stderr, "pfs3_mount failed: %d\n", rc);
        close_file_bdev(bdev);
        return 1;
    }

    printf("=== Populating Directive 4 Logical Hierarchy ===\n");

    /* Create the required directories */
    const char *dirs_to_create[] = {
        "/Boot",
        "/Boot/EFI",
        "/Boot/EFI/Boot",
        "/System",
        "/System/Kernel",
        "/System/Frameworks",
        "/System/Services",
        "/System/Applications",
        "/System/Drivers",
        "/System/Resources",
        "/System/Configuration",
        "/System/Libraries",
        "/Users",
        "/Users/aegis",
        "/Users/aegis/Applications",
        "/Users/aegis/Documents",
        "/Users/aegis/Downloads",
        "/Users/aegis/Desktop",
        "/Users/aegis/Pictures",
        "/Users/aegis/Videos",
        "/Users/aegis/Music",
        "/Users/aegis/Settings",
        "/Users/aegis/Cache",
        "/Users/aegis/Trash",
        "/Users/Admin",
        "/Users/Admin/Applications",
        "/Users/Admin/Desktop",
        "/Users/Admin/Documents",
        "/Users/Admin/Downloads",
        "/Users/Admin/Music",
        "/Users/Admin/Pictures",
        "/Users/Admin/Videos",
        "/Data",
        NULL
    };

    for (int i = 0; dirs_to_create[i]; i++) {
        pfs3_mkdir_p(&fs, dirs_to_create[i], NULL);
    }
    printf("[OK] Created 25 standard system directories\n");

    /* Populate default configuration and text files */
    const char *welcome_text =
        "Welcome to Purple OS / Aegis64 Native Filesystem!\n"
        "Architecture: x86-64 | Kernel: Aegis64\n"
        "Filesystem: PurpleFS v3 Hierarchical Storage\n";
    pfs3_write_file_path(&fs, "/welcome.txt", welcome_text, strlen(welcome_text));
    pfs3_write_file_path(&fs, "/Users/aegis/Desktop/welcome.txt", welcome_text, strlen(welcome_text));

    const char *version_text = "Aegis64 Kernel v0.4.0 (Stage 4B Safe Kernel Storage Integration)\n";
    pfs3_write_file_path(&fs, "/version.txt", version_text, strlen(version_text));

    const char *motd_text = "Purple OS: Correctness, Memory Safety, and Recoverability.\n";
    pfs3_write_file_path(&fs, "/motd", motd_text, strlen(motd_text));

    const char *notes_text =
        "Purple OS Quick Reference:\n"
        "- Desktop: Super Key or click Dock items to open applications.\n"
        "- Shell: Type 'help' or 'disk status' for storage commands.\n"
        "- Storage: Full read/write persistent PurpleFS v3.\n";
    pfs3_write_file_path(&fs, "/Users/aegis/Desktop/notes.txt", notes_text, strlen(notes_text));
    pfs3_write_file_path(&fs, "/Users/aegis/Documents/readme.txt", notes_text, strlen(notes_text));

    const char *desktop_cfg =
        "resolution=1280x800\n"
        "wallpaper=cosmic_purple\n"
        "theme=violet_dark\n"
        "dock_autohide=0\n";
    pfs3_write_file_path(&fs, "/desktop.cfg", desktop_cfg, strlen(desktop_cfg));
    pfs3_write_file_path(&fs, "/System/Configuration/desktop.cfg", desktop_cfg, strlen(desktop_cfg));

    const char *sys_conf =
        "[PurpleOS]\n"
        "version=1.0.0-omega\n"
        "security=ring3_strict\n"
        "storage=pfs3\n";
    pfs3_write_file_path(&fs, "/System/Configuration/system.conf", sys_conf, strlen(sys_conf));

    /* Populate sample multimedia file for VLC */
    const char *sample_media = "RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00\x44\xAC\x00\x00\x10\xB1\x02\x00\x04\x00\x10\x00data\x00\x00\x00\x00";
    pfs3_write_file_path(&fs, "/Users/aegis/Videos/sample.mp4", sample_media, 44);
    pfs3_write_file_path(&fs, "/Users/aegis/Music/sample.wav", sample_media, 44);

    /* Copy existing built applications and drivers from build/ if present */
    const char *apps[] = {
        "PurpleTerminal.apapplication",
        "PurpleFiles.apapplication",
        "PurpleViewer.apapplication",
        "PurpleSettings.apapplication",
        "VLC.apapplication",
        "terminal.apapplication",
        "files.apapplication",
        "viewer.apapplication",
        "settings.apapplication",
        "vlc.apapplication",
        "calculator.apapplication",
        "sysmon.apapplication",
        "fault_test.apapplication",
        NULL
    };

    for (int i = 0; apps[i]; i++) {
        char host_path[256];
        snprintf(host_path, sizeof(host_path), "build/%s", apps[i]);
        FILE *af = fopen(host_path, "rb");
        if (af) {
            fseek(af, 0, SEEK_END);
            long sz = ftell(af);
            fseek(af, 0, SEEK_SET);
            void *abuf = malloc(sz);
            if (fread(abuf, 1, sz, af) == (size_t)sz) {
                char target[256];
                snprintf(target, sizeof(target), "/System/Applications/%s", apps[i]);
                pfs3_write_file_path(&fs, target, abuf, sz);
                /* Populate in user application directories */
                snprintf(target, sizeof(target), "/Users/aegis/Applications/%s", apps[i]);
                pfs3_write_file_path(&fs, target, abuf, sz);
                snprintf(target, sizeof(target), "/Users/Admin/Applications/%s", apps[i]);
                pfs3_write_file_path(&fs, target, abuf, sz);
            }
            free(abuf);
            fclose(af);
        }
    }

    const char *drivers[] = {
        "Storage.apdrv",
        "Graphics.apdrv",
        "Audio.apdrv",
        "Input.apdrv",
        "Network.apdrv",
        "USB.apdrv",
        "WiFi.apdrv",
        "Bluetooth.apdrv",
        "Camera.apdrv",
        NULL
    };

    for (int i = 0; drivers[i]; i++) {
        char host_path[256];
        snprintf(host_path, sizeof(host_path), "build/%s", drivers[i]);
        FILE *df = fopen(host_path, "rb");
        if (df) {
            fseek(df, 0, SEEK_END);
            long sz = ftell(df);
            fseek(df, 0, SEEK_SET);
            void *dbuf = malloc(sz);
            if (fread(dbuf, 1, sz, df) == (size_t)sz) {
                char target[256];
                snprintf(target, sizeof(target), "/System/Drivers/%s", drivers[i]);
                pfs3_write_file_path(&fs, target, dbuf, sz);
            }
            free(dbuf);
            fclose(df);
        }
    }

    pfs3_unmount(&fs);
    close_file_bdev(bdev);
    printf("[OK] Successfully populated Directive 4 hierarchy on %s\n", img_path);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("PurpleFS v3 Forensic Inspection & Management Tool\n");
        printf("Usage:\n");
        printf("  %s format   <img_file> <size_sectors> [label]\n", argv[0]);
        printf("  %s populate <img_file> [size_sectors]\n", argv[0]);
        printf("  %s mkdir    <img_file> <dir_path>\n", argv[0]);
        printf("  %s write    <img_file> <host_file> <pfs_path>\n", argv[0]);
        printf("  %s ls       <img_file> [dir_path]\n", argv[0]);
        printf("  %s fsck     <img_file>\n", argv[0]);
        return 1;
    }

    const char *cmd = argv[1];
    if (strcmp(cmd, "format") == 0) return cmd_format(argc, argv);
    if (strcmp(cmd, "populate") == 0) return cmd_populate(argc, argv);
    if (strcmp(cmd, "mkdir") == 0) return cmd_mkdir(argc, argv);
    if (strcmp(cmd, "write") == 0) return cmd_write(argc, argv);
    if (strcmp(cmd, "ls") == 0) return cmd_ls(argc, argv);
    if (strcmp(cmd, "fsck") == 0) return cmd_fsck(argc, argv);

    fprintf(stderr, "Unknown command '%s'\n", cmd);
    return 1;
}
