# Assets Reference

GBADE turns every PNG under `assets/` into C data at build time. There is no
import step and no asset list: drop an image in, build, and include the
generated header.

```c
#include "assets.h"
```

The generated files are `build/gen/assets.h` and `build/gen/assets.c`, rewritten
at every build. Never edit them.

## Naming

The file name becomes the symbol prefix, lowercased with every character that
is not a letter or digit replaced by an underscore:

| File | Prefix | Notes |
|---|---|---|
| `assets/hero.png` | `hero` | lowercase name |
| `assets/Hero Walk-1.png` | `hero_walk_1` | spaces and dashes become underscores |
| `assets/hero.16x16.png` | `hero` | the `.WxH` suffix sets the frame size |
| `assets/backgrounds/world.png` | `world` | a tile map instead of a sprite sheet |

Two images whose names would produce the same prefix are an error; the
Problems panel names both files.

A prefix that would start with a digit gets a leading underscore.

## What gets generated

For every image:

| Symbol | Type | Content |
|---|---|---|
| `<prefix>_pal` | `const u16[]` | palette in BGR555, index 0 transparent |
| `<prefix>_tiles` | `const u32[]` | 8x8 tiles, packed for VRAM |
| `<prefix>_bitmap` | `const u16[]` | mode 3 pixels, only when the image fits 240x160 |
| `<prefix>_map` | `const u16[]` | tile map, only for `assets/backgrounds/` |

and these constants:

| Constant | Meaning |
|---|---|
| `<PREFIX>_WIDTH` `<PREFIX>_HEIGHT` | image size in pixels |
| `<PREFIX>_COLORS` | number of colours, excluding transparent |
| `<PREFIX>_BPP` | 4 or 8 bits per pixel |
| `<PREFIX>_TILE_COUNT` | number of tiles |
| `<PREFIX>_FRAME_WIDTH` `<PREFIX>_FRAME_HEIGHT` | frame size |
| `<PREFIX>_FRAME_TILES` | tiles per frame |
| `<PREFIX>_PAL_SIZE` `<PREFIX>_TILES_SIZE` | byte sizes, for `memcpy` |
| `<PREFIX>_MAP_WIDTH` `<PREFIX>_MAP_HEIGHT` `<PREFIX>_MAP_SIZE` | map size |
| `<PREFIX>_BITMAP_SIZE` | bitmap byte size |

The arrays are aligned for copying and marked `const`, so they stay in ROM.
The linker drops any array your game does not use.

## Transparency and colours

- A pixel with alpha below 128 is transparent.
- Pure magenta `(255, 0, 255)` is also transparent, for editors that cannot
  save alpha.
- Transparent pixels become colour index 0. Sprites never draw index 0, and
  map entries with index 0 show the backdrop.
- Up to 255 colours plus transparency. More is an error.
- 15 colours or fewer use 4bpp tiles (a 16-colour palette). More use 8bpp.
  4bpp uses half the VRAM and is the normal choice for pixel art.

Palette order is the order in which colours first appear, scanning left to
right and top to bottom. That keeps the palette stable when only a few pixels
change.

## Sprite sheets and frames

A plain image is one tile grid with 8x8 frames. Add `.WxH` before `.png` to
choose another frame size:

```text
hero.16x16.png     four 16x16 frames in a 64x16 strip
walker.32x32.png   any number of 32x32 frames
```

The frame size must be a multiple of 8 and must divide the image size. Tiles
are stored frame by frame, so frame `n` starts at tile
`n * <PREFIX>_FRAME_TILES`; that is exactly what 1D sprite mapping needs.

```c
memcpy(&tile_mem[4][0], hero_tiles, HERO_TILES_SIZE);
sprite_set(0, x, y, SPRITE_16x16, frame * HERO_FRAME_TILES, 0);
```

The name of the frame suffix is not the sprite size the hardware sees: a
16x16 frame still needs `SPRITE_16x16` at runtime. Keep the two in sync.

## Backgrounds

Any PNG inside `assets/backgrounds/` is converted as a tile map instead of a
plain sprite sheet:

- every 8x8 tile is extracted and de-duplicated;
- tiles that are a flipped copy of an earlier tile reuse it, with the flip
  bits set in the map entry;
- `<prefix>_tiles` holds only the unique tiles, `<prefix>_map` the entries.

That is why a 512x256 hand-drawn world can fit in a handful of kilobytes.

Background rules:

- Both dimensions must be multiples of 8.
- Up to 256 pixels wide: any tile-map size the hardware supports.
- Wider than 256 pixels: exactly 512 pixels wide, and 256 or 512 pixels tall.
  The GBA's larger map sizes are 64x32 and 64x64 entries.

```c
memcpy(pal_bg_bank(0), world_pal, WORLD_PAL_SIZE);
memcpy(&tile_mem[0][0], world_tiles, WORLD_TILES_SIZE);
memcpy(&se_mem[28][0], world_map, WORLD_MAP_SIZE);
REG_BG0CNT = BG_CBB(0) | BG_SBB(28) | BG_4BPP | BG_REG_64x32 | BG_PRIO(1);
```

The map is 16-bit entries: the tile index plus `SE_HFLIP`, `SE_VFLIP` and a
palette bank. `SE_PALBANK` is 0 because the converter keeps one palette.

## Bitmaps for mode 3

Any image at most 240x160 also gets `<prefix>_bitmap`, a row-major array of
16-bit colours you can copy straight into VRAM:

```c
for (int i = 0; i < HERO_BITMAP_SIZE / 2; i++) {
    m3_mem[i] = hero_bitmap[i];
}
```

Transparent pixels become 0, which in mode 3 is a real colour (black). This
array is convenient for title screens and backgrounds; for sprites use the
tile arrays.

## Errors you may see

| Message | Fix |
|---|---|
| "backgrounds must be a multiple of 8" | resize the image to a multiple of 8 |
| "backgrounds wider than 256 pixels must be exactly 512 pixels wide and 256 or 512 pixels tall" | use 512x256 or 512x512, or resize to at most 256 wide |
| "frame size must be a multiple of 8 and divide the image size" | fix the `.WxH` suffix or the sheet |
| "has more than 255 colours" | reduce colours, or use a background image that is not a tile sheet |
| "has the same C name" | rename one of the two files |

Asset errors are reported with the file name in the Problems panel, and the
build fails, so a typo cannot silently produce a wrong image.
