#ifndef SERIAL_H
#define SERIAL_H

#include "types.h"

#define COM1 0x3F8

void serial_init(void);
void serial_putchar(char c);
void serial_write(const char *s);
int serial_has_char(void);
char serial_getchar(void);

/* Kernel logging and formatted output */
void kprintf(const char *fmt, ...);
void kinfo(const char *fmt, ...);
void kerror(const char *fmt, ...);
void kwarn(const char *fmt, ...);

#endif /* SERIAL_H */
