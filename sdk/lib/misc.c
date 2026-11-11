/*
 * GBADE library: sound helpers, maths, debug output, save RAM.
 */
#include <gba.h>
#include <stdarg.h>
#include <stdio.h>

/* ========================================================================== */
/* Sound (DMG channels)                                                        */
/* ========================================================================== */

void sound_init(void) {
    REG_SNDSTAT = SSTAT_ENABLE;          /* master enable: must be first */
    /* Left/right volume 7 (max), channels 1-4 enabled on both sides. */
    REG_SNDDMGCNT = 0x0077 | 0xFF00;
    REG_SNDDSCNT = 0x0002;               /* DMG channels at 100% */
}

/* The DMG channels take a "rate" value: hz = 131072 / (2048 - rate). */
static u16 hz_to_rate(int hz) {
    if (hz < 64) hz = 64;
    if (hz > 131072) hz = 131072;
    return (u16)(2048 - 131072 / hz);
}

/* Length counter: the sound stops after (64 - n) / 256 seconds. One frame is
 * about 4.3 units. 0 frames = play until stopped. */
static u16 frames_to_length(int frames, u16 *use_length) {
    if (frames <= 0) { *use_length = 0; return 0; }
    int units = frames * 256 / 60;
    if (units > 64) units = 64;
    if (units < 1) units = 1;
    *use_length = 0x4000;
    return (u16)(64 - units);
}

void sound_beep(int hz, int frames) {
    u16 use_len;
    u16 len = frames_to_length(frames, &use_len);
    REG_SND1SWEEP = 0x0008;                          /* no sweep */
    REG_SND1CNT = (u16)(0xF000 | (2 << 6) | len);    /* vol 15, 50% duty */
    REG_SND1FREQ = (u16)(0x8000 | use_len | hz_to_rate(hz));
}

void sound_tone(int hz, int frames, int duty) {
    u16 use_len;
    u16 len = frames_to_length(frames, &use_len);
    REG_SND2CNT = (u16)(0xF000 | ((duty & 3) << 6) | len);
    REG_SND2FREQ = (u16)(0x8000 | use_len | hz_to_rate(hz));
}

void sound_noise(int frames, int pitch) {
    u16 use_len;
    u16 len = frames_to_length(frames, &use_len);
    /* volume 15, decreasing envelope step 2 */
    REG_SND4CNT = (u16)(0xF000 | (2 << 8) | len);
    REG_SND4FREQ = (u16)(0x8000 | use_len | ((pitch & 7) << 4) | 3);
}

void sound_stop(void) {
    REG_SND1CNT = 0;
    REG_SND1FREQ = 0x8000;
    REG_SND2CNT = 0;
    REG_SND2FREQ = 0x8000;
    REG_SND4CNT = 0;
    REG_SND4FREQ = 0x8000;
}

/* ========================================================================== */
/* Maths                                                                       */
/* ========================================================================== */

static const s16 sin_lut[512] = {
#include "sin_table.inc"
};

s32 lu_sin(u32 angle) { return sin_lut[(angle >> 7) & 511]; }
s32 lu_cos(u32 angle) { return sin_lut[((angle >> 7) + 128) & 511]; }

static u32 rng_state = 0x12345678;

void rand_seed(u32 seed) { rng_state = seed ? seed : 0x12345678; }

u32 rand_u32(void) {
    u32 x = rng_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    return rng_state = x;
}

int rand_range(int min, int max) {
    if (max <= min) return min;
    u32 span = (u32)(max - min) + 1;
    return min + (int)(rand_u32() % span);
}

/* ========================================================================== */
/* Debug output (mGBA debug registers)                                         */
/* ========================================================================== */
/*
 * mGBA exposes a "debug print" device:
 *   0x04FFF780  write 0xC0DE to enable; reads back 0x1DEA when available
 *   0x04FFF600  256-byte string buffer
 *   0x04FFF700  write (level | 0x100) to print the buffer
 */
#define REG_DEBUG_ENABLE (*(vu16 *)0x04FFF780)
#define REG_DEBUG_FLAGS  (*(vu16 *)0x04FFF700)
#define REG_DEBUG_STRING ((char *)0x04FFF600)

static int debug_state; /* 0 = unknown, 1 = available, -1 = not available */

bool debug_available(void) {
    if (debug_state == 0) {
        REG_DEBUG_ENABLE = 0xC0DE;
        debug_state = (REG_DEBUG_ENABLE == 0x1DEA) ? 1 : -1;
    }
    return debug_state > 0;
}

static void debug_emit(LogLevel level, const char *s, int len) {
    if (!debug_available()) return;
    if (len > 255) len = 255;
    char *dst = REG_DEBUG_STRING;
    for (int i = 0; i < len; i++) dst[i] = s[i];
    dst[len] = 0;
    REG_DEBUG_FLAGS = (u16)((level & 7) | 0x100);
}

/* printf() support: printf.c calls _putchar for every character. We collect a
 * line and send it to the console at each '\n'. */
static char line_buf[256];
static int line_len;

void _putchar(char c) {
    if (c == '\n') {
        debug_emit(LOG_INFO, line_buf, line_len);
        line_len = 0;
        return;
    }
    line_buf[line_len++] = c;
    if (line_len == 255) {
        debug_emit(LOG_INFO, line_buf, line_len);
        line_len = 0;
    }
}

void debug_puts(const char *s) {
    int n = 0;
    while (s[n]) n++;
    debug_emit(LOG_INFO, s, n);
}

void debug_log(LogLevel level, const char *fmt, ...) {
    char buf[256];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    if (n > 255) n = 255;
    debug_emit(level, buf, n);
}

void debug_breakpoint(void) {
    /* GBADE watches the console for this marker and pauses the game. */
    debug_emit(LOG_DEBUG, "\x01GBADE:BREAK", 12);
}

void debug_panic(const char *file, int line, const char *msg) {
    char buf[256];
    int n = snprintf(buf, sizeof buf, "\x01GBADE:PANIC %s:%d: %s", file, line, msg);
    if (n > 255) n = 255;
    debug_emit(LOG_ERROR, buf, n);
    /* Show something on screen too, in case it runs on real hardware. */
    REG_IME = 0;
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    m3_fill(RGB15(12, 0, 0));
    m3_text(8, 8, "PANIC!", CLR_WHITE);
    m3_text(8, 24, file, CLR_YELLOW);
    m3_printf(8, 34, CLR_YELLOW, "line %d", line);
    m3_text(8, 50, msg, CLR_WHITE);
    for (;;) {
        Halt();
    }
}

/* ========================================================================== */
/* Save RAM                                                                    */
/* ========================================================================== */

/* Emulators detect the save type by searching the ROM for this string. */
__attribute__((used, aligned(4))) static const char save_type_id[] = "SRAM_V113";

/* SRAM must be accessed 8 bits at a time, and the code doing it should not
 * run from ROM on some flash carts. We keep it simple: a byte loop. */
void save_write(u32 offset, const void *data, u32 size) {
    __asm__ volatile("" : : "r"(save_type_id)); /* keep the marker linked */
    const u8 *src = data;
    for (u32 i = 0; i < size && offset + i < SAVE_SIZE; i++) sram_mem[offset + i] = src[i];
}

void save_read(u32 offset, void *data, u32 size) {
    __asm__ volatile("" : : "r"(save_type_id)); /* keep the marker linked */
    u8 *dst = data;
    for (u32 i = 0; i < size && offset + i < SAVE_SIZE; i++) dst[i] = sram_mem[offset + i];
}

void save_erase(void) {
    for (u32 i = 0; i < SAVE_SIZE; i++) sram_mem[i] = 0xFF;
}
