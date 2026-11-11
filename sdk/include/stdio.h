/*
 * stdio.h - GBADE's tiny standard I/O
 *
 * There is no file system or terminal on the GBA. In GBADE:
 *   printf / puts / putchar   -> the Console panel of the IDE (mGBA debug output)
 *   sprintf / snprintf        -> format into a string (e.g. for text_print)
 *
 * Formatting is provided by the "printf" library by Marco Paland (MIT).
 * Floating point (%f) works but is slow on the GBA.
 */
#ifndef GBADE_STDIO_H
#define GBADE_STDIO_H

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EOF (-1)

#define printf    printf_
#define sprintf   sprintf_
#define snprintf  snprintf_
#define vsnprintf vsnprintf_
#define vprintf   vprintf_

int printf_(const char *format, ...) __attribute__((format(printf, 1, 2)));
int sprintf_(char *buffer, const char *format, ...) __attribute__((format(printf, 2, 3)));
int snprintf_(char *buffer, size_t count, const char *format, ...) __attribute__((format(printf, 3, 4)));
int vsnprintf_(char *buffer, size_t count, const char *format, va_list va);
int vprintf_(const char *format, va_list va);

int puts(const char *s);
int putchar(int c);

#ifdef __cplusplus
}
#endif

#endif
