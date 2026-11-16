# 5. Collisions

Coins to collect, walls that stop you, enemies that hurt: almost every
gameplay rule starts with one question: do these two things touch?

## What you will learn

- The rectangle overlap test that most 2D games use.
- How to stop at a wall: move, test, undo.
- Why you test each axis separately.

## Boxes, not pixels

Comparing every pixel of two sprites would be far too slow. Instead, give
each object a rectangle (a **hitbox**): x, y, width and height. Two
rectangles that are lined up with the screen overlap exactly when they
overlap horizontally *and* vertically. Four comparisons:

```c
bool overlap(Box a, Box b) {
    return a.x < b.x + b.w && b.x < a.x + a.w
        && a.y < b.y + b.h && b.y < a.y + a.h;
}
```

This "axis-aligned bounding box" test is the classic one; the Pong example
that comes with GBADE uses it for the ball and the paddles. Hitboxes do not
have to match the picture exactly: a slightly smaller hitbox often feels
fairer to the player.

## Walls: move, test, undo

To stop at a wall, move first, then check. If the new position overlaps a
wall, go back to the old position. Do it **one axis at a time**: move
horizontally and check, then vertically and check. That way, pushing
diagonally into a wall still lets you slide along it.

```c
player.x += dx;
if (overlap(player, wall)) player.x -= dx;
player.y += dy;
if (overlap(player, wall)) player.y -= dy;
```

## Run the example

Click **▶ Try this example**. Move the white square with the arrows, collect
the yellow coins and slide along the grey walls. Each coin adds to the score
printed at the top and in the Console.

## Play with it

- Make the hitbox of the player 2 pixels smaller than its picture.
- Add a red square that moves on its own and resets the score when touched.
- Put the coin in a random place that is not inside a wall (test with
  `overlap` before accepting the position).

## Go further

- The **Pong** example (in the welcome screen) is a complete game built on
  this test: paddles, ball, score and sound.
- When a game has many objects, testing every pair gets slow.
  [Game Programming Patterns: Spatial Partition](https://gameprogrammingpatterns.com/spatial-partition.html)
  explains how games only test objects that are near each other.
