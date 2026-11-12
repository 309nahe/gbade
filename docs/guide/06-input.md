# Input

The GBA has ten buttons: A, B, Select, Start, the four directions, L and R.
Reading them takes two lines: poll once per frame, then ask.

```c
while (1) {
    vsync();
    key_poll();

    if (key_held(KEY_LEFT))  x -= 2;
    if (key_held(KEY_RIGHT)) x += 2;
    if (key_hit(KEY_A))      jump();
}
```

## The three questions

| Function | Answers | Use for |
|---|---|---|
| `key_held(mask)` | is any of these down right now? | movement, holding fire |
| `key_hit(mask)` | did any of these go down this frame? | jumps, menu confirm, one-shot actions |
| `key_released(mask)` | did any of these go up this frame? | charge shots, menu release |

The difference matters. `key_held(KEY_A)` is true for as long as the button is
down, so a counter incremented on `key_held` grows about 60 times a second.
`key_hit(KEY_A)` is true only on the frame the press happened, which is what
"press A to start" means.

`key_poll()` must be called exactly once per frame, after `vsync()`. It copies
the current state into `keys_now` and shifts the previous one into
`keys_prev`, which is how `key_hit` and `key_released` can compare the two.
Calling it twice in one frame loses presses; not calling it freezes all input.

## Masks

Every button has a bit, and you combine them with `|`:

| Button | Constant | Button | Constant |
|---|---|---|---|
| A | `KEY_A` | D-pad right | `KEY_RIGHT` |
| B | `KEY_B` | D-pad left | `KEY_LEFT` |
| Select | `KEY_SELECT` | D-pad up | `KEY_UP` |
| Start | `KEY_START` | D-pad down | `KEY_DOWN` |
| L | `KEY_L` | R | `KEY_R` |

`KEY_ANY` is every button, `KEY_DPAD` is the four directions. Any of these
works with all three questions:

```c
if (key_hit(KEY_A | KEY_START)) { /* either one was just pressed */ }
```

A mask is an "any", not an "all": `key_held(KEY_A | KEY_B)` is true when A
alone is down. If you need both, ask twice.

## Movement without stutter

The D-pad can only be fully on. For smooth movement, the two triangle helpers
turn it into a direction:

```c
int dx = key_tri_horz();   /* -1 left, +1 right, 0 otherwise */
int dy = key_tri_vert();   /* -1 up,   +1 down,  0 otherwise */

x = CLAMP(x + dx * speed, 0, SCREEN_WIDTH - width);
y = CLAMP(y + dy * speed, 0, SCREEN_HEIGHT - height);
```

When left and right are pressed together, the helpers return 0, because the
two bits cancel. That is a reasonable default; if you want a diagonal to be
the same length as a straight line, normalise or use a fixed-point vector.

## The raw register

`REG_KEYINPUT` holds one bit per button, and a bit is **0 when the button is
pressed**. That inversion is the single most common surprise in GBA code:

```c
u16 held = ~REG_KEYINPUT & KEY_ANY;   /* now 1 means pressed */
```

`key_poll()` does exactly this for you. Read the register directly only if you
need non-blocking or interrupt-driven input; `gba/irq.h` defines
`IRQ_KEYPAD`, which fires when a button is pressed or released.

## Edge cases worth knowing

- **Menus**: use `key_hit` plus a repeat timer if you want held-direction
  scrolling to repeat after a pause.
- **Typing in the editor**: while a text field has focus, the IDE stops
  sending keys to the game, so typing in the editor never moves the player.
- **Pause on break**: when the game stops at a breakpoint or a panic, input
  stops too.
- **Combinations**: `KEY_DPAD` with `key_hit` tells you "the player touched
  the pad", which is often enough for a title screen.

## Trying it

Run the [Hello World](../../examples/hello-world/README.md) example and press
buttons; every press is echoed on screen and in the Console panel. Then open
[Drawing](../../examples/drawing/README.md) to see movement with `key_tri_horz`
and `key_tri_vert`.
