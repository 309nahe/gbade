/*
 * gba/debug.h - printing to the GBADE console
 *
 * The best debugging tool is printf. GBADE's emulator (built on mGBA)
 * supports a special "debug output" device: text written there appears in the
 * Console panel of the IDE. On a real GBA it is simply ignored, so it is safe
 * to leave debug prints in your game.
 *
 *     #include <stdio.h>
 *     printf("player at %d,%d\n", x, y);       // -> Console panel
 *
 *     debug_log(LOG_WARN, "low health: %d", hp);  // with a level/colour
 *
 *     ASSERT(index < 10);   // if false: prints file + line and stops the game
 */
#ifndef GBA_DEBUG_H
#define GBA_DEBUG_H

#include "types.h"

typedef enum {
    LOG_FATAL = 0,
    LOG_ERROR = 1,
    LOG_WARN  = 2,
    LOG_INFO  = 3,
    LOG_DEBUG = 4,
} LogLevel;

/* true when running in an emulator with the debug output device. */
bool debug_available(void);
/* Print one line at a given level (adds the newline for you). */
void debug_log(LogLevel level, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
/* Print a string as-is (no formatting). */
void debug_puts(const char *s);

/* Stop the game and show a message. GBADE pauses and highlights the line. */
void debug_panic(const char *file, int line, const char *msg) __attribute__((noreturn));

#define ASSERT(cond) \
    do { if (!(cond)) debug_panic(__FILE__, __LINE__, "assertion failed: " #cond); } while (0)

/* A software breakpoint: GBADE pauses the game here (like a debugger). */
#define BREAKPOINT() debug_breakpoint()
void debug_breakpoint(void);

#endif /* GBA_DEBUG_H */
