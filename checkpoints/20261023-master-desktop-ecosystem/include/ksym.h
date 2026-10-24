#ifndef KSYM_H
#define KSYM_H

#include "types.h"

#define KSYM_MAX_SYMBOLS 128
#define KSYM_NAME_MAX    32

typedef struct {
    char     name[KSYM_NAME_MAX];
    uint64_t address;
} ksym_entry_t;

void ksym_init(void);
int  ksym_register(const char *name, uint64_t address);
int  ksym_unregister(const char *name);
uint64_t ksym_lookup(const char *name);
uint32_t ksym_get_count(void);
const ksym_entry_t *ksym_get_entry(uint32_t index);
void ksym_dump(const char *filter);

#endif /* KSYM_H */
