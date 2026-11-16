# 3. Buttons

A game is something you play: let's read the buttons. The GBA has ten of
them, and all ten are bits in a single hardware register.

## What you will learn

- How the GBA reports its buttons (`REG_KEYINPUT`).
- The difference between a button being **held** and being **hit**.
- A neat trick for directions: values of -1, 0 or +1.

## Ten bits, upside down

The D-pad (4 directions), A, B, Select, Start, L and R are bits 0 to 9 of
`REG_KEYINPUT`. The bits are **active-low**: a bit is 0 while its button is
pressed and 1 when it is not, so with no button pressed the register reads
`0x03FF`. Tonc warns about this because it trips everyone up once.

`key_poll()` reads the register once per frame and flips it, so that in the
rest of your code 1 means "pressed". Call it right after `vsync()`.

## Held, hit and released

- `key_held(KEY_A)`: A is down right now. Good for walking or charging.
- `key_hit(KEY_A)`: A went down this frame (it was up last frame). Good for
  jumping, shooting, opening a menu: one action per press.
- `key_released(KEY_A)`: A went up this frame.

`key_poll()` keeps the previous frame's state to answer these questions.
Using `key_held` for a pause button is a classic bug: holding Start toggles
pause on and off every frame.

## Directions as -1, 0, +1

`key_tri_horz()` returns -1 when Left is held, +1 for Right and 0 for neither
(or both). Moving becomes one line, with no `if`:

```c
x += key_tri_horz() * speed;
y += key_tri_vert() * speed;
```

## Run the example

Click **▶ Try this example**, then click the game screen so it gets the
keyboard. The arrow keys move the square, **X** (the GBA's A button) changes
its colour, holding **Z** (B) makes it faster. The raw register value is
shown at the top: watch the bits change.

## Play with it

- Make L and R change the square's size.
- Count how many times A was hit and print it with `m3_printf`.
- Replace `key_hit` with `key_held` for the colour change and see why `hit`
  exists.

## Go further

- [Tonc: The GBA keypad](https://gbadev.net/tonc/keys.html) explains the
  register, transitional states and tribools; GBADE's input functions follow
  the same design.
- The **Input** guide in GBADE's Learn panel lists every key function.
