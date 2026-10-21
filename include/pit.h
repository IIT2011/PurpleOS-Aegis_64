#ifndef PIT_H
#define PIT_H

#include "types.h"

#define PIT_BASE_FREQ 1193182
#define PIT_TARGET_HZ 100

void pit_init(uint32_t frequency);
uint64_t pit_get_ticks(void);
void pit_delay_10ms(void);

#endif /* PIT_H */
