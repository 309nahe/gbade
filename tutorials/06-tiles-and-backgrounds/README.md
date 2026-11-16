# 6. Tiles and Backgrounds

Mode 3 is easy, but redrawing 38,400 pixels is far too slow for a real game.
Nearly every GBA game uses **tiled backgrounds** instead: small 8x8 pictures
arranged by a map, drawn by the hardware for free.

## What you will learn

- What tiles, maps, charblocks and screenblocks are.
- How to switch to mode 0 and set up a background.
- How a whole level becomes a small grid of numbers.

## Tiles and a map

A **tile** is an 8x8 picture. In the common 4bpp format each pixel is a 4-bit
colour number (0 to 15), so a tile takes 32 bytes; colour 0 is transparent.
A **map** is a grid of tile numbers: "put tile 3 here, tile 1 there". To fill
the 240x160 screen you only need a 30x20 map, and the hardware draws the tiles
by itself every frame.

## Where they live

Video memory is divided into blocks. Tiles go in **charblocks** (16 KB each,
room for 512 4bpp tiles); maps go in **screenblocks** (2 KB each, one 32x32
map, i.e. 256x256 pixels). Both kinds of block share the same memory, so pick
numbers that do not overlap: tiles in charblock 0, map in screenblock 30 is a
safe choice. Each map entry also holds flip bits and a palette bank.

```c
REG_DISPCNT = DCNT_MODE0 | DCNT_BG0;                         // tiled mode, background 0
REG_BG0CNT  = BG_CBB(0) | BG_SBB(30) | BG_4BPP | BG_REG_32x32;
tile_mem[0][1] = brick_tile;          // tile number 1 in charblock 0
se_mem[30][y * 32 + x] = 1;           // put tile 1 at map cell (x, y)
pal_bg_mem[1] = RGB15(24, 6, 4);      // colour 1 of palette bank 0
```

## Drawing tiles in practice

The example builds its tiles from text, so you can see each pixel. In a real
project you draw your tiles in an image editor: put a PNG in `assets/` and
GBADE converts it to tiles, a map and a palette at every build (see the
**Assets** reference in the Learn panel).

## Run the example

Click **▶ Try this example**. The level is a 30x20 grid of characters
(`#` for bricks, `=` for grass, `.` for sky) turned into a tile map.

## Play with it

- Redraw the level: change the strings, rebuild.
- Edit the brick tile's text to make a different pattern.
- Change palette colours: the whole level changes colour without touching
  any tile, because tiles store colour numbers, not colours.

## Go further

- [Tonc: Sprite and tiled background overview](https://gbadev.net/tonc/objbg.html)
  explains tiles, charblocks and palettes for both backgrounds and sprites.
- [Tonc: Regular tiled backgrounds](https://gbadev.net/tonc/regbg.html) covers
  map sizes, the control register and the screen entry format.
- The **Scrolling World** example shows a background converted from a PNG.
