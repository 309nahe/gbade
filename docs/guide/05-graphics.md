# Graphics

Everything the GBA shows is one of three things: a bitmap, a tiled background,
or a sprite. The display mode decides which of them the hardware can mix, and
the rest is memory and registers.

## The six display modes

The mode is set in `REG_DISPCNT` and cannot be changed mid-frame. Backgrounds
are numbered BG0 to BG3; lower numbers are drawn in front when their
priorities are equal.

| Mode | Backgrounds | Best for |
|---|---|---|
| 0 | four tiled, no rotation | most 2D games |
| 1 | two tiled, one affine | rotating or scaling a layer |
| 2 | two affine | two rotating layers |
| 3 | one 240x160 bitmap, 16-bit colour | learning, painting tools |
| 4 | two 240x160 bitmaps, 8-bit palette | page-flipped software rendering |
| 5 | two 160x128 bitmaps, 16-bit colour | small fast software rendering |

Sprites work in every mode, on top of the backgrounds.

A minimal setup looks like this:

```c
REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
```

Enable the backgrounds and sprites you actually use. A mode 0 game with one
background sets `DCNT_MODE0 | DCNT_BG0`; adding `DCNT_OBJ` turns sprites on.

## Colours

The GBA stores colours as 15-bit values: 5 bits each for red, green and blue,
with bit 15 unused.

```c
RGB15(31, 0, 0)      /* full red, using the hardware's 0..31 range */
RGB8(255, 0, 0)      /* the same red, using the usual 0..255 range */
CLR_WHITE            /* ready-made constants: CLR_BLACK, CLR_RED, ... */
```

There are two separate palettes: 256 entries for backgrounds at
`0x05000000` and 256 for sprites at `0x05000200`. In 4bpp modes each graphics
tile or sprite uses a 16-colour bank from its palette, selected by a block or
bank number. Colour index 0 is transparent for sprites and for the first
background palette entry.

## Bitmap drawing (mode 3)

Mode 3 gives you one array of 240x160 16-bit pixels starting at
`0x06000000`, the same memory the tiles would use.

```c
REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;

m3_fill(RGB15(0, 0, 8));                    /* clear */
m3_plot(10, 10, CLR_WHITE);                 /* one pixel, bounds-checked */
m3_rect(20, 20, 50, 30, CLR_RED);           /* filled rectangle */
m3_frame(80, 20, 50, 30, CLR_LIME);         /* outline only */
m3_line(0, 100, 239, 140, CLR_YELLOW);
m3_circle(120, 80, 30, CLR_CYAN, true);     /* filled */
m3_text(40, 130, "Hello!", CLR_WHITE);
```

You can also write directly, which is slightly faster but you must stay
inside the screen:

```c
m3_mem[y * SCREEN_WIDTH + x] = color;
```

Mode 3 is the easiest way to draw, and the slowest: the CPU writes every
visible pixel itself. It is ideal for a first game and for tools, not for a
scrolling action game.

The [Drawing example](11-examples-tour.md) uses mode 3 for a paint program and
shows a useful trick: save the pixels under a cursor before drawing it, then
put them back next frame. A bitmap has no layers, so anything you draw must be
erased by hand.

## Page flipping (mode 4)

Mode 4 is a 240x160 bitmap with 8-bit palette indices, and two pages: one is
shown while you draw in the other. Drawing a whole frame in the hidden page
and then flipping removes flicker.

```c
REG_DISPCNT = DCNT_MODE4 | DCNT_BG2;
m4_fill(0);            /* clear the hidden page */
/* ... draw the whole frame with m4_plot, m4_rect ... */
m4_flip();             /* show it; the other page becomes the hidden one */
```

The `m4_*` functions always draw into the hidden page, never the visible one.
`m4_flip` swaps the pages and returns the new hidden page, which is where the
next frame goes. Fill `pal_bg_mem` with your 256 colours before drawing. Because VRAM is 16-bit wide, `m4_plot` reads
and writes two pixels at once; it is slower than it looks.

## Tiled backgrounds

A tiled background is two arrays:

- **Tiles**: 8x8 pixel images in a *character block*, 16 KB of VRAM. A 4bpp
  tile is 32 bytes, an 8bpp tile 64 bytes.
- **Map**: a grid of 16-bit entries in a *screen block*, 2 KB each. Each entry
  says which tile to show, whether to flip it, and which palette bank to use.

The hardware repeats the map across the screen; you never draw the tiles
yourself. A 32x32 entry map covers 256x256 pixels, larger maps use more screen
blocks.

```c
/* Put a converted image into VRAM. The map is 64 columns wide. */
memcpy(pal_bg_bank(0), world_pal, WORLD_PAL_SIZE);
memcpy(&tile_mem[0][0], world_tiles, WORLD_TILES_SIZE);
memcpy(&se_mem[28][0], world_map, WORLD_MAP_SIZE);

/* Background 0: tiles in block 0, map in block 28, 64x32, behind sprites. */
REG_BG0CNT = BG_CBB(0) | BG_SBB(28) | BG_4BPP | BG_REG_64x32 | BG_PRIO(1);
REG_DISPCNT = DCNT_MODE0 | DCNT_BG0;
```

`tile_mem[n]` is character block `n`, `se_mem[n]` is screen block `n`. Both
arrays index VRAM for you.

A screen block row is always 32 entries. Maps that are exactly 256 or 512
pixels wide (32 or 64 columns) are stored in that order and copy with one
`memcpy`. Narrower maps store one row per image row, so copy them row by row
into 32-entry rows; [Assets](../assets.md) shows the loop.

Map sizes:

| Constant | Pixels | Screen blocks |
|---|---|---|
| `BG_REG_32x32` | 256x256 | 1 |
| `BG_REG_64x32` | 512x256 | 2, side by side |
| `BG_REG_32x64` | 256x512 | 2, one above the other |
| `BG_REG_64x64` | 512x512 | 4 |

## Scrolling

The visible window onto a tiled background is 240x160 pixels. Scrolling moves
that window, in hardware, by writing two registers:

```c
REG_BG0HOFS = camera_x;
REG_BG0VOFS = camera_y;
```

The values wrap around the background's size, so a 512x256 world repeats when
the camera passes its edge. Clamp the camera to the world to avoid showing the
wrap:

```c
int cam_x = CLAMP(hero_x - SCREEN_WIDTH / 2, 0, WORLD_W - SCREEN_WIDTH);
```

These registers are write-only on real hardware. Do not read them back; keep
the camera position in a variable of your own.

Sprites use screen coordinates, not world coordinates. Subtract the camera
when you position one:

```c
sprite_move(0, hero_x - cam_x, hero_y - cam_y);
```

## Priorities

Every background and every sprite has a priority from 0 (front) to 3 (back).
Within the same priority, lower background numbers are in front. Use
priorities to put a text layer above the world without moving it in memory:

```c
REG_BG1CNT |= BG_PRIO(0);   /* text in front */
```

Sprites use `ATTR2_PRIO(n)`; `sprite_set_priority` writes it for you.

## Blend, fade, mosaic and windows

Three effects are built into the display controller and cost nothing per
pixel:

- **Alpha blending and fades**: `REG_BLDCNT` selects which layers are
  affected, the mode (alpha, fade to white, fade to black) and the second
  layer for alpha; `REG_BLDALPHA` and `REG_BLDY` set the amounts.
- **Mosaic**: `REG_MOSAIC` enlarges pixels into blocks, a cheap "pixelate".
- **Windows**: `REG_WIN0H`, `REG_WIN0V`, `REG_WININ` and `REG_WINOUT` restrict
  drawing to a rectangle or to sprite-shaped areas.

These are registers, not API calls. The [SDK reference](../sdk-api.md) lists
the constants.

## Where the memory is

The 96 KB of VRAM is shared by every layer, so block numbers are a budget:

- Character blocks: 0 to 3 for backgrounds, 4 to 5 for sprites (sprite block
  `4` is `0x06010000`).
- Screen blocks: 0 to 31.
- Mode 3 overwrites most of VRAM with the bitmap. Do not mix it with many
  tiled backgrounds.
- Text mode uses the end of VRAM by default (character block 3, screen block
  31), which is why the scrolling example puts the world map in blocks 28 and
  29.

## See it working

- [Drawing](../../examples/drawing/README.md) mode 3 pixels, lines and circles.
- [Scrolling World](../../examples/scrolling/README.md) a 512x256 tiled world with
  a camera.
- [Sprites](07-sprites.md) the moving character on top.
