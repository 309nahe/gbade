/*
 * string.h - memory and string functions (GBADE libc)
 *
 * Reminder: VRAM, palette and OAM ignore 8-bit writes. memcpy/memset here are
 * careful to use 16/32-bit accesses when pointers and sizes are aligned, but
 * for VRAM prefer dma_copy() or copying u16/u32 arrays yourself.
 */
#ifndef GBADE_STRING_H
#define GBADE_STRING_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void *memcpy(void *dst, const void *src, size_t n);
void *memmove(void *dst, const void *src, size_t n);
void *memset(void *dst, int c, size_t n);
int memcmp(const void *a, const void *b, size_t n);
void *memchr(const void *s, int c, size_t n);

size_t strlen(const char *s);
int strcmp(const char *a, const char *b);
int strncmp(const char *a, const char *b, size_t n);
char *strcpy(char *dst, const char *src);
char *strncpy(char *dst, const char *src, size_t n);
char *strcat(char *dst, const char *src);
char *strchr(const char *s, int c);
char *strrchr(const char *s, int c);
char *strstr(const char *haystack, const char *needle);

#ifdef __cplusplus
}
#endif

#endif
