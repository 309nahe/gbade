# 10. Sound Effects

A jump without a "boing" feels wrong. The GBA keeps the Game Boy's sound
chip, which makes beeps and noise with almost no work from your program:
perfect for sound effects.

## What you will learn

- The GBA's six sound channels, and which ones are easy to start with.
- Playing a beep, a tone and a noise burst for a set number of frames.
- Playing a short melody by scheduling notes in the game loop.

## Six channels

Tonc's sound chapter lists them: two **square wave** channels (1 and 2), a
**wave** channel that plays a small sample (3), and a **noise** channel (4).
These four come from the original Game Boy and are often called the DMG
channels. The GBA adds two **Direct Sound** channels, A and B, that play
8-bit digital samples: that is how GBA games play music and voices, with a
software mixer.

The square and noise channels are the quickest way to get sound: set a
frequency and a length, and the hardware does the rest. Several master
registers must be configured first; `sound_init()` does it.

## Playing sounds

```c
sound_init();                    // once, at start
sound_beep(NOTE_A4, 8);          // square wave on channel 1, 8 frames
sound_tone(NOTE_C5, 8, 2);       // channel 2, duty 0..3 changes the timbre
sound_noise(20, 4);              // channel 4: explosions, hits, steps
```

Lengths are in frames (60 per second) and timed from VBlank, so keep calling
`vsync()` in your loop. Each channel plays one sound at a time: a new
`sound_beep` replaces the one still playing on channel 1.

## A melody is a schedule

To play a tune, keep a list of notes and a counter. When the counter
reaches zero, play the next note and reset the counter to its length. The
game keeps running in between: nothing waits.

## Run the example

Click **▶ Try this example**. **X** (A) jumps, **Z** (B) collects a coin,
**A** (L) explodes, **S** (R) plays a melody.

## Play with it

- Make the coin sound go up: two quick notes, the second higher.
- Change the duty of `sound_tone` (0 to 3) and listen to the difference.
- Add a sound to one of the earlier tutorials, like the coin in Collisions.

## Go further

- [Tonc: Beep! GBA sound introduction](https://gbadev.net/tonc/sndsqr.html)
  explains the square wave channels register by register.
- [Sound on the Gameboy Advance](https://stuij.github.io/deku-sound-tutorial/),
  Deku's tutorial, mirrored by the community: how to write a Direct Sound
  mixer for music and samples.
- The **Sound** guide in GBADE's Learn panel lists the sound functions.
