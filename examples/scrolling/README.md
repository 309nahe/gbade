# Scrolling World
<!-- order: 4 -->

A 512x256 pixel world made of tiles, scrolled by the hardware, with a sprite on top. This is how most 2D GBA games draw their levels.

## What you will learn

- Tiled backgrounds: tiles (8x8 images) + a map (which tile goes where).
- How GBADE turns a big PNG in `assets/backgrounds/` into de-duplicated tiles and a map.
- Character blocks and screen blocks: where tiles and maps live in VRAM.
- Hardware scrolling with `REG_BG0HOFS` / `REG_BG0VOFS`.
- Using two backgrounds at once (the world and a text layer).

## Files

`src/main.c` sets up two backgrounds, copies the converted world into VRAM and
updates the camera and sprite every frame. The comment block at the top lists
the VRAM layout; that layout is the part most worth understanding.

## Run it

```sh
cargo run -p gbade -- run examples/scrolling --frames 120
```

Or open the folder in GBADE and press Cmd+R. Walk with the arrow keys and
watch the camera follow.

## Next

[Pong](../pong/README.md) combines drawing, sound and saving into a complete
game.
