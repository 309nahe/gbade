# Hello World
<!-- order: 1 -->

Your first GBA program: text on screen, printf to the Console, and reading the buttons. Start here.

## What you will learn

- The structure of every GBA game: set things up, then loop forever once per frame.
- `vsync()` and `key_poll()`, the two calls at the top of every frame.
- Printing on the GBA screen (`text_print`) versus printing to GBADE's Console (`printf`).

## Things to try

1. Change the colours: `text_set_background(RGB8(120, 0, 0))`.
2. Print how many frames have passed: `text_printf("%d\n", frame_count());`.
3. Make B clear the screen too.
