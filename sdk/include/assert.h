/*
 * assert.h - if the condition is false, GBADE stops the game, prints the file
 * and line in the Console, and highlights the line in the editor.
 * Define NDEBUG before including this header to disable assertions.
 */
#ifndef GBADE_ASSERT_H
#define GBADE_ASSERT_H

void debug_panic(const char *file, int line, const char *msg) __attribute__((noreturn));

#ifdef NDEBUG
#define assert(cond) ((void)0)
#else
#define assert(cond) ((cond) ? (void)0 : debug_panic(__FILE__, __LINE__, "assertion failed: " #cond))
#endif

#endif
