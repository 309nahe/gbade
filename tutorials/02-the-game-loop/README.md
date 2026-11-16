# 2. The Game Loop

Every game is a loop: read the buttons, move things, draw, wait, repeat. On
the GBA the "wait" part is set by the screen itself.

## What you will learn

- Why a GBA game runs one loop turn per screen refresh (a **frame**).
- What **VBlank** is and why you update the picture during it.
- How to make things move at a steady speed by counting frames.

## Input, update, render

Game Programming Patterns describes the game loop as three steps run over and
over: process input, update the game, render it. One turn of the loop is a
frame. On a PC the loop has to measure time itself; on the GBA the screen
gives the beat.

## The screen draws itself, line by line

The GBA's LCD refreshes about 59.73 times a second. It draws the 160 visible
lines one after another (this is called VDraw), then waits for the length of
68 more lines before starting again (VBlank). If you change the picture while
the screen is drawing it, the top half can show the old picture and the
bottom half the new one: that is called **tearing**. So games wait for VBlank,
then update the screen.

`vsync()` does exactly that: the CPU sleeps until the next VBlank begins.
Calling it once at the top of the loop gives you a loop that turns about 60
times a second, every second, on every GBA.

```c
while (1) {
    vsync();        // wait for the next frame
    key_poll();     // 1. input
    x += 1;         // 2. update
    draw(x);        // 3. render
}
```

## Speed is pixels per frame

Because the loop runs at a fixed rate, speeds are written "per frame": moving
1 pixel each frame crosses the 240-pixel screen in 4 seconds. `frame_count()`
tells you how many frames have passed since the start, which is also a clock:
60 frames is one second.

## Run the example

Click **▶ Try this example**. A square slides across the screen and wraps
around; the seconds are printed at the top, and the frame count goes to the
Console.

## Only redraw what changed

There is a limit to what fits in one frame. Tonc notes that you cannot build
up a whole screen of individual pixels every frame: there are too many. The
example redraws the text only when the number of seconds changes. Try
redrawing it every frame: the end of the line starts to flicker, because the
screen shows it before the program has finished writing it.

## Play with it

- Change the speed to 2, 3, then 0 pixels per frame.
- Make the square bounce instead of wrapping: flip the direction at the edges.
- Call `vsync()` twice in the loop: the game now runs at 30 frames per
  second, so the square moves at half speed.

## Go further

- [Tonc: Video Introduction](https://gbadev.net/tonc/video.html) has the exact
  timings of VDraw, HBlank and VBlank, and several ways to wait for VBlank.
- [Game Programming Patterns: Game Loop](https://gameprogrammingpatterns.com/game-loop.html)
  by Robert Nystrom explains the pattern and how PC games handle variable
  speeds.
