#include "host_purplefs_v2.h"
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>

/* Memory Block Device Backend */
typedef struct {
    uint8_t *data;
} mem_dev_priv_t;

static int mem_read_block(pfs2_block_dev_t *dev, uint32_t block_num, void *buf) {
    if (!dev || block_num >= dev->block_count || !buf) return -1;
    mem_dev_priv_t *priv = (mem_dev_priv_t *)dev->priv;
    memcpy(buf, priv->data + (size_t)block_num * PFS2_BLOCK_SIZE, PFS2_BLOCK_SIZE);
    return 0;
}

static int mem_write_block(pfs2_block_dev_t *dev, uint32_t block_num, const void *buf) {
    if (!dev || block_num >= dev->block_count || !buf) return -1;
    mem_dev_priv_t *priv = (mem_dev_priv_t *)dev->priv;
    memcpy(priv->data + (size_t)block_num * PFS2_BLOCK_SIZE, buf, PFS2_BLOCK_SIZE);
    return 0;
}

static int mem_flush(pfs2_block_dev_t *dev) {
    (void)dev;
    return 0;
}

pfs2_block_dev_t *pfs2_mem_dev_create(uint32_t block_count) {
    pfs2_block_dev_t *dev = (pfs2_block_dev_t *)calloc(1, sizeof(pfs2_block_dev_t));
    if (!dev) return NULL;
    mem_dev_priv_t *priv = (mem_dev_priv_t *)calloc(1, sizeof(mem_dev_priv_t));
    if (!priv) {
        free(dev);
        return NULL;
    }
    priv->data = (uint8_t *)calloc(block_count, PFS2_BLOCK_SIZE);
    if (!priv->data) {
        free(priv);
        free(dev);
        return NULL;
    }
    dev->block_count = block_count;
    dev->read_block = mem_read_block;
    dev->write_block = mem_write_block;
    dev->flush = mem_flush;
    dev->priv = priv;
    return dev;
}

void pfs2_mem_dev_destroy(pfs2_block_dev_t *dev) {
    if (!dev) return;
    mem_dev_priv_t *priv = (mem_dev_priv_t *)dev->priv;
    if (priv) {
        free(priv->data);
        free(priv);
    }
    free(dev);
}

uint8_t *pfs2_mem_dev_get_data(pfs2_block_dev_t *dev) {
    if (!dev) return NULL;
    mem_dev_priv_t *priv = (mem_dev_priv_t *)dev->priv;
    return priv ? priv->data : NULL;
}

/* File Block Device Backend */
typedef struct {
    int fd;
} file_dev_priv_t;

static int file_read_block(pfs2_block_dev_t *dev, uint32_t block_num, void *buf) {
    if (!dev || block_num >= dev->block_count || !buf) return -1;
    file_dev_priv_t *priv = (file_dev_priv_t *)dev->priv;
    off_t offset = (off_t)block_num * PFS2_BLOCK_SIZE;
    ssize_t ret = pread(priv->fd, buf, PFS2_BLOCK_SIZE, offset);
    return (ret == PFS2_BLOCK_SIZE) ? 0 : -1;
}

static int file_write_block(pfs2_block_dev_t *dev, uint32_t block_num, const void *buf) {
    if (!dev || block_num >= dev->block_count || !buf) return -1;
    file_dev_priv_t *priv = (file_dev_priv_t *)dev->priv;
    off_t offset = (off_t)block_num * PFS2_BLOCK_SIZE;
    ssize_t ret = pwrite(priv->fd, buf, PFS2_BLOCK_SIZE, offset);
    return (ret == PFS2_BLOCK_SIZE) ? 0 : -1;
}

static int file_flush(pfs2_block_dev_t *dev) {
    if (!dev) return -1;
    file_dev_priv_t *priv = (file_dev_priv_t *)dev->priv;
    return fdatasync(priv->fd);
}

pfs2_block_dev_t *pfs2_file_dev_create(const char *filepath, uint32_t block_count) {
    int fd = open(filepath, O_RDWR | O_CREAT, 0644);
    if (fd < 0) return NULL;

    off_t expected_size = (off_t)block_count * PFS2_BLOCK_SIZE;
    struct stat st;
    if (fstat(fd, &st) == 0) {
        if (st.st_size < expected_size) {
            if (ftruncate(fd, expected_size) != 0) {
                close(fd);
                return NULL;
            }
        }
    }

    pfs2_block_dev_t *dev = (pfs2_block_dev_t *)calloc(1, sizeof(pfs2_block_dev_t));
    file_dev_priv_t *priv = (file_dev_priv_t *)calloc(1, sizeof(file_dev_priv_t));
    if (!dev || !priv) {
        close(fd);
        free(dev);
        free(priv);
        return NULL;
    }
    priv->fd = fd;
    dev->block_count = block_count;
    dev->read_block = file_read_block;
    dev->write_block = file_write_block;
    dev->flush = file_flush;
    dev->priv = priv;
    return dev;
}

void pfs2_file_dev_destroy(pfs2_block_dev_t *dev) {
    if (!dev) return;
    file_dev_priv_t *priv = (file_dev_priv_t *)dev->priv;
    if (priv) {
        close(priv->fd);
        free(priv);
    }
    free(dev);
}

/* Fault-Injecting Block Device */
static int fault_read_block(pfs2_block_dev_t *dev, uint32_t block_num, void *buf) {
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)dev;
    if (fdev->fail_reads) return -1;
    return fdev->target->read_block(fdev->target, block_num, buf);
}

static int fault_write_block(pfs2_block_dev_t *dev, uint32_t block_num, const void *buf) {
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)dev;
    fdev->write_count++;
    if (fdev->fail_writes) return -1;
    if (fdev->fail_after_write_count > 0 && fdev->write_count >= fdev->fail_after_write_count) {
        return -1; // Injected crash / write failure
    }
    return fdev->target->write_block(fdev->target, block_num, buf);
}

static int fault_flush(pfs2_block_dev_t *dev) {
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)dev;
    fdev->flush_count++;
    if (fdev->fail_flushes) return -1;
    if (fdev->fail_after_flush_count > 0 && fdev->flush_count >= fdev->fail_after_flush_count) {
        return -1; // Injected flush failure
    }
    return fdev->target->flush(fdev->target);
}

pfs2_block_dev_t *pfs2_fault_dev_create(pfs2_block_dev_t *target) {
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)calloc(1, sizeof(pfs2_fault_dev_t));
    if (!fdev) return NULL;
    fdev->target = target;
    fdev->base.block_count = target->block_count;
    fdev->base.read_block = fault_read_block;
    fdev->base.write_block = fault_write_block;
    fdev->base.flush = fault_flush;
    fdev->base.priv = fdev;
    return &fdev->base;
}

void pfs2_fault_dev_destroy(pfs2_block_dev_t *dev) {
    if (!dev) return;
    free(dev);
}

void pfs2_fault_dev_reset(pfs2_block_dev_t *dev) {
    if (!dev) return;
    pfs2_fault_dev_t *fdev = (pfs2_fault_dev_t *)dev;
    fdev->fail_reads = 0;
    fdev->fail_writes = 0;
    fdev->fail_flushes = 0;
    fdev->fail_after_write_count = 0;
    fdev->fail_after_flush_count = 0;
    fdev->write_count = 0;
    fdev->flush_count = 0;
}
