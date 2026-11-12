# Project Anatomy

A GBADE project is an ordinary folder. Nothing is hidden in a database and
nothing needs a daemon: the folder contains your code, your images and one
small settings file.

```text
my-game/
├── gbade.toml        project settings: name, ROM header, compiler flags
├── src/              C and assembly sources, in subfolders too
│   └── main.c
├── include/          optional headers of your own
├── assets/           optional PNG images, converted at every build
│   └── backgrounds/  large images for tiled backgrounds
└── build/            created by GBADE: objects, ELF, ROM, save file
```

Only `gbade.toml` and `src/` are required. The other folders appear when you
need them.

## gbade.toml

```toml
[game]
name = "My Game"
title = "MYGAME"
code = "GBDE"
maker = "01"
version = 0

[build]
optimization = "O2"
cflags = []
defines = []
werror = false
```

Every key is optional; GBADE fills in the defaults shown above.

### The [game] section

| Key | Meaning |
|---|---|
| `name` | Human name. Also used for the output files, slugified: "My Game" becomes `my-game.gba`. |
| `title` | Up to 12 characters in the cartridge header, shown by some emulators and tools. |
| `code` | Exactly four characters: the game code in the header. |
| `maker` | Exactly two characters: the maker code. |
| `version` | One byte, the software version in the header. |

`gbade new` sets `title` from the name for you, in uppercase.

### The [build] section

| Key | Meaning |
|---|---|
| `optimization` | `O0`, `O1`, `O2`, `O3`, `Os` or `Oz`. Default `O2`. |
| `cflags` | Extra flags for every C file, for example `["-Wextra"]`. |
| `defines` | Preprocessor definitions, for example `["DEBUG=1"]`. |
| `werror` | Treat warnings as errors. |

`Os` and `Oz` optimise for size, which matters when a ROM or the 32 KB of
fast RAM gets tight. `O0` is easier to follow in the debugger.

## src/

All `.c` files and `.s` or `.S` files under `src/`, at any depth, are compiled
and linked together. There is no makefile and no list to maintain: add a file,
press Build, and it is part of the program.

The only requirement is one `int main(void)`. It never returns: the game runs
a loop forever. See [C on the GBA](04-c-on-the-gba.md).

## include/

Headers in `include/` are on the compiler's include path, so
`#include "mygame.h"` works from any source file. You can also keep headers
next to their `.c` file; that works too.

## assets/

Every `.png` under `assets/` becomes C arrays in `build/gen/assets.c` and
`build/gen/assets.h`, regenerated on each build. Include the generated header
with:

```c
#include "assets.h"
```

The file name decides the symbol names:

- `assets/hero.png` produces `hero_pal`, `hero_tiles`, `hero_bitmap` (if the
  image is at most 240x160) and the constants `HERO_WIDTH`, `HERO_HEIGHT`,
  `HERO_TILE_COUNT`.
- `assets/hero.16x16.png` also sets the frame size. The image is a strip of
  16x16 frames, and frame `n` starts at tile `n * HERO_FRAME_TILES`. This is
  what sprite animation needs.
- `assets/backgrounds/world.png` is treated as a tile map: GBADE de-duplicates
  the 8x8 tiles and produces `world_tiles` plus `world_map`.

Pixels that are fully transparent, and pure magenta `(255, 0, 255)`, become
colour index 0, which the hardware never draws. See
[Assets Reference](../assets.md) for the exact rules and every generated
symbol.

## build/

Created on the first build. It contains:

- `build/<name>.gba` the ROM you can run anywhere.
- `build/<name>.elf` the linked program with debug information, used by the
  breakpoints.
- `build/<name>.sav` the game's save memory, kept between runs.
- `build/gen/assets.c` and `build/gen/assets.h` the converted images.
- `build/obj/` object files and dependency files.

`build/` is safe to delete: the next build recreates it. Do not put anything
of your own there, because a clean build may remove it.

## What is not in the folder

The compiler, the SDK headers and the emulator are not copied into each
project. GBADE finds them once, in the app bundle or in this repository, and
every project uses the same ones. That is why a project is small enough to
email or commit to git.

## Environment variables

These are mostly for development and for continuous integration:

| Variable | Effect |
|---|---|
| `GBADE_TOOLCHAIN_DIR` | Use this folder instead of the bundled compiler. It must contain `bin/clang` and `bin/ld.lld`. |
| `GBADE_SDK_DIR` | Use this `sdk/` folder instead of the bundled one. |
| `GBADE_CACHE_DIR` | Where the compiled runtime is cached. |
| `GBADE_RESOURCES_DIR` | Where to find `examples/` and `docs/`. |

## Version control

Commit `gbade.toml`, `src/`, `include/` and `assets/`. Ignore `build/`: it is
output. `gbade new` does not create a `.gitignore` for you, so add one
line yourself if you use git:

```text
build/
```
