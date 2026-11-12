# The Examples, Explained

The repository ships five projects in `examples/`. They are ordered from
simplest to most complete, and each one is small enough to read in one sitting.
This page explains what each teaches and where to look in its source.

Create your own copy from any of them with **File > New Project…**, or from a
terminal:

```sh
gbade new my-game --template sprites --name "My Game"
```

## 1. Hello World

The smallest complete program: text, buttons and a console.

**Look at `src/main.c`** and notice the shape that every later example keeps:

```c
text_init();
while (1) {
    vsync();
    key_poll();
    /* ... */
}
```

- `text_init()` builds a 30x20 character console on background 0.
- `text_print` and `text_printf` draw on the screen.
- `printf` goes to the IDE's Console panel instead.
- `key_hit(KEY_A)` counts presses; `key_held` and `key_tri_horz` behave
  differently, which the example shows side by side.

**Try:** change the backdrop colour, print `frame_count()`, make B clear the
screen, or change what START does.

## 2. Drawing (mode 3)

A paint program with a colour toolbar, a moving brush and a "save what is
under the cursor" trick.

- `REG_DISPCNT = DCNT_MODE3 | DCNT_BG2` switches to a 240x160 bitmap.
- `m3_fill`, `m3_rect`, `m3_frame`, `m3_line`, `m3_circle` and `m3_text` draw
  into VRAM directly.
- The cursor is not a layer: the code saves the 5x5 pixels under it and
  restores them before moving. Any animated thing in a bitmap needs this
  erase-and-redraw pattern.
- Holding A paints while the D-pad moves, so painting and moving happen in the
  same frame.

**Try:** add a bigger brush, a colour picker that reads the pixel under the
cursor, or an undo buffer (a copy of `m3_mem`).

## 3. Sprites

A four-frame character that walks, flips and jumps with gravity.

- `assets/hero.16x16.png` is a 64x16 strip: four 16x16 frames. The generated
  `hero_tiles` and `hero_pal` are copied into sprite VRAM and palette.
- `oam_init()` hides all 128 sprites; `sprite_set` places sprite 0.
- `oam_update()` runs right after `vsync()` so OAM changes land in VBlank.
- The jump uses fixed point: `vy = FIX(-4.5)`, then `vy += FIX(0.25)` every
  frame. Integer pixel speeds would look jerky.
- Animation is a counter: every 6 frames, advance to the next of 4 frames.

**Try:** make the hero run faster when B is held, add a second sprite that
follows the player, or read the palette and tiles in the inspector while the
game runs.

## 4. Scrolling World

A 512x256 tiled world with a camera and the same hero on top.

- `assets/backgrounds/world.png` becomes `world_tiles` (only unique 8x8 tiles)
  and `world_map`. A 512x256 world is 64x32 tiles and needs two screen blocks.
- The VRAM layout is written at the top of `main.c`: world tiles in character
  block 0, the map in screen blocks 28 and 29, text in block 31, sprites in
  block 4.
- Scrolling is two registers: `REG_BG0HOFS` and `REG_BG0VOFS`.
- The camera is clamped to the world, and sprites are drawn at
  `hero - camera` because sprites use screen coordinates.

**Try:** make the world 512x512 (four screen blocks), add a parallax layer
moving at half speed, or let the camera lead the direction of travel.

## 5. Pong

A complete game in one file: title screen, playing, game over, sound and a
saved best rally.

- A three-state machine (`TITLE`, `PLAYING`, `GAME_OVER`) shows how a game's
  screens fit together without a scene system.
- Paddles, ball and score use only mode 3 rectangles and text.
- Only what moves is redrawn: the old paddle and ball positions are erased
  before the new ones are drawn. A full-screen redraw every frame would be too
  slow in mode 3.
- Collision is rectangle overlap, a four-comparison function.
- `sound_beep`, `sound_tone` and `sound_noise` cover paddle hits, the serve and
  scoring.
- `save_read` and `save_write` keep `best_rally` in SRAM, with a magic number
  to detect an empty save.

**Try:** add a second ball, make the computer paddle miss more often as the
rally grows, or add a pause state.

## What the examples deliberately leave out

There is no engine, no scene manager, no asset pipeline beyond PNG conversion
and no music. Each example is a single `main.c` you can read top to bottom.
That is the point: copy the pattern you need into your own game, and replace
the rest.

When you outgrow one file, split the game into modules under `src/`; the build
picks up every `.c` file automatically. See
[Project Anatomy](03-project-anatomy.md).
