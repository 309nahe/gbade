/*
 * gba/bios.h - BIOS functions
 *
 * The GBA's built-in BIOS ROM contains useful routines, called with the `swi`
 * (software interrupt) instruction. GBADE's emulator uses mGBA's
 * high-level BIOS emulation, so no BIOS file is needed.
 */
#ifndef GBA_BIOS_H
#define GBA_BIOS_H

#include "types.h"

/* Sleep until the next VBlank interrupt (prefer vsync() from video.h). */
void VBlankIntrWait(void);
/* Sleep until one of the interrupts in `flags` fires. */
void IntrWait(bool discard_old, u16 flags);
/* Stop the CPU until any interrupt. */
void Halt(void);
/* Restart the game. */
void SoftReset(void) __attribute__((noreturn));

/* Integer division/modulo/square root done by the BIOS.
 * (Plain `/` and `%` in C also work: the compiler uses a software routine.) */
s32 Div(s32 num, s32 den);
s32 DivMod(s32 num, s32 den);
u32 Sqrt(u32 num);
/* Angle of the vector (x, y): 0..0xFFFF for 0..360 degrees. */
u16 ArcTan2(s16 x, s16 y);

/* Copy/fill memory with the BIOS. `mode` = word count | CS_32BIT | CS_FILL. */
#define CS_FILL     (1u << 24)
#define CS_32BIT    (1u << 26)
void CpuSet(const void *src, void *dst, u32 mode);
void CpuFastSet(const void *src, void *dst, u32 mode); /* 32-byte blocks */

/* Decompress data packed with LZ77 (e.g. by GBADE's asset converter). Use the
 * Vram variant when the destination is VRAM (16-bit writes only). */
void LZ77UnCompWram(const void *src, void *dst);
void LZ77UnCompVram(const void *src, void *dst);
void RLUnCompWram(const void *src, void *dst);
void RLUnCompVram(const void *src, void *dst);

#endif /* GBA_BIOS_H */
