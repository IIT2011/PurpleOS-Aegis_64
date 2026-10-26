#include "string.h"
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
#include <stdlib.h>
#endif

void *memset(void *s, int c, size_t n) {
    unsigned char *p = (unsigned char *)s;
    unsigned char val = (unsigned char)c;
    for (size_t i = 0; i < n; i++) {
        p[i] = val;
    }
    return s;
}

void *memcpy(void *dest, const void *src, size_t n) {
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;
    for (size_t i = 0; i < n; i++) {
        d[i] = s[i];
    }
    return dest;
}

void *memmove(void *dest, const void *src, size_t n) {
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;
    if (d < s) {
        for (size_t i = 0; i < n; i++) {
            d[i] = s[i];
        }
    } else if (d > s) {
        for (size_t i = n; i > 0; i--) {
            d[i - 1] = s[i - 1];
        }
    }
    return dest;
}

int memcmp(const void *s1, const void *s2, size_t n) {
    const unsigned char *p1 = (const unsigned char *)s1;
    const unsigned char *p2 = (const unsigned char *)s2;
    for (size_t i = 0; i < n; i++) {
        if (p1[i] != p2[i]) {
            return p1[i] - p2[i];
        }
    }
    return 0;
}

size_t strlen(const char *s) {
    size_t len = 0;
    while (s[len] != '\0') {
        len++;
    }
    return len;
}

int strcmp(const char *s1, const char *s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(const unsigned char *)s1 - *(const unsigned char *)s2;
}

int strncmp(const char *s1, const char *s2, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (s1[i] != s2[i] || s1[i] == '\0') {
            return (unsigned char)s1[i] - (unsigned char)s2[i];
        }
    }
    return 0;
}

char *strcpy(char *dest, const char *src) {
    char *d = dest;
    while ((*d++ = *src++))
        ;
    return dest;
}

char *strncpy(char *dest, const char *src, size_t n) {
    size_t i;
    for (i = 0; i < n && src[i] != '\0'; i++) {
        dest[i] = src[i];
    }
    for (; i < n; i++) {
        dest[i] = '\0';
    }
    return dest;
}

char *strcat(char *dest, const char *src) {
    char *d = dest;
    while (*d) d++;
    while ((*d++ = *src++)) ;
    return dest;
}

char *strncat(char *dest, const char *src, size_t n) {
    char *d = dest;
    while (*d) d++;
    size_t i = 0;
    while (i < n && *src) {
        *d++ = *src++;
        i++;
    }
    *d = '\0';
    return dest;
}

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wnonnull-compare"
char *strstr(const char *haystack, const char *needle) {
    if (!haystack || !needle) return NULL;
    if (*needle == '\0') return (char *)haystack;
    for (; *haystack != '\0'; haystack++) {
        if (*haystack == *needle) {
            const char *h = haystack;
            const char *n = needle;
            while (*h && *n && *h == *n) {
                h++;
                n++;
            }
            if (*n == '\0') return (char *)haystack;
        }
    }
    return NULL;
}
char *strchr(const char *s, int c) {
    if (!s) return NULL;
    char ch = (char)c;
    while (*s != '\0') {
        if (*s == ch) return (char *)s;
        s++;
    }
    return (ch == '\0') ? (char *)s : NULL;
}

char *strrchr(const char *s, int c) {
    if (!s) return NULL;
    char ch = (char)c;
    const char *last = NULL;
    while (*s != '\0') {
        if (*s == ch) last = s;
        s++;
    }
    if (ch == '\0') return (char *)s;
    return (char *)last;
}

char *strdup(const char *s) {
    if (!s) return NULL;
    size_t len = strlen(s) + 1;
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
    char *copy = (char *)malloc(len);
#else
    extern void *pmm_alloc_frames(size_t count);
    char *copy = (char *)pmm_alloc_frames((len + 4095) / 4096);
#endif
    if (copy) {
        memcpy(copy, s, len);
    }
    return copy;
}

int vsnprintf(char *str, size_t size, const char *format, va_list ap) {
    if (!str || size == 0) return 0;
    size_t written = 0;

    for (const char *p = format; *p != '\0'; p++) {
        if (*p != '%') {
            if (written + 1 < size) {
                str[written++] = *p;
            }
            continue;
        }

        p++; // skip '%'
        if (*p == '\0') break;
        if (*p == '%') {
            if (written + 1 < size) {
                str[written++] = '%';
            }
            continue;
        }

        int width = 0;
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
            case 's': {
                const char *s = va_arg(ap, const char *);
                if (!s) s = "(null)";
                while (*s && written + 1 < size) {
                    str[written++] = *s++;
                }
                break;
            }
            case 'c': {
                char c = (char)va_arg(ap, int);
                if (written + 1 < size) {
                    str[written++] = c;
                }
                break;
            }
            case 'd':
            case 'i': {
                int64_t val = (is_long >= 1) ? va_arg(ap, int64_t) : va_arg(ap, int);
                if (val < 0) {
                    if (written + 1 < size) str[written++] = '-';
                    val = -val;
                }
                char num_buf[32];
                int idx = 0;
                uint64_t uval = (uint64_t)val;
                if (uval == 0) num_buf[idx++] = '0';
                else {
                    while (uval > 0) {
                        num_buf[idx++] = '0' + (uval % 10);
                        uval /= 10;
                    }
                }
                for (int j = idx - 1; j >= 0; j--) {
                    if (written + 1 < size) str[written++] = num_buf[j];
                }
                break;
            }
            case 'u': {
                uint64_t uval = (is_long >= 1) ? va_arg(ap, uint64_t) : (uint64_t)va_arg(ap, unsigned int);
                char num_buf[32];
                int idx = 0;
                if (uval == 0) num_buf[idx++] = '0';
                else {
                    while (uval > 0) {
                        num_buf[idx++] = '0' + (uval % 10);
                        uval /= 10;
                    }
                }
                for (int j = idx - 1; j >= 0; j--) {
                    if (written + 1 < size) str[written++] = num_buf[j];
                }
                break;
            }
            case 'x':
            case 'X':
            case 'p': {
                uint64_t uval = (*p == 'p' || is_long >= 1) ? va_arg(ap, uint64_t) : (uint64_t)va_arg(ap, unsigned int);
                char num_buf[32];
                int idx = 0;
                const char *hex_digits = (*p == 'X') ? "0123456789ABCDEF" : "0123456789abcdef";
                if (uval == 0) num_buf[idx++] = '0';
                else {
                    while (uval > 0) {
                        num_buf[idx++] = hex_digits[uval & 0xF];
                        uval >>= 4;
                    }
                }
                while (idx < width) num_buf[idx++] = '0';
                for (int j = idx - 1; j >= 0; j--) {
                    if (written + 1 < size) str[written++] = num_buf[j];
                }
                break;
            }
            default:
                if (written + 1 < size) str[written++] = *p;
                break;
        }
    }

    str[written] = '\0';
    return (int)written;
}

int snprintf(char *str, size_t size, const char *format, ...) {
    va_list ap;
    va_start(ap, format);
    int ret = vsnprintf(str, size, format, ap);
    va_end(ap);
    return ret;
}

#pragma GCC diagnostic pop
