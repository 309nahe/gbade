# Debugging

Three tools, in the order you will reach for them: print statements, the
problems panel, and the debugger. GBADE is designed so that none of them
requires leaving the app.

## Print statements

`printf` goes to the IDE's **Console**, not to the game screen:

```c
#include <stdio.h>

printf("player at %d,%d\n", x, y);
```

In a build without the emulator's debug device, such as a real cartridge, the
output is simply ignored. Leave debug prints in your code without guilt; they
cost a few cycles and nothing else.

For levels and colours, use `debug_log`:

```c
#include <gba.h>

debug_log(LOG_INFO, "level %d loaded", level);
debug_log(LOG_WARN, "low health: %d", hp);
```

| Level | Meaning |
|---|---|
| `LOG_FATAL` | the game cannot continue |
| `LOG_ERROR` | something failed |
| `LOG_WARN` | suspicious but survivable |
| `LOG_INFO` | normal information |
| `LOG_DEBUG` | noisy detail |

`debug_available()` tells you whether anyone is listening (true in GBADE,
false on hardware). `debug_puts` prints a string with no formatting.

The Console tab separates game output from the emulator's own warnings. Turn
**View > Emulator warnings in Console** off when a game reads unused memory on
purpose and the noise gets in the way.

## Assertions and panics

```c
ASSERT(index < MAX_ITEMS);
```

If the condition is false, the game stops, prints the file and line, and the
IDE highlights the line. In the Console, the panic message and
`BREAKPOINT()` hits are clickable.

`debug_panic(file, line, message)` does the same with a message of your own.
`ASSERT` is the convenient form and works from any source file.

## Breakpoints

Click in the grey margin left of a line number. A red dot appears and the game
will stop there on its next pass.

When it stops:

- The inspector switches to **CPU** and shows registers, flags and the
  disassembly at the program counter.
- The editor highlights the source line.
- **F5** continues, **F10** steps one line, **F2** and **F4** save and load a
  state.

Breakpoints need debug information, which the build produces automatically:
the linked `.elf` maps addresses to source lines. A breakpoint in a header or
in code that was inlined may land on the nearest real line.

`BREAKPOINT()` is the in-code version:

```c
BREAKPOINT();    /* pause here, like a breakpoint */
```

It is ignored on real hardware. For precise stops, prefer a gutter breakpoint:
it uses a hardware address and does not disturb the code.

## The problems panel

Every error and warning from clang and the linker lands in **Problems**, with
a one-line hint for mistakes beginners make. Click a message to jump to it.

The hints cover the classics: missing `main`, undefined and duplicate symbols,
IWRAM or EWRAM overflow, missing includes, a forgotten semicolon, a pointer
mixed up with an integer, and a non-void function that can reach its end.

If the panel says nothing, open the **Build log**. The raw log shows the
commands and the compiler's unparsed output.

## When the game stops by itself

A crash, an illegal instruction or an emulator-detected bad access stops the
game and prints a reason in the Console, for example the address that could
not be read. Look at the CPU tab at that moment: the program counter, the
stack pointer and the disassembly usually point at the cause.

The most common causes on the GBA:

- Running out of stack or heap. `malloc` comes from EWRAM; the stack lives in
  IWRAM.
- An out-of-bounds array write that overwrites a variable or a return address.
- An `ASSERT` or `debug_panic` that fired.
- An illegal instruction, for example after a corrupted return address.

## When the picture is wrong but the game keeps running

Some mistakes produce a visual glitch, not a stop. Check these before hunting
for a crash:

- **Tearing or flicker**: VRAM or OAM is written while the screen is drawing.
  Move the write after `vsync()` and call `oam_update()` there.
- **Stray sprites at the top-left corner**: `oam_update()` runs before
  `oam_init()`, so the zeroed shadow OAM shows 128 sprites at `(0, 0)`.
  Call `oam_init()` first.
- **Wrong or cut-off sprite graphics**: the `SpriteSize` and the first tile
  index do not match the loaded sheet.
- **A sheared tiled background**: a map narrower than 256 pixels was copied in
  one `memcpy`; copy it row by row into 32-entry screen-block rows. See
  [Assets](../assets.md) for the loop.

## Inspecting the machine

The hardware inspector answers "what is the hardware actually doing?" while
paused or running:

- **CPU**: registers, current mode (ARM or Thumb), flags, disassembly.
- **I/O**: decoded `DISPCNT`, `BG0CNT`, `KEYINPUT` and friends, with a
  one-line explanation of each.
- **Palettes**, **Tiles**, **Sprites**: the actual memory, drawn.
- **Memory**: hex view of every region, with "go to" for an address.

If a background is invisible, check `DISPCNT` first: the mode bit and the
background's enable bit are the usual culprits.

## Checking without a window

`gbade run` is a small test harness:

```sh
gbade run path/to/project --frames 120
gbade run path/to/project --frames 300 --press a@60 --screenshot out.png
```

It prints the game's console output and returns a non-zero exit code when the
build fails, the game panics or the emulator stops. This is how the repository's
end-to-end tests check examples: build, run a number of frames, assert on the
printed lines and the pixels.

## A debugging routine that works

1. Reproduce with `printf`: the values you think are true rarely are.
2. Add `ASSERT` where an invariant must hold; let the game tell you the line.
3. Set a breakpoint just before the bad value and step until it appears.
4. Read the hardware state in the inspector, not from memory.
5. Keep the smallest failing example; then fix it.
