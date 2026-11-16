# 7. Sprites and Animation

Backgrounds are for the world; **sprites** are for everything that moves on
top of it: the hero, enemies, bullets. The hardware draws up to 128 of them,
each with its own position, size and picture.

## What you will learn

- How sprites are built from tiles, like backgrounds.
- Why you change sprites in a copy (the **shadow OAM**) and send it in VBlank.
- How to animate: show a different set of tiles every few frames.

## Sprites are made of tiles

Sprite tiles live in their own part of video memory (charblock 4, at
`0x06010000`, available in GBADE as `tile_obj_mem`) and use their own palette
(`pal_obj_mem`). A 16x16 sprite uses 4 tiles. With **1D mapping**
(`DCNT_OBJ_1D` in `REG_DISPCNT`) those 4 tiles simply follow each other in
memory: top-left, top-right, bottom-left, bottom-right. Sprite memory is a
pool: several sprites can show the same tiles.

## OAM and its shadow

Each sprite's settings (position, size, first tile, palette, flips) are 8
bytes in **OAM** (Object Attribute Memory, `0x07000000`), room for 128
sprites. OAM is locked while the screen is being drawn, so games keep a copy
in normal memory, change that copy whenever they like, and send it to OAM
during VBlank. In GBADE the copy is `oam_buffer`:

```c
oam_init();                        // hide all 128 sprites
sprite_set(0, x, y, SPRITE_16x16, 0, 0);   // id, position, size, first tile, palette
...
vsync();
oam_update();                      // copy oam_buffer to OAM, right after vsync
```

`oam_init()` matters: an all-zero OAM does not mean "no sprites", it means
128 small sprites showing tile 0 in the top-left corner.

## Animation

An animation is a list of frames, each a different first tile. Count frames
and switch tile every few of them: `sprite_set_tile(id, frame * 4)` for
16x16 frames stored one after another. Flip the sprite horizontally with
`sprite_flip` when it walks left, instead of drawing a second set of frames.

## Run the example

Click **▶ Try this example**. Walk with the arrows: the hero animates while
moving and turns around. Five friends below share the hero's tiles.

## Play with it

- Make the animation faster or slower (`FRAME_TIME`).
- Add a third animation frame.
- Give a friend palette bank 1 with different colours (`pal_obj_bank(1)`).

## Go further

- [Tonc: Regular sprites](https://gbadev.net/tonc/regobj.html) explains the
  three attributes, 1D and 2D mapping and the shadow OAM.
- The **Sprites** guide in the Learn panel and the **Sprites** example show
  the same thing with a sprite sheet drawn in a PNG.
