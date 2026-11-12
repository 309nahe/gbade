/*
 * GBADE libc: memory and string functions, plus the __aeabi_mem* entry points
 * the compiler uses for struct copies and array initialisation.
 *
 * This file is compiled with -fno-builtin so the compiler does not turn these
 * loops back into calls to memcpy/memset (which would recurse forever).
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

void *memcpy(void *dst, const void *src, size_t n) {
    uint8_t *d = dst;
    const uint8_t *s = src;
    if ((((uintptr_t)d | (uintptr_t)s) & 3) == 0) {
        uint32_t *dw = (uint32_t *)d;
        const uint32_t *sw = (const uint32_t *)s;
        while (n >= 16) {
            dw[0] = sw[0]; dw[1] = sw[1]; dw[2] = sw[2]; dw[3] = sw[3];
            dw += 4; sw += 4; n -= 16;
        }
        while (n >= 4) { *dw++ = *sw++; n -= 4; }
        d = (uint8_t *)dw;
        s = (const uint8_t *)sw;
        /* A 2-byte tail is copied as one 16-bit write: VRAM, palette and OAM
         * ignore 8-bit writes. */
        if (n >= 2) {
            *(uint16_t *)d = *(const uint16_t *)s;
            d += 2; s += 2; n -= 2;
        }
    } else if ((((uintptr_t)d | (uintptr_t)s) & 1) == 0) {
        uint16_t *dh = (uint16_t *)d;
        const uint16_t *sh = (const uint16_t *)s;
        while (n >= 2) { *dh++ = *sh++; n -= 2; }
        d = (uint8_t *)dh;
        s = (const uint8_t *)sh;
    }
    while (n--) *d++ = *s++;
    return dst;
}

void *memmove(void *dst, const void *src, size_t n) {
    uint8_t *d = dst;
    const uint8_t *s = src;
    if (d == s || n == 0) return dst;
    if (d < s || d >= s + n) return memcpy(dst, src, n);
    d += n;
    s += n;
    while (n--) *--d = *--s;
    return dst;
}

void *memset(void *dst, int c, size_t n) {
    uint8_t *d = dst;
    uint32_t v = (uint8_t)c;
    v |= v << 8;
    v |= v << 16;
    while (n && ((uintptr_t)d & 3)) {
        if (((uintptr_t)d & 1) == 0 && n >= 2) {
            *(uint16_t *)d = (uint16_t)v;
            d += 2; n -= 2;
        } else {
            *d++ = (uint8_t)c; n--;
        }
    }
    uint32_t *dw = (uint32_t *)d;
    while (n >= 4) { *dw++ = v; n -= 4; }
    d = (uint8_t *)dw;
    if (n >= 2) { *(uint16_t *)d = (uint16_t)v; d += 2; n -= 2; }
    if (n) *d = (uint8_t)c;
    return dst;
}

int memcmp(const void *a, const void *b, size_t n) {
    const uint8_t *x = a, *y = b;
    for (; n; n--, x++, y++)
        if (*x != *y) return *x - *y;
    return 0;
}

void *memchr(const void *s, int c, size_t n) {
    const uint8_t *p = s;
    for (; n; n--, p++)
        if (*p == (uint8_t)c) return (void *)p;
    return NULL;
}

size_t strlen(const char *s) {
    const char *p = s;
    while (*p) p++;
    return (size_t)(p - s);
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return (uint8_t)*a - (uint8_t)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (; n; n--, a++, b++) {
        if (*a != *b) return (uint8_t)*a - (uint8_t)*b;
        if (!*a) return 0;
    }
    return 0;
}

char *strcpy(char *dst, const char *src) {
    char *d = dst;
    while ((*d++ = *src++)) {}
    return dst;
}

char *strncpy(char *dst, const char *src, size_t n) {
    size_t i = 0;
    for (; i < n && src[i]; i++) dst[i] = src[i];
    for (; i < n; i++) dst[i] = 0;
    return dst;
}

char *strcat(char *dst, const char *src) {
    strcpy(dst + strlen(dst), src);
    return dst;
}

char *strchr(const char *s, int c) {
    for (;; s++) {
        if (*s == (char)c) return (char *)s;
        if (!*s) return NULL;
    }
}

char *strrchr(const char *s, int c) {
    const char *last = NULL;
    for (;; s++) {
        if (*s == (char)c) last = s;
        if (!*s) return (char *)last;
    }
}

char *strstr(const char *h, const char *n) {
    size_t len = strlen(n);
    if (!len) return (char *)h;
    for (; *h; h++)
        if (*h == *n && strncmp(h, n, len) == 0) return (char *)h;
    return NULL;
}

/* ARM EABI aliases. Note the argument order of __aeabi_memset: (dst, n, c). */
void __aeabi_memcpy(void *d, const void *s, size_t n)  { memcpy(d, s, n); }
void __aeabi_memcpy4(void *d, const void *s, size_t n) { memcpy(d, s, n); }
void __aeabi_memcpy8(void *d, const void *s, size_t n) { memcpy(d, s, n); }
void __aeabi_memmove(void *d, const void *s, size_t n)  { memmove(d, s, n); }
void __aeabi_memmove4(void *d, const void *s, size_t n) { memmove(d, s, n); }
void __aeabi_memmove8(void *d, const void *s, size_t n) { memmove(d, s, n); }
void __aeabi_memset(void *d, size_t n, int c)  { memset(d, c, n); }
void __aeabi_memset4(void *d, size_t n, int c) { memset(d, c, n); }
void __aeabi_memset8(void *d, size_t n, int c) { memset(d, c, n); }
void __aeabi_memclr(void *d, size_t n)  { memset(d, 0, n); }
void __aeabi_memclr4(void *d, size_t n) { memset(d, 0, n); }
void __aeabi_memclr8(void *d, size_t n) { memset(d, 0, n); }
