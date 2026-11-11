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
