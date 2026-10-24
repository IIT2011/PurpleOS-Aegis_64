#ifndef SHELL_H
#define SHELL_H

#include "types.h"

#define SHELL_MAX_FILENAME_LEN 23

/* Shell-side path normalization helper for root-level PurpleFS operations */
int shell_normalize_path(const char *path, char *out, size_t out_cap);

void shell_init(void);
void shell_step(void);
void shell_run(void);

#endif /* SHELL_H */
