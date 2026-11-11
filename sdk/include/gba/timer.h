/*
 * gba/timer.h - hardware timers
 *
 * 4 timers count up at the CPU clock (16.78 MHz) divided by 1, 64, 256 or
 * 1024. When a timer overflows past 0xFFFF it reloads its start value, can
 * fire an interrupt, and can make the next timer count +1 ("cascade").
 *
 * Example: a seconds counter.
 *   Timer 0 at 1024 cycles/tick counts 16384 ticks per second, so starting
 *   at 65536 - 16384 it overflows once per second. Timer 1 in cascade mode
 *   then counts seconds:
 *
 *     REG_TM0D = 65536 - 16384;  REG_TM0CNT = TM_FREQ_1024 | TM_ENABLE;
 *     REG_TM1D = 0;              REG_TM1CNT = TM_CASCADE | TM_ENABLE;
 *     ... seconds = REG_TM1D;
 */
#ifndef GBA_TIMER_H
#define GBA_TIMER_H

#include "types.h"

#define REG_TM0D        (*(vu16 *)0x04000100)  /* read: count, write: reload */
#define REG_TM0CNT      (*(vu16 *)0x04000102)
#define REG_TM1D        (*(vu16 *)0x04000104)
#define REG_TM1CNT      (*(vu16 *)0x04000106)
#define REG_TM2D        (*(vu16 *)0x04000108)
#define REG_TM2CNT      (*(vu16 *)0x0400010A)
#define REG_TM3D        (*(vu16 *)0x0400010C)
#define REG_TM3CNT      (*(vu16 *)0x0400010E)

#define TM_FREQ_1       0x0000  /* 16.78 MHz */
#define TM_FREQ_64      0x0001  /* 262 kHz */
#define TM_FREQ_256     0x0002  /* 65.5 kHz */
#define TM_FREQ_1024    0x0003  /* 16.4 kHz */
#define TM_CASCADE      0x0004
#define TM_IRQ          0x0040
#define TM_ENABLE       0x0080

/* Simple profiling helpers using timers 2 and 3 cascaded (CPU cycles). */
void profile_start(void);
u32 profile_stop(void);

#endif /* GBA_TIMER_H */
