# 8. Scrolling and a Camera

Your levels can be bigger than the screen. The GBA scrolls tiled backgrounds
in hardware: you write two numbers, and the whole background moves.

## What you will learn

- The scroll registers `REG_BG0HOFS` and `REG_BG0VOFS`.
- Bigger maps: 64x32 tiles is a 512x256-pixel world.
- A camera that follows the player, and world versus screen coordinates.

## Two numbers move the world

`REG_BG0HOFS` and `REG_BG0VOFS` say which point of the map appears in the
top-left corner of the screen. Increase the horizontal offset and the map
slides **left**, as if the camera moved right. Two things to know, both
pointed out by Tonc:

- The registers are **write-only**: reading them back does not work, so keep
  the scroll position in your own variables.
- Regular backgrounds **wrap around**: scrolling past the right edge of the
  map shows its left edge again.

## Bigger maps

`BG_REG_64x32` makes a map 64 tiles wide and 32 tall (512x256 pixels). It
uses two screenblocks side by side: the left 32x32 half in the first, the
right half in the next one. So cell (x, y) is in screenblock `base + x / 32`,
at `y * 32 + x % 32`.

## A camera

Keep everything in **world coordinates** (pixels in the level). The camera
is the world position of the screen's top-left corner: centre it on the
player, then clamp it so it never shows outside the level. Draw things at
`world position - camera`:

```c
cam_x = CLAMP(player_x - 120, 0, WORLD_W - 240);
REG_BG0HOFS = cam_x;
sprite_move(0, player_x - cam_x, player_y - cam_y);
```

## Run the example

Click **▶ Try this example** and walk with the arrows. The level is twice as
wide as the screen; the camera follows the player and stops at the edges.

## Play with it

- Remove the `CLAMP` and see the map wrap around.
- Make the camera lag behind: move it by a quarter of the distance to its
  target every frame.
- Add a second background (BG1) for distant hills that scrolls at half
  speed: that is parallax.

## Go further

- [Tonc: Regular tiled backgrounds](https://gbadev.net/tonc/regbg.html)
  explains the scroll registers and the layout of bigger maps.
- The **Scrolling World** example scrolls a background converted from a PNG.
