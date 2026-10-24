#ifndef VMM_H
#define VMM_H

#include "types.h"

#define PAGE_PRESENT      (1ULL << 0)
#define PAGE_WRITABLE     (1ULL << 1)
#define PAGE_USER         (1ULL << 2)
#define PAGE_PWT          (1ULL << 3)
#define PAGE_PCD          (1ULL << 4)
#define PAGE_ACCESSED     (1ULL << 5)
#define PAGE_DIRTY        (1ULL << 6)
#define PAGE_HUGE         (1ULL << 7)
#define PAGE_GLOBAL       (1ULL << 8)
#define PAGE_NO_EXECUTE   (1ULL << 63)

#define PAGE_SIZE_4K      4096ULL
#define PAGE_SIZE_2M      (2ULL * 1024ULL * 1024ULL)

/* Page permission convenience macros */
#define VMM_PROT_READ     PAGE_PRESENT
#define VMM_PROT_WRITE    PAGE_WRITABLE
#define VMM_PROT_EXEC     0ULL          /* NX bit is 0 for executable */
#define VMM_PROT_NO_EXEC  PAGE_NO_EXECUTE

/* Error codes */
#define VMM_OK                  0
#define VMM_ERR_NONCANONICAL   -1
#define VMM_ERR_UNALIGNED      -2
#define VMM_ERR_NO_MEM         -3
#define VMM_ERR_NOT_MAPPED     -4
#define VMM_ERR_ALREADY_MAPPED -5

typedef uint64_t pt_entry_t;

typedef struct page_table {
    pt_entry_t entries[512];
} __attribute__((aligned(4096))) page_table_t;

void vmm_init(void);
page_table_t *vmm_get_kernel_pml4(void);
page_table_t *vmm_create_pml4(void);

int vmm_is_canonical(uint64_t vaddr);
int vmm_map_page(page_table_t *pml4, uint64_t vaddr, uint64_t paddr, uint64_t flags);
int vmm_unmap_page(page_table_t *pml4, uint64_t vaddr);
int vmm_get_mapping(page_table_t *pml4, uint64_t vaddr, uint64_t *out_paddr, uint64_t *out_flags);
int vmm_set_permissions(page_table_t *pml4, uint64_t vaddr, uint64_t flags);
int vmm_split_huge_page(page_table_t *pml4, uint64_t vaddr);

void vmm_switch_pml4(page_table_t *pml4);

#endif /* VMM_H */
