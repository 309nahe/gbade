# Troubleshooting

Start here when something does not work. The IDE's **Problems** panel usually
has the answer first; this page explains the common cases in more detail.

## "Toolchain problem: clang not found" or "GBADE SDK not found"

The compiler lives in `.cache/llvm` in a checkout, or in the app bundle. If you
cloned the repository and never downloaded it:

```sh
cargo xtask fetch-llvm
```

GBADE looks for the toolchain in this order:

1. `$GBADE_TOOLCHAIN_DIR` and `$GBADE_SDK_DIR`, when set.
2. `GBADE.app/Contents/Resources/toolchain` and `.../sdk`.
3. The development checkout: `.cache/llvm` and `sdk/`.

The bundled compiler is a macOS arm64 build. On another operating system,
`fetch-llvm` and the build do not work as-is; you would need your own
`clang`/`ld.lld` that targets `arm-none-eabi`, pointed at with
`GBADE_TOOLCHAIN_DIR`, and that configuration is not supported yet.

## "no gbade.toml found"

The folder you opened is not a project, or you ran the command outside it.
`gbade build` and `gbade run` walk up from the given folder, so the simplest
fix is to pass the project folder explicitly:

```sh
gbade build path/to/my-game
```

If the project came from an example, make sure the whole folder was copied,
including `gbade.toml`.

## "No source files: add a .c file in the src/ folder"

The build found no `.c` or `.s` files under `src/`. Create `src/main.c` or
restore the file you moved. Files anywhere below `src/` count; files next to
the project root do not.

## "Undefined symbol: main"

Every program needs exactly one entry point:

```c
int main(void) { ... }
```

Check that the function is `main`, not `Main` or `start`, that it is not
`static`, and that it is in a `.c` file under `src/`.

## "Undefined symbol: something"

The linker cannot find a function or variable. The hint in the Problems panel
repeats the name. Common causes:

- A typo, or a name that differs in case.
- The function is declared in a header but defined nowhere.
- The definition is `static` in a different file, so it is private to that
  file.
- A `.c` file is outside `src/` and therefore not compiled.

## "Duplicate symbol"

The same function or global variable is defined in two `.c` files. Keep one
definition and put a declaration in a header for the other file:

```c
/* game.h */
extern int score;                 /* declaration */

/* game.c */
int score;                        /* definition, once */
```

## "Section will not fit in region 'iwram'"

The fast 32 KB of RAM is full. Globals, the stack and `IWRAM_CODE` functions
all live there. Fixes, in order of effort:

- Make constant tables `const` so they stay in ROM.
- Move big arrays to EWRAM with `EWRAM_BSS` or `EWRAM_DATA`.
- Remove `IWRAM_CODE` from large functions.

## "Section will not fit in region 'ewram'"

The 256 KB of external RAM is full. Make constant data `const`, free buffers
you no longer need, or shrink fixed-size arrays. `malloc` also takes its memory
from EWRAM.

## "Implicit declaration of function" or "call to undeclared function"

The compiler has not seen the function before this line. Add the right header
(`<gba.h>`, `<stdio.h>`, `<string.h>`, ...) or declare the function above its
first use. For your own functions, put a prototype in a header in `include/`.

## "Use of undeclared identifier"

A variable or constant does not exist in this scope. C is case-sensitive;
check the spelling, that the declaration comes before the use, and that the
name is not inside another block.

## "Expected ';' before ..."

A statement is missing a semicolon. The reported line is often the one *after*
the mistake; look at the end of the line above it.

## "File not found" for a header

The include cannot be resolved. GBADE provides `<gba.h>`, `<stdio.h>`,
`<stdlib.h>`, `<string.h>`, `<stdint.h>`, `<stdbool.h>` and a few more. Your
own headers go in `include/` or next to the `.c` file. Generated asset headers
are included as `#include "assets.h"` and exist only after a build.

## "Incompatible pointer" warnings

A pointer and an integer, or two different pointer types, are being mixed. On
the GBA, hardware addresses are cast explicitly:

```c
vu16 *reg = (vu16 *)0x04000000;
```

## The game runs but the screen is blank

Check, in this order:

1. `REG_DISPCNT` has a mode bit and the layer's enable bit. The inspector's
   I/O tab shows the decoded value.
2. The right memory is loaded: palette, tiles and map for a background; palette
   and tiles for sprites.
3. Tile and map block numbers match between the copy (`tile_mem[n]`,
   `se_mem[n]`) and the control register (`BG_CBB(n)`, `BG_SBB(n)`).
4. For sprites, `DCNT_OBJ | DCNT_OBJ_1D` is set and `oam_update()` runs after
   `vsync()`.
5. Nothing overwrote VRAM later. The tile viewer shows what is really there.

## The game flickers or tears

Memory is being changed while the screen is drawing. Move VRAM, palette and OAM
updates after `vsync()`, and call `oam_update()` there too. For bitmap modes,
draw into the hidden page and flip (mode 4) instead of drawing into the visible
page.

## The emulator stops with a reason

A panic, an illegal instruction or a bad memory access stopped the game. The
Console shows the reason and often the address. Open the CPU tab: the program
counter and disassembly point at the fault. Assertions (`ASSERT`) and
`debug_panic` also stop the game this way, with the file and line.

## "Documentation not found" in the Learn panel

The `docs/` folder is missing next to the executable or the checkout. In a
development checkout it is at the repository root; in the app bundle it is
`Contents/Resources/docs`. Restore the folder, or set `GBADE_RESOURCES_DIR` to
a directory that contains `docs/`.

## The build says a doc or asset is missing during bundling

`cargo xtask bundle` copies `sdk/`, `examples/`, `docs/` and `licenses/` into
the app. All four folders must exist. A missing `docs/` or `licenses/` folder
stops the bundle; restore it from git rather than creating an empty one.

## Builds are slow the first time

The first build compiles the GBADE runtime (startup code, libc, soft-float
helpers) and stores it in the cache. Later builds reuse it. If the cache is
cold, or `GBADE_CACHE_DIR` points somewhere new, expect the first build to take
longer; the Build log says when the runtime was compiled.

## Nothing here matches

Read the **Build log** for the exact command and the compiler's raw output.
Then check the source line the Problems panel points at, not the line you
suspect. If it still looks impossible, reduce the project to the smallest file
that reproduces it.
