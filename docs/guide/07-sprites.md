# Sprites

A sprite is a small image the hardware draws on top of the backgrounds. You do
not redraw its pixels every frame: you load its tiles once and then tell the
hardware where to put it. Moving a sprite is one register pair, and it stays
fast even with many of them.

The GBA calls sprites "objects", hence OAM (Object Attribute Memory) and the
`DCNT_OBJ` display bit.

## The three steps

```c
#include "assets.h"        /* generated from assets/hero.16x16.png */

/* 1. Palette: sprite palette bank 0. */
memcpy(pal_obj_bank(0), hero_pal, HERO_PAL_SIZE);

/* 2. Tiles: sprite tile memory, character block 4. */
memcpy(&tile_mem[4][0], hero_tiles, HERO_TILES_SIZE);

/* 3. Sprites: hide all 128, then place ours. */
oam_init();
sprite_set(0, 100, 60, SPRITE_16x16, 0, 0);
REG_DISPCNT |= DCNT_OBJ | DCNT_OBJ_1D;
```

`DCNT_OBJ_1D` tells the hardware that a sprite's tiles are consecutive in
memory, which is how GBADE stores sprite sheets. Without it, tiles are laid
out in the old 2D pattern and a 16x16 sprite would read the wrong blocks.

## Shadow OAM

Writing to the real OAM while the screen is drawing causes glitches. The SDK
keeps a copy in normal RAM and copies it during VBlank:

```c
while (1) {
    vsync();
    oam_update();     /* copy the whole shadow OAM to the hardware */
    key_poll();

    sprite_move(0, x, y);
    sprite_set_tile(0, frame * HERO_FRAME_TILES);
    sprite_flip(0, facing_left, false);
}
```

All `sprite_*` functions change the shadow copy, never the hardware. Call
`oam_update()` once per frame, right after `vsync()`.

`oam_init()` hides all 128 sprites and resets the shadow copy. It is not
optional: OAM powers on with garbage, and a hidden sprite is marked with
`ATTR0_HIDE`.

## Positions and sizes

`sprite_set(id, x, y, size, tile, palbank)` and `sprite_move(id, x, y)` take
the **top-left corner** in screen pixels. A 16x16 sprite placed at `(112, 96)`
covers `112..127` and `96..111`.

| Size constant | Pixels | Tiles used (4bpp) |
|---|---|---|
| `SPRITE_8x8` | 8x8 | 1 |
| `SPRITE_16x16` | 16x16 | 4 |
| `SPRITE_32x32` | 32x32 | 16 |
| `SPRITE_64x64` | 64x64 | 64 |
| `SPRITE_16x8` | 16x8 | 2 |
| `SPRITE_32x8` | 32x8 | 4 |
| `SPRITE_32x16` | 32x16 | 8 |
| `SPRITE_64x32` | 64x32 | 16 |
| `SPRITE_8x16` | 8x16 | 2 |
| `SPRITE_8x32` | 8x32 | 4 |
| `SPRITE_16x32` | 16x32 | 8 |
| `SPRITE_32x64` | 32x64 | 16 |

`sprite_width(size)` and `sprite_height(size)` return the pixel size when you
need it, for example in collision code.

Positions are screen coordinates. For a scrolling game, subtract the camera
before calling `sprite_move`; see [Graphics](05-graphics.md).

## Tiles, flips and priority

```c
sprite_set_tile(0, tile);        /* first tile of this frame */
sprite_flip(0, true, false);     /* horizontal flip, no vertical flip */
sprite_set_priority(0, 0);       /* 0 = in front of backgrounds */
sprite_hide(0);
sprite_show(0);
```

A sprite's tiles are a window into sprite VRAM: `tile` is the index of the
first tile, and the size decides how many consecutive tiles are used. With
the generated `hero.16x16` sheet, frame `n` starts at `n * HERO_FRAME_TILES`
because each 16x16 frame is 4 tiles and the frames are stored one after
another.

Flipping is free: it is a bit in the sprite's attributes. Use it for
left-facing and right-facing versions of the same artwork instead of a second
sheet.

## Palettes and colours

Sprites use their own 256-colour palette at `0x05000200`. A 4bpp sprite uses
one 16-colour bank, chosen by the last argument of `sprite_set` or with
`ATTR2_PALBANK`. Bank 0 is the usual choice.

Colour index 0 is transparent, so a sprite always lets the background show
through. In `assets/`, fully transparent pixels and pure magenta
`(255, 0, 255)` both become index 0. That makes magenta a reliable
"invisible" colour in an image editor that cannot save transparency.

## Animation

An animation is a loop over frame numbers, slowed down by a counter:

```c
if (moving && ++anim_timer >= 6) {
    anim_timer = 0;
    frame = (frame + 1) % (HERO_TILE_COUNT / HERO_FRAME_TILES);
}
if (!on_ground) {
    frame = 2;              /* in the air */
}
```

`HERO_TILE_COUNT / HERO_FRAME_TILES` is the number of frames in the sheet. Six
frames at 60 Hz is about ten animation frames per second, a common rate
for GBA art. Count frames, not milliseconds: the console has no clock to ask
and the frame loop is the clock.

For a sprite sheet, name the file with its frame size, such as
`hero.16x16.png`, and GBADE cuts it into frames for you. See
[Project Anatomy](03-project-anatomy.md) and the
[Assets reference](../assets.md).

## A whole sprite in one call

`sprite_set` writes all three attribute words: position, size, tile, palette
bank and colour depth. Multi-value helpers are:

```c
OBJ_ATTR *a = sprite_set(3, x, y, SPRITE_16x16, 0, 0);
a->attr0 |= ATTR0_BLEND;    /* semi-transparent, if blending is enabled */
```

`sprite_set` returns the entry in the shadow OAM, so the raw `ATTR0_*`,
`ATTR1_*` and `ATTR2_*` constants remain available for effects the helpers do
not cover: rotation, mosaic, blending and windows.

## Limits

- 128 sprites, but only 32 per scanline. A game that draws all its bullets as
  sprites can hit this; split large effects into background tiles or accept
  flicker.
- 1024 tiles (32 KB) of sprite VRAM shared by all sprites.
- OAM is 1 KB: 128 entries of 8 bytes.

## Try it

The [Sprites example](../../examples/sprites/README.md) walks a four-frame hero
with gravity and a jump, and [Scrolling World](../../examples/scrolling/README.md)
puts the same sprite on a scrolling background.
