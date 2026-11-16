# 9. Game States and Text

A game is more than its gameplay: a title screen, the game itself, a pause, a
"game over" with the score. Organising these as **states** keeps the code
simple, and the text console shows the words.

## What you will learn

- The finite state machine: a small, standard way to organise screens.
- Entering and leaving a state cleanly.
- Writing text and numbers on the screen with the tile text console.

## One state at a time

Game Programming Patterns describes the finite state machine like this: a
fixed set of states, the machine is in exactly one of them at a time, and
inputs cause transitions from one state to another. For a whole game:

- **Title**: wait for Start, then go to Play.
- **Play**: run the game; when time runs out, go to Game Over.
- **Game Over**: show the score; Start goes back to Title.

In C, an `enum` names the states and a `switch` in the main loop runs the
current one:

```c
typedef enum { TITLE, PLAY, GAME_OVER } State;
State state = TITLE;

while (1) {
    vsync();
    key_poll();
    switch (state) {
    case TITLE: if (key_hit(KEY_START)) enter(PLAY); break;
    case PLAY:  ...
    }
}
```

A small `enter(new_state)` function is the place to set up a state: clear the
screen, reset the score, start a timer. Each state's code then only cares
about itself.

## Text on a tiled screen

There is no "print" on a console: text is graphics. `text_init()` turns
background 0 into a 30x20 grid of 8x8 letters (mode 0), and then:

```c
text_print_at(9, 8, "PRESS START");   // column, row
text_goto(2, 2);
text_printf("Score: %d", score);
text_clear();
```

Text is tiles, so changing it costs almost nothing: perfect for a score or a
timer that changes every frame.

## Run the example

Click **▶ Try this example**. Press **Enter** (Start), then hit **X** (A) as
many times as you can in 10 seconds. Your best score is kept until you reset.

## Play with it

- Add a **Pause** state: Start during Play pauses, Start again resumes.
- Show "NEW BEST!" on the Game Over screen when the record is beaten.
- Make the Title screen blink "PRESS START" every half second using
  `frame_count()`.

## Go further

- [Game Programming Patterns: State](https://gameprogrammingpatterns.com/state.html)
  by Robert Nystrom: finite state machines, and how they grow.
- [Tonc: Text systems](https://gbadev.net/tonc/text.html) explains how text
  works on tiled backgrounds, bitmaps and sprites.
- The **Pong** example is organised with exactly these three states:
  `TITLE`, `PLAYING` and `GAME_OVER`.
