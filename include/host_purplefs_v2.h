#ifndef HOST_PURPLEFS_V2_H
#define HOST_PURPLEFS_V2_H

#include "purplefs_v2.h"

/* In-memory block device */
pfs2_block_dev_t *pfs2_mem_dev_create(uint32_t block_count);
void pfs2_mem_dev_destroy(pfs2_block_dev_t *dev);

/* Regular file-backed block device */
pfs2_block_dev_t *pfs2_file_dev_create(const char *filepath, uint32_t block_count);
void pfs2_file_dev_destroy(pfs2_block_dev_t *dev);

/* Fault-injecting block device wrapper */
typedef struct pfs2_fault_dev {
    pfs2_block_dev_t base;
    pfs2_block_dev_t *target;
    int fail_reads;
    int fail_writes;
    int fail_flushes;
    int fail_after_write_count;
    int write_count;
    int fail_after_flush_count;
    int flush_count;
} pfs2_fault_dev_t;

pfs2_block_dev_t *pfs2_fault_dev_create(pfs2_block_dev_t *target);
void pfs2_fault_dev_destroy(pfs2_block_dev_t *dev);
void pfs2_fault_dev_reset(pfs2_block_dev_t *dev);

#endif /* HOST_PURPLEFS_V2_H */
