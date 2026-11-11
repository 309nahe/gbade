/*
 * stdlib.h - GBADE libc
 *
 * malloc/free use the EWRAM left over after your EWRAM variables (up to
 * 256 KB). On a console it is usually better to use static arrays, but
 * malloc is there when you need it.
 */
#ifndef GBADE_STDLIB_H
#define GBADE_STDLIB_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RAND_MAX 0x7FFFFFFF
#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

void *malloc(size_t size);
void *calloc(size_t count, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);

int rand(void);
void srand(unsigned seed);

int abs(int x);
long labs(long x);
int atoi(const char *s);
long strtol(const char *s, char **end, int base);

void abort(void) __attribute__((noreturn));
void exit(int status) __attribute__((noreturn));

void qsort(void *base, size_t count, size_t size, int (*compare)(const void *, const void *));

#ifdef __cplusplus
}
#endif

#endif
