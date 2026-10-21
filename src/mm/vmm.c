#include "vmm.h"
#include "pmm.h"
#include "string.h"
#include "io.h"
#include "serial.h"

static page_table_t *kernel_pml4 = NULL;

int vmm_is_canonical(uint64_t vaddr) {
    int64_t sign = (int64_t)vaddr >> 47;
    return (sign == 0 || sign == -1LL);
}

page_table_t *vmm_get_kernel_pml4(void) {
    return kernel_pml4;
}

static inline uint64_t get_pml4_idx(uint64_t vaddr) { return (vaddr >> 39) & 0x1FF; }
static inline uint64_t get_pdpt_idx(uint64_t vaddr) { return (vaddr >> 30) & 0x1FF; }
static inline uint64_t get_pd_idx(uint64_t vaddr)   { return (vaddr >> 21) & 0x1FF; }
static inline uint64_t get_pt_idx(uint64_t vaddr)   { return (vaddr >> 12) & 0x1FF; }

#define ENTRY_ADDR_MASK 0x000FFFFFFFFFF000ULL

int vmm_split_huge_page(page_table_t *pml4, uint64_t vaddr) {
    if (!vmm_is_canonical(vaddr)) return VMM_ERR_NONCANONICAL;
    if (!pml4) return VMM_ERR_NO_MEM;

    uint64_t pml4_i = get_pml4_idx(vaddr);
    uint64_t pdpt_i = get_pdpt_idx(vaddr);
    uint64_t pd_i   = get_pd_idx(vaddr);

    if (!(pml4->entries[pml4_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pdpt = (page_table_t *)(uintptr_t)(pml4->entries[pml4_i] & ENTRY_ADDR_MASK);

    if (!(pdpt->entries[pdpt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pd = (page_table_t *)(uintptr_t)(pdpt->entries[pdpt_i] & ENTRY_ADDR_MASK);

    pt_entry_t pd_val = pd->entries[pd_i];
    if (!(pd_val & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;

    /* If not a 2 MiB huge page, it is already 4 KiB mapped */
    if (!(pd_val & PAGE_HUGE)) {
        return VMM_OK;
    }

    /* Allocate new page table for splitting */
    void *new_pt_frame = pmm_alloc_frame();
    if (!new_pt_frame) {
        kerror("VMM: Out of memory while splitting huge page at %p", (void *)vaddr);
        return VMM_ERR_NO_MEM;
    }

    kinfo("VMM: Splitting huge page at %p (pd=%p, pd_i=%u, pd_val=0x%llx, new_pt=%p)",
          (void *)vaddr, pd, (uint32_t)pd_i, (unsigned long long)pd_val, new_pt_frame);

    page_table_t *new_pt = (page_table_t *)new_pt_frame;
    memset(new_pt, 0, sizeof(page_table_t));

    uint64_t huge_phys_base = pd_val & 0x000FFFFFFFE00000ULL;
    uint64_t common_flags = (pd_val & 0xFFF) & ~PAGE_HUGE;
    uint64_t nx_flag = pd_val & PAGE_NO_EXECUTE;

    for (uint64_t i = 0; i < 512; i++) {
        new_pt->entries[i] = (huge_phys_base + i * PAGE_SIZE_4K) | common_flags | nx_flag;
    }

    /* Replace 2 MiB huge page in PD with pointer to new 4 KiB PT */
    pd->entries[pd_i] = (uint64_t)(uintptr_t)new_pt_frame | PAGE_PRESENT | PAGE_WRITABLE | (pd_val & PAGE_USER);
    write_cr3(read_cr3());

    uint64_t lapic_phys = 0, lapic_flags = 0;
    int lapic_res = vmm_get_mapping(pml4, 0xfee00000, &lapic_phys, &lapic_flags);
    kinfo("VMM: After split, LAPIC (0xfee00000) mapping res=%d, phys=0x%llx, flags=0x%llx",
          lapic_res, (unsigned long long)lapic_phys, (unsigned long long)lapic_flags);

    return VMM_OK;
}

int vmm_map_page(page_table_t *pml4, uint64_t vaddr, uint64_t paddr, uint64_t flags) {
    if (!vmm_is_canonical(vaddr)) return VMM_ERR_NONCANONICAL;
    if ((vaddr & 0xFFF) || (paddr & 0xFFF)) return VMM_ERR_UNALIGNED;
    if (!pml4) return VMM_ERR_NO_MEM;

    uint64_t pml4_i = get_pml4_idx(vaddr);
    uint64_t pdpt_i = get_pdpt_idx(vaddr);
    uint64_t pd_i   = get_pd_idx(vaddr);
    uint64_t pt_i   = get_pt_idx(vaddr);

    void *alloc_pdpt = NULL;
    void *alloc_pd = NULL;
    void *alloc_pt = NULL;

    page_table_t *pdpt = NULL;
    if (!(pml4->entries[pml4_i] & PAGE_PRESENT)) {
        alloc_pdpt = pmm_alloc_frame();
        if (!alloc_pdpt) return VMM_ERR_NO_MEM;
        memset(alloc_pdpt, 0, sizeof(page_table_t));
        pml4->entries[pml4_i] = (uint64_t)(uintptr_t)alloc_pdpt | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        pdpt = (page_table_t *)alloc_pdpt;
    } else {
        pdpt = (page_table_t *)(uintptr_t)(pml4->entries[pml4_i] & ENTRY_ADDR_MASK);
        if (flags & PAGE_USER) pml4->entries[pml4_i] |= PAGE_USER;
    }

    page_table_t *pd = NULL;
    if (!(pdpt->entries[pdpt_i] & PAGE_PRESENT)) {
        alloc_pd = pmm_alloc_frame();
        if (!alloc_pd) {
            if (alloc_pdpt) {
                pml4->entries[pml4_i] = 0;
                pmm_free_frame(alloc_pdpt);
            }
            return VMM_ERR_NO_MEM;
        }
        memset(alloc_pd, 0, sizeof(page_table_t));
        pdpt->entries[pdpt_i] = (uint64_t)(uintptr_t)alloc_pd | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        pd = (page_table_t *)alloc_pd;
    } else {
        pd = (page_table_t *)(uintptr_t)(pdpt->entries[pdpt_i] & ENTRY_ADDR_MASK);
        if (flags & PAGE_USER) pdpt->entries[pdpt_i] |= PAGE_USER;
    }

    /* Check if PD entry is a huge page */
    if (pd->entries[pd_i] & PAGE_HUGE) {
        int split_res = vmm_split_huge_page(pml4, vaddr);
        if (split_res != VMM_OK) {
            return split_res;
        }
    }

    page_table_t *pt = NULL;
    if (!(pd->entries[pd_i] & PAGE_PRESENT)) {
        alloc_pt = pmm_alloc_frame();
        if (!alloc_pt) {
            if (alloc_pd) {
                pdpt->entries[pdpt_i] = 0;
                pmm_free_frame(alloc_pd);
            }
            if (alloc_pdpt) {
                pml4->entries[pml4_i] = 0;
                pmm_free_frame(alloc_pdpt);
            }
            return VMM_ERR_NO_MEM;
        }
        memset(alloc_pt, 0, sizeof(page_table_t));
        pd->entries[pd_i] = (uint64_t)(uintptr_t)alloc_pt | PAGE_PRESENT | PAGE_WRITABLE | (flags & PAGE_USER);
        pt = (page_table_t *)alloc_pt;
    } else {
        pt = (page_table_t *)(uintptr_t)(pd->entries[pd_i] & ENTRY_ADDR_MASK);
        if (flags & PAGE_USER) pd->entries[pd_i] |= PAGE_USER;
    }

    pt->entries[pt_i] = (paddr & ENTRY_ADDR_MASK) | flags | PAGE_PRESENT;
    invlpg(vaddr);

    return VMM_OK;
}

int vmm_unmap_page(page_table_t *pml4, uint64_t vaddr) {
    if (!vmm_is_canonical(vaddr)) return VMM_ERR_NONCANONICAL;
    if (vaddr & 0xFFF) return VMM_ERR_UNALIGNED;
    if (!pml4) return VMM_ERR_NO_MEM;

    uint64_t pml4_i = get_pml4_idx(vaddr);
    uint64_t pdpt_i = get_pdpt_idx(vaddr);
    uint64_t pd_i   = get_pd_idx(vaddr);
    uint64_t pt_i   = get_pt_idx(vaddr);

    if (!(pml4->entries[pml4_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pdpt = (page_table_t *)(uintptr_t)(pml4->entries[pml4_i] & ENTRY_ADDR_MASK);

    if (!(pdpt->entries[pdpt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pd = (page_table_t *)(uintptr_t)(pdpt->entries[pdpt_i] & ENTRY_ADDR_MASK);

    if (pd->entries[pd_i] & PAGE_HUGE) {
        int split_res = vmm_split_huge_page(pml4, vaddr);
        if (split_res != VMM_OK) return split_res;
    }

    if (!(pd->entries[pd_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pt = (page_table_t *)(uintptr_t)(pd->entries[pd_i] & ENTRY_ADDR_MASK);

    if (!(pt->entries[pt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;

    pt->entries[pt_i] = 0;
    invlpg(vaddr);

    return VMM_OK;
}

int vmm_get_mapping(page_table_t *pml4, uint64_t vaddr, uint64_t *out_paddr, uint64_t *out_flags) {
    if (!vmm_is_canonical(vaddr)) return VMM_ERR_NONCANONICAL;
    if (!pml4) return VMM_ERR_NO_MEM;

    uint64_t pml4_i = get_pml4_idx(vaddr);
    uint64_t pdpt_i = get_pdpt_idx(vaddr);
    uint64_t pd_i   = get_pd_idx(vaddr);
    uint64_t pt_i   = get_pt_idx(vaddr);

    if (!(pml4->entries[pml4_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pdpt = (page_table_t *)(uintptr_t)(pml4->entries[pml4_i] & ENTRY_ADDR_MASK);

    if (!(pdpt->entries[pdpt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pd = (page_table_t *)(uintptr_t)(pdpt->entries[pdpt_i] & ENTRY_ADDR_MASK);

    if (!(pd->entries[pd_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;

    if (pd->entries[pd_i] & PAGE_HUGE) {
        if (out_paddr) *out_paddr = (pd->entries[pd_i] & 0x000FFFFFFFE00000ULL) + (vaddr & 0x1FFFFFULL);
        if (out_flags) *out_flags = (pd->entries[pd_i] & 0xFFF) | (pd->entries[pd_i] & PAGE_NO_EXECUTE);
        return VMM_OK;
    }

    page_table_t *pt = (page_table_t *)(uintptr_t)(pd->entries[pd_i] & ENTRY_ADDR_MASK);
    if (!(pt->entries[pt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;

    if (out_paddr) *out_paddr = (pt->entries[pt_i] & ENTRY_ADDR_MASK) + (vaddr & 0xFFFULL);
    if (out_flags) *out_flags = (pt->entries[pt_i] & 0xFFF) | (pt->entries[pt_i] & PAGE_NO_EXECUTE);

    return VMM_OK;
}

int vmm_set_permissions(page_table_t *pml4, uint64_t vaddr, uint64_t flags) {
    if (!vmm_is_canonical(vaddr)) return VMM_ERR_NONCANONICAL;
    if (vaddr & 0xFFF) return VMM_ERR_UNALIGNED;
    if (!pml4) return VMM_ERR_NO_MEM;

    uint64_t pml4_i = get_pml4_idx(vaddr);
    uint64_t pdpt_i = get_pdpt_idx(vaddr);
    uint64_t pd_i   = get_pd_idx(vaddr);
    uint64_t pt_i   = get_pt_idx(vaddr);

    if (!(pml4->entries[pml4_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pdpt = (page_table_t *)(uintptr_t)(pml4->entries[pml4_i] & ENTRY_ADDR_MASK);

    if (!(pdpt->entries[pdpt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pd = (page_table_t *)(uintptr_t)(pdpt->entries[pdpt_i] & ENTRY_ADDR_MASK);

    if (pd->entries[pd_i] & PAGE_HUGE) {
        int split_res = vmm_split_huge_page(pml4, vaddr);
        if (split_res != VMM_OK) return split_res;
    }

    if (!(pd->entries[pd_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;
    page_table_t *pt = (page_table_t *)(uintptr_t)(pd->entries[pd_i] & ENTRY_ADDR_MASK);

    if (!(pt->entries[pt_i] & PAGE_PRESENT)) return VMM_ERR_NOT_MAPPED;

    uint64_t paddr = pt->entries[pt_i] & ENTRY_ADDR_MASK;
    pt->entries[pt_i] = paddr | flags | PAGE_PRESENT;
    invlpg(vaddr);

    return VMM_OK;
}

page_table_t *vmm_create_pml4(void) {
    void *frame = pmm_alloc_frame();
    if (!frame) return NULL;
    page_table_t *pml4 = (page_table_t *)frame;
    memset(pml4, 0, sizeof(page_table_t));

    /* If kernel_pml4 exists, copy higher-half / kernel mappings (entries 256..511) */
    if (kernel_pml4) {
        for (int i = 256; i < 512; i++) {
            pml4->entries[i] = kernel_pml4->entries[i];
        }
        /* Also keep identity mapping for low memory if active */
        pml4->entries[0] = kernel_pml4->entries[0];
    }
    return pml4;
}

void vmm_switch_pml4(page_table_t *pml4) {
    if (!pml4) return;
    write_cr3((uint64_t)(uintptr_t)pml4);
}

void vmm_init(void) {
    /* Read active CR3 from boot */
    uint64_t cr3 = read_cr3();
    kernel_pml4 = (page_table_t *)(uintptr_t)(cr3 & ENTRY_ADDR_MASK);

    /* Enforce CR0.WP (Write Protect) */
    uint64_t cr0 = read_cr0();
    cr0 |= (1ULL << 16); // CR0.WP
    write_cr0(cr0);

    kinfo("VMM Initialized: Kernel PML4 at %p, CR0.WP enforced", (void *)kernel_pml4);
}
