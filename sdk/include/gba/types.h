/*
 * gba/types.h - basic types and helper macros
 *
 * The GBA's CPU (an ARM7TDMI running at 16.78 MHz) is a 32-bit CPU:
 *   - `int` and pointers are 32 bits,
 *   - `short` is 16 bits, `char` is 8 bits.
 *
 * Game Boy Advance code traditionally uses these short type names. They make
 * the size of every variable obvious, which matters a lot when you write to
 * hardware registers.
 */
#ifndef GBA_TYPES_H
#define GBA_TYPES_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

typedef uint8_t  u8;    /* unsigned  8-bit: 0 .. 255 */
typedef uint16_t u16;   /* unsigned 16-bit: 0 .. 65535 */
typedef uint32_t u32;   /* unsigned 32-bit */
typedef uint64_t u64;
typedef int8_t   s8;    /* signed  8-bit: -128 .. 127 */
typedef int16_t  s16;   /* signed 16-bit */
typedef int32_t  s32;   /* signed 32-bit */
typedef int64_t  s64;

/* "volatile" tells the compiler that a value can change behind its back
 * (because hardware changes it). Every hardware register must be volatile,
 * otherwise the compiler may optimise your reads and writes away. */
typedef volatile u8  vu8;
typedef volatile u16 vu16;
typedef volatile u32 vu32;
typedef volatile s16 vs16;
typedef volatile s32 vs32;

/* --- Where code and data go -----------------------------------------------
 *
 * By default your code runs from ROM as Thumb code (16-bit instructions,
 * which is the fastest option for ROM because the cartridge bus is 16 bits).
 *
 * For a function that must be really fast (called thousands of times per
 * frame), put it in IWRAM as ARM code:
 *
 *     IWRAM_CODE void draw_particles(void) { ... }
 *
 * EWRAM_DATA / EWRAM_BSS place big arrays in the 256 KB external RAM instead
 * of the small 32 KB internal RAM:
 *
 *     EWRAM_BSS u8 big_buffer[100000];        // zero-initialised
 *     EWRAM_DATA int table[4] = {1, 2, 3, 4}; // initialised
 */
#define IWRAM_CODE  __attribute__((section(".iwram.text"), target("arm"), noinline))
#define ARM_CODE    __attribute__((target("arm")))
#define THUMB_CODE  __attribute__((target("thumb")))
#define IWRAM_DATA  __attribute__((section(".iwram.data")))
#define EWRAM_DATA  __attribute__((section(".ewram.data")))
#define EWRAM_BSS   __attribute__((section(".ewram.bss")))

/* Alignment helpers: some hardware (DMA, BIOS copies) needs aligned data. */
#define ALIGN(n)    __attribute__((aligned(n)))
#define ALIGN4      __attribute__((aligned(4)))
#define INLINE      static inline

/* Bit helpers */
#define BIT(n)              (1u << (n))
#define ARRAY_SIZE(a)       (sizeof(a) / sizeof((a)[0]))
#define CLAMP(x, lo, hi)    ((x) < (lo) ? (lo) : ((x) > (hi) ? (hi) : (x)))
#define MIN(a, b)           ((a) < (b) ? (a) : (b))
#define MAX(a, b)           ((a) > (b) ? (a) : (b))

#endif /* GBA_TYPES_H */
