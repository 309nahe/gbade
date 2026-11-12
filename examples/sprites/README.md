# Sprites
<!-- order: 3 -->

A character that walks around with an animation, using hardware sprites and a PNG image converted automatically from the assets folder.

## What you will learn

- How images in `assets/` become C arrays (`#include "assets.h"`).
- Loading a palette and tiles into sprite memory.
- Showing, moving, flipping and animating a sprite with the shadow OAM.

## How the image is converted

`assets/hero.16x16.png` is a 64x16 image with 4 frames of 16x16 pixels.
The `.16x16` in the name tells GBADE the frame size, so each frame's 4 tiles
are stored one after another: frame `n` starts at tile `n * HERO_FRAME_TILES`.
Open `build/gen/assets.h` after a build to see everything that was generated.

## Files

`src/main.c` loads the palette and tiles, sets up OAM and runs the walk/jump
loop. `assets/hero.16x16.png` is the four-frame sheet.

## Run it

```sh
cargo run -p gbade -- run examples/sprites --frames 120
```

Or open the folder in GBADE and press Cmd+R. Walk with the arrow keys, jump
with X. The generated names (`hero_tiles`, `HERO_FRAME_TILES`, ...) are in
`build/gen/assets.h` after the first build.

## Next

[Scrolling World](../scrolling/README.md) puts the same character on a tiled
background with a camera.
