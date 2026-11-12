# Sound

The GBA has four legacy "DMG" channels (two square waves, a programmable wave
channel and a noise channel) plus two direct-sound FIFOs for streaming audio.
GBADE's library wraps three of them (both square waves and the noise
channel) as one-line sound effects. That is
enough for menu blips, jumps, hits and explosions.

## Start the sound engine

Call `sound_init()` once, before the first sound:

```c
sound_init();
```

It enables the master output, sets the volume to maximum and turns the DMG
channels on. Calling it again is harmless.

## Three effects

```c
sound_beep(NOTE_C5, 6);          /* square wave, 6 frames (~0.1 s) */
sound_tone(440, 30, 2);          /* 440 Hz, 30 frames, duty 2 (50%) */
sound_noise(10, 3);              /* noise burst, 10 frames, pitch 3 */
sound_stop();                    /* silence every channel */
```

| Function | Channel | Character |
|---|---|---|
| `sound_beep(hz, frames)` | 1, square | a clean beep; no sweep |
| `sound_tone(hz, frames, duty)` | 2, square | a tone with a duty cycle from 0 to 3 |
| `sound_noise(frames, pitch)` | 4, noise | hits, explosions, static |

`frames` is measured in video frames at about 60 per second, so 6 frames is
roughly a tenth of a second. Pass 0 to play until `sound_stop()` or until
another sound on the same channel replaces it.

Up to 15 frames, the sound chip itself times the sound. Longer sounds are
stopped by GBADE's VBlank interrupt, so they need your main loop to call
`vsync()`, as every example does.

`duty` changes the timbre, not the pitch. 0 and 3 are thin and nasal, 2 is the
classic 50% square, 1 is in between. `pitch` for noise goes from 0 (high
hiss) to 7 (low rumble).

## Notes

The header defines one octave:

```c
NOTE_C4  NOTE_D4  NOTE_E4  NOTE_F4  NOTE_G4  NOTE_A4  NOTE_B4  NOTE_C5
```

Double a frequency for the octave above, halve it for the octave below:

```c
sound_beep(NOTE_A4 * 2, 10);     /* one octave up */
```

Keep frequencies between roughly 64 Hz and 131 kHz; the helper clamps values
outside that range.

## One sound at a time per channel

The three helpers use three different channels, so a jump can play over a
noise effect:

```c
if (key_hit(KEY_A) && on_ground) {
    sound_beep(NOTE_C5, 6);      /* channel 1 */
}
if (ball_hits_paddle) {
    sound_noise(5, 2);           /* channel 4, does not cut the beep */
}
```

Two calls to the same helper cut each other off, which is usually what you
want for repeated effects: a second jump replaces the first beep instead of
stacking.

## Sound and the frame loop

Sound starts immediately when you call these functions. You do not need to
call them from `vsync()` or feed a buffer. If a game needs music or sampled
audio, that is a bigger project: the FIFO registers
(`REG_FIFO_A`, `REG_FIFO_B`) and DMA are exposed in `gba/sound.h` and
`gba/dma.h`, but there is no music helper in the SDK yet.

Sound is muted when **Muted** is on or **Volume** is zero in the IDE settings.
The emulator outputs audio through the system's default device.

## Try it

The [Pong example](../../examples/pong/README.md) uses beeps for paddle hits and
noise for scoring. The [Sprites example](../../examples/sprites/README.md) plays
a short beep on every jump.
