#include "serial.h"
#include "io.h"
#include "string.h"
#include <stdarg.h>

static int serial_initialized = 0;

void serial_init(void) {
    outb(COM1 + 1, 0x00);    // Disable all interrupts
    outb(COM1 + 3, 0x80);    // Enable DLAB (set baud rate divisor)
    outb(COM1 + 0, 0x01);    // Set divisor to 1 (lo byte) 115200 baud
    outb(COM1 + 1, 0x00);    //                  (hi byte)
    outb(COM1 + 3, 0x03);    // 8 bits, no parity, one stop bit
    outb(COM1 + 2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
    outb(COM1 + 4, 0x0B);    // IRQs enabled, RTS/DSR set
    serial_initialized = 1;
}

static int is_transmit_empty(void) {
#ifdef __KERNEL_HOST_TEST__
    return 1;
#else
    return inb(COM1 + 5) & 0x20;
#endif
}

int serial_has_char(void) {
#ifdef __KERNEL_HOST_TEST__
    return 0;
#else
    return inb(COM1 + 5) & 0x01;
#endif
}

char serial_getchar(void) {
#ifdef __KERNEL_HOST_TEST__
    return 0;
#else
    while (!serial_has_char()) {
        __asm__ volatile("pause");
    }
    return inb(COM1);
#endif
}

void serial_putchar(char c) {
    if (!serial_initialized) {
        serial_init();
    }
    while (!is_transmit_empty()) {
        __asm__ volatile("pause");
    }
    outb(COM1, c);
}

void serial_write(const char *s) {
    if (!s) return;
    while (*s) {
        if (*s == '\n') {
            serial_putchar('\r');
        }
        serial_putchar(*s++);
    }
}

static void print_uint(uint64_t val, int base, int width, char pad) {
    char buf[65];
    char digits[] = "0123456789abcdef";
    int i = 0;

    if (val == 0) {
        buf[i++] = '0';
    } else {
        while (val > 0) {
            buf[i++] = digits[val % base];
            val /= base;
        }
    }

    while (i < width) {
        buf[i++] = pad;
    }

    for (int j = i - 1; j >= 0; j--) {
        serial_putchar(buf[j]);
    }
}

static void print_int(int64_t val, int width, char pad) {
    if (val < 0) {
        serial_putchar('-');
        print_uint((uint64_t)(-val), 10, width > 0 ? width - 1 : 0, pad);
    } else {
        print_uint((uint64_t)val, 10, width, pad);
    }
}

void kvprintf(const char *fmt, va_list args) {
    for (const char *p = fmt; *p != '\0'; p++) {
        if (*p != '%') {
            if (*p == '\n') {
                serial_putchar('\r');
            }
            serial_putchar(*p);
            continue;
        }

        p++; // skip '%'
        char pad = ' ';
        int width = 0;
        int left_align = 0;

        if (*p == '-') {
            left_align = 1;
            p++;
        }

        if (*p == '0') {
            pad = '0';
            p++;
        }
        while (*p >= '0' && *p <= '9') {
            width = width * 10 + (*p - '0');
            p++;
        }

        int is_long = 0;
        if (*p == 'l') {
            is_long = 1;
            p++;
            if (*p == 'l') {
                is_long = 2;
                p++;
            }
        }

        switch (*p) {
            case 'c': {
                char c = (char)va_arg(args, int);
                serial_putchar(c);
                break;
            }
            case 's': {
                const char *s = va_arg(args, const char *);
                if (!s) s = "(null)";
                int len = (int)strlen(s);
                if (!left_align && width > len) {
                    for (int w = 0; w < width - len; w++) serial_putchar(' ');
                }
                serial_write(s);
                if (left_align && width > len) {
                    for (int w = 0; w < width - len; w++) serial_putchar(' ');
                }
                break;
            }
            case 'd':
            case 'i': {
                int64_t val = (is_long > 0) ? va_arg(args, int64_t) : va_arg(args, int);
                print_int(val, width, pad);
                break;
            }
            case 'u': {
                uint64_t val = (is_long > 0) ? va_arg(args, uint64_t) : va_arg(args, unsigned int);
                print_uint(val, 10, width, pad);
                if (left_align && width > 0) {
                    // Left align padding handled if needed
                }
                break;
            }
            case 'x':
            case 'X': {
                uint64_t val = (is_long > 0) ? va_arg(args, uint64_t) : va_arg(args, unsigned int);
                print_uint(val, 16, width, pad);
                break;
            }
            case 'p': {
                uint64_t val = (uint64_t)va_arg(args, void *);
                serial_write("0x");
                print_uint(val, 16, width > 2 ? width - 2 : 16, '0');
                break;
            }
            case '%': {
                serial_putchar('%');
                break;
            }
            default: {
                serial_putchar('%');
                serial_putchar(*p);
                break;
            }
        }
    }
}

void kprintf(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    kvprintf(fmt, args);
    va_end(args);
}

void kinfo(const char *fmt, ...) {
    serial_write("[INFO] ");
    va_list args;
    va_start(args, fmt);
    kvprintf(fmt, args);
    va_end(args);
    serial_putchar('\n');
    serial_putchar('\r');
}

void kerror(const char *fmt, ...) {
    serial_write("[ERROR] ");
    va_list args;
    va_start(args, fmt);
    kvprintf(fmt, args);
    va_end(args);
    serial_putchar('\n');
    serial_putchar('\r');
}

void kwarn(const char *fmt, ...) {
    serial_write("[WARN] ");
    va_list args;
    va_start(args, fmt);
    kvprintf(fmt, args);
    va_end(args);
    serial_putchar('\n');
    serial_putchar('\r');
}
