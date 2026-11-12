# Drawing (Mode 3)
<!-- order: 2 -->

Draw pixels, lines, rectangles and circles directly on the screen, then paint with the D-pad. The simplest graphics mode of the GBA.

## What you will learn

- **Mode 3**: the screen is a 240x160 array of 16-bit colours in VRAM.
- How colours work on the GBA (`RGB15`, 5 bits per channel).
- How to draw a moving cursor without leaving a trail (save what is under it, restore it next frame).

## Controls

- D-pad: move the brush
- A: paint
- B: change colour
- START: clear the canvas

## Files

`src/main.c` holds the toolbar, the shape demo and the cursor. The three ideas
worth copying are `REG_DISPCNT = DCNT_MODE3 | DCNT_BG2`, the `m3_*` drawing
functions, and the save-under/restore pattern that lets a cursor move without
leaving a trail.

## Run it

```sh
cargo run -p gbade -- run examples/drawing --frames 120
```

Or open the folder in GBADE and press Cmd+R. Move with the arrow keys, paint
with X, change colour with Z, clear with Enter.

## Next

[Sprites](../sprites/README.md) stops drawing pixels by hand and lets the
hardware draw a character.
