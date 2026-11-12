# C on the GBA

A GBA game is a normal C program with two unusual rules: the memory is small
and split into regions, and some "variables" are hardware registers that
change on their own. Everything else you already know applies.

This page covers what is different. It does not teach C.

## The machine in one paragraph

The CPU is an ARM7TDMI at 16.78 MHz, a 32-bit processor with 16-bit Thumb
instructions. The screen is 240x160 pixels and refreshes about 60 times per
second. The console has no operating system: your program starts at the
cartridge's entry point, gets a stack and a heap, and runs forever.

## Types come in exact sizes

`<gba.h>` provides the usual fixed-width names:

| Type | Size | Range |
|---|---|---|
| `u8` / `s8` | 1 byte | 0..255 / -128..127 |
| `u16` / `s16` | 2 bytes | 0..65535 / -32768..32767 |
| `u32` / `s32` | 4 bytes | 32-bit integer |
| `u64` / `s64` | 8 bytes | 64-bit integer |
| `bool` | 1 byte | `true` or `false` |

Use them for anything that touches hardware or a file format. Plain `int` is
32 bits here, which is fine for loop counters and arithmetic.

## Hardware registers are volatile

A register such as `REG_DISPCNT` is a fixed address, not a variable:

```c
#define REG_DISPCNT (*(vu16 *)0x04000000)
```

The `v` in `vu16` means `volatile`: the value can change without the compiler
writing to it, so every read and write must really happen. Without `volatile`
the optimiser may keep a copy in a register and your hardware writes silently
disappear. If you declare your own register or shared buffer that hardware
touches, make it `volatile` too.

## The memory map

| Region | Address | Size | What it is for |
|---|---|---|---|
| EWRAM | `0x02000000` | 256 KB | big buffers, the heap |
| IWRAM | `0x03000000` | 32 KB | globals, the stack, fast code |
| I/O | `0x04000000` | 1 KB | hardware registers |
| Palette | `0x05000000` | 1 KB | background and sprite colours |
| VRAM | `0x06000000` | 96 KB | tiles, maps, bitmaps |
| OAM | `0x07000000` | 1 KB | sprite attributes |
| ROM | `0x08000000` | 32 MB | your program and constant data |
| SRAM | `0x0E000000` | 32 KB | save memory |

IWRAM is fast but tiny. EWRAM is eight times bigger and slower. The linker
script reserves part of IWRAM for the stack, and `malloc` carves its heap out
of EWRAM.

## Telling the linker where things go

By default, code runs from ROM as Thumb and globals live in IWRAM. Two macros
override that:

```c
IWRAM_CODE void draw_particles(void) { ... }   /* fast ARM code in IWRAM */
EWRAM_BSS  u8 level_data[100000];              /* zeroed, in the big RAM */
EWRAM_DATA int scores[4] = { 1, 2, 3, 4 };     /* initialised, in the big RAM */
```

- `IWRAM_CODE` copies the function to IWRAM at startup and runs it as ARM
  code. Use it only for the few functions that really need speed: IWRAM is
  shared with globals and the stack.
- `EWRAM_BSS` and `EWRAM_DATA` move big arrays out of IWRAM. They cost a
  little speed and are the right home for level maps, sound buffers and
  anything measured in kilobytes.

Constant tables should be `const`. `const` data stays in ROM and costs no RAM
at all.

If the linker complains that a region overflowed, the `Problems` panel
suggests the fix: move arrays to EWRAM, make tables `const`, or remove
`IWRAM_CODE` from large functions.

## The shape of every game

```c
#include <gba.h>
#include <stdio.h>

int main(void) {
    text_init();                 /* set the screen up once */
    text_print("Hello!\n");

    while (1) {
        vsync();                 /* wait for the start of VBlank */
        key_poll();              /* read the buttons once per frame */

        /* update the game, then draw it */
        if (key_hit(KEY_A)) {
            printf("A was pressed\n");
        }
    }
}
```

`vsync()` is not a pause for its own sake. The screen is drawn line by line,
and memory cannot be changed safely while it is being read. VBlank is the
short gap between two frames, and waiting for it is how a game keeps its
timing and avoids tearing. `vsync()` also counts frames, so `frame_count()`
returns the number of frames since the game started.

`key_poll()` samples the buttons once per frame. Call it after `vsync()` and
before you read `key_hit`, `key_held` or `key_released`.

## Numbers without floating point

The ARM7TDMI has no floating-point unit. Floating point still works through
software routines, and the SDK includes them, but it is slow. Games usually
use fixed point instead:

```c
fixed x = INT_TO_FIX(100);   /* 100.0 */
fixed v = FIX(0.5);          /* 0.5, compile-time constant */
x += v;                      /* add */
x = fx_mul(x, FIX(2));       /* multiply */
int pixels = FIX_TO_INT(x);  /* back to an integer */
```

`fixed` is a 32-bit integer with 8 fractional bits, so one unit is 256. It
covers about -8 million to +8 million with a resolution of 1/256. That is
plenty for positions and speeds on a 240x160 screen.

## What the C library gives you

The SDK ships a small libc, not a full one:

- `<stdio.h>`: `printf`, `sprintf`, `snprintf`, `puts`, `putchar`. Output goes
  to the GBADE Console, not to the screen.
- `<string.h>`: `memcpy`, `memmove`, `memset`, `memcmp`, `strlen`, `strcmp`,
  `strcpy`, `strcat`, `strchr`, `strstr` and friends.
- `<stdlib.h>`: `malloc`, `free`, `calloc`, `realloc`, `rand`, `srand`,
  `atoi`, `strtol`, `qsort`, `abs`.
- `<ctype.h>`, `<assert.h>`, `<stdint.h>`, `<stdbool.h>` and a few others.

`printf` is the [Marco Paland](https://github.com/mpaland/printf)
implementation. It supports integer, string and floating-point conversions,
width and precision. `%f` works but is slow: it runs through the software
float routines, so prefer fixed point in a game loop.

There is no file system and no `fopen`. Persistent data goes to SRAM, see
[Saving the Game](09-saving.md).

## Two ways to get speed

- **Thumb** is the default. Instructions are 16 bits and the cartridge bus is
  16 bits, so Thumb code is compact and reads quickly from ROM.
- **ARM** instructions are 32 bits and faster per instruction but bigger.
  `IWRAM_CODE` copies one function to fast RAM and switches it to ARM for the
  hot loops that cannot keep up otherwise.

Measure before optimising: `profile_start()` and `profile_stop()` (from
`gba/timer.h`) count the CPU cycles a piece of code takes. One frame is about
280,000 cycles. The "ms/frame" in GBADE's status bar is how long your Mac
needs to emulate a frame, not how busy the GBA is.

## Where to go next

- [Graphics](05-graphics.md) for the screen modes and drawing.
- [Input](06-input.md) for buttons in detail.
- [Debugging](10-debugging.md) for printf, breakpoints and the inspector.
