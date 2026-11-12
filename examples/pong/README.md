# Pong
<!-- order: 5 -->

A complete little game: two paddles, a ball, a computer opponent, scores, sound effects and a saved high score. Read it to see how a whole game fits together.

## What you will learn

- Organising a game in states (title screen, playing, game over).
- Fixed-point maths for smooth movement without floating point.
- Collision detection between rectangles.
- Sound effects with the GBA's square-wave channels.
- Saving the best score in the cartridge's save memory (SRAM).

## Files

`src/main.c` is the whole game: state machine, paddles, ball, collision, sound
and the saved best rally. The functions are ordered roughly as the frame loop
uses them, so reading top to bottom follows one frame.

## Run it

```sh
cargo run -p gbade -- run examples/pong --frames 600
```

Or open the folder in GBADE and press Cmd+R. Move your paddle with the up and
down arrows; Enter starts a game. The best rally is stored in
`build/pong.sav`.

## Next

Read the [Examples tour](../../docs/guide/11-examples-tour.md) for the ideas
behind all five projects, then start your own with **File > New Project**.
