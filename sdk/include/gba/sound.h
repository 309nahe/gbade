/*
 * gba/sound.h - sound
 *
 * The GBA has 6 sound channels:
 *   1, 2  square waves ("beeps"; channel 1 can also slide the pitch)
 *   3     a custom 32-sample waveform
 *   4     noise (explosions, drums, footsteps)
 *   A, B  "Direct Sound": play 8-bit samples (music, voices) fed by DMA
 *
 * Channels 1-4 come from the original Game Boy, which is why they are called
 * the "DMG" channels. They are perfect for retro sound effects and need no
 * memory at all. This file gives you a few easy functions for them:
 *
 *     sound_init();
 *     sound_beep(440, 10);       // 440 Hz (an A note) for 10 frames
 *     sound_noise(20, 3);        // a short "explosion"
 *
 * For full control, the raw registers are here too (see GBATEK / Tonc).
 */
#ifndef GBA_SOUND_H
#define GBA_SOUND_H

#include "types.h"

#define REG_SND1SWEEP   (*(vu16 *)0x04000060)
#define REG_SND1CNT     (*(vu16 *)0x04000062)
#define REG_SND1FREQ    (*(vu16 *)0x04000064)
#define REG_SND2CNT     (*(vu16 *)0x04000068)
#define REG_SND2FREQ    (*(vu16 *)0x0400006C)
#define REG_SND3SEL     (*(vu16 *)0x04000070)
#define REG_SND3CNT     (*(vu16 *)0x04000072)
#define REG_SND3FREQ    (*(vu16 *)0x04000074)
#define REG_SND4CNT     (*(vu16 *)0x04000078)
#define REG_SND4FREQ    (*(vu16 *)0x0400007C)
#define REG_SNDDMGCNT   (*(vu16 *)0x04000080)
#define REG_SNDDSCNT    (*(vu16 *)0x04000082)
#define REG_SNDSTAT     (*(vu16 *)0x04000084)
#define REG_SNDBIAS     (*(vu16 *)0x04000088)
#define REG_WAVE_RAM    ((vu32 *)0x04000090)
#define REG_FIFO_A      (*(vu32 *)0x040000A0)
#define REG_FIFO_B      (*(vu32 *)0x040000A4)

#define SSTAT_ENABLE    0x0080

/* Turn on the sound hardware with the 4 DMG channels at full volume. */
void sound_init(void);

/* Play a square-wave note on channel 1 (`hz` = 64..131072) for `frames`
 * frames (1 frame = 1/60 s; 0 = until stopped). */
void sound_beep(int hz, int frames);
/* Same on channel 2, with a duty cycle 0..3 (12.5%, 25%, 50%, 75%) which
 * changes the timbre. */
void sound_tone(int hz, int frames, int duty);
/* A noise burst on channel 4. `frames` = length, `pitch` 0 (high) .. 7 (low). */
void sound_noise(int frames, int pitch);
/* Silence channels 1, 2 and 4. */
void sound_stop(void);

/* Musical notes, octave 4 (middle). Multiply/divide by 2 to change octave. */
#define NOTE_C4  262
#define NOTE_D4  294
#define NOTE_E4  330
#define NOTE_F4  349
#define NOTE_G4  392
#define NOTE_A4  440
#define NOTE_B4  494
#define NOTE_C5  523

#endif /* GBA_SOUND_H */
