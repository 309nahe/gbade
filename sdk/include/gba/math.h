/*
 * gba/math.h - fixed-point maths, trigonometry, random numbers
 *
 * The GBA has NO floating point hardware. `float` and `double` work (the
 * compiler emulates them in software) but they are roughly 50-100x slower than
 * integers. Games use FIXED-POINT numbers instead: an integer where the low
 * bits are the fractional part.
 *
 * GBADE's `fixed` type is 24.8: the value 1.0 is stored as 256.
 *     fixed speed = FIX(1.5);           // 384
 *     pos += speed;                     // addition just works
 *     fixed area = fx_mul(w, h);        // multiplication needs a shift
 *     int pixel_x = FIX_TO_INT(pos);    // drop the fraction to draw
 *
 * Angles for lu_sin/lu_cos go from 0 to 0xFFFF for a full circle
 * (0x4000 = 90 degrees). Results are .12 fixed point (4096 = 1.0).
 */
#ifndef GBA_MATH_H
#define GBA_MATH_H

#include "types.h"

typedef s32 fixed;

#define FIX_SHIFT       8
#define FIX_ONE         (1 << FIX_SHIFT)
#define FIX(x)          ((fixed)((x) * FIX_ONE))   /* compile-time constants only */
#define INT_TO_FIX(n)   ((fixed)((n) << FIX_SHIFT))
#define FIX_TO_INT(f)   ((int)((f) >> FIX_SHIFT))
#define FIX_FRAC(f)     ((f) & (FIX_ONE - 1))

static inline fixed fx_mul(fixed a, fixed b) { return (fixed)(((s64)a * b) >> FIX_SHIFT); }
static inline fixed fx_div(fixed a, fixed b) { return (fixed)(((s64)a << FIX_SHIFT) / b); }

#define DEG_TO_ANGLE(d) ((u16)((d) * 0x10000 / 360))

/* sin/cos from a lookup table. angle: 0..0xFFFF, result: -4096..4096. */
s32 lu_sin(u32 angle);
s32 lu_cos(u32 angle);

/* Pseudo-random numbers (xorshift32). */
void rand_seed(u32 seed);
u32 rand_u32(void);
/* A random integer in [min, max] (inclusive). */
int rand_range(int min, int max);

static inline int iabs(int x) { return x < 0 ? -x : x; }
static inline int isign(int x) { return (x > 0) - (x < 0); }

#endif /* GBA_MATH_H */
