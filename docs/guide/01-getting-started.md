# Getting Started

GBADE is a development environment for the Game Boy Advance: an editor, a C
compiler and an emulator in one window. There is nothing to install around it:
the compiler (a copy of LLVM) and the emulator (mGBA) travel with the app.

This guide takes you from a fresh checkout to a running game, then explains
the command line for when you do not want a window.

## What you need

- A Mac with Apple silicon running macOS 12 or newer, or a Linux x86-64 PC
  with glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and newer).
- Rust 1.89 or newer, installed with [rustup](https://rustup.rs).
- On macOS, Xcode's command line tools, for `codesign`, `iconutil` and
  `hdiutil`: `xcode-select --install`.
- On Linux, CMake and the ALSA headers: `sudo apt install libasound2-dev
  pkg-config cmake`.

If you only want to *use* GBADE, you need none of this: download the DMG
(macOS) or the single executable (Linux) and see
[Platforms and Packaging](../platforms.md).

On Linux, every shortcut written ⌘ in these guides is Ctrl.

Building GBADE itself needs no other dependency. The games you write need
only `#include <gba.h>`, which comes from the SDK in this repository.

## Build the app

From the repository root:

```sh
cargo xtask fetch-llvm
cargo run -p gbade
```

`fetch-llvm` downloads the compiler and linker into `.cache/llvm`. It is a
large one-time download, and only the parts GBADE needs are kept: `clang`,
`ld.lld` and the compiler headers. `cargo run -p gbade` builds the IDE and
opens it.

> A plain `cargo run` also works because `gbade` is the default workspace
> member. The explicit `-p gbade` is only a reminder of what is being built.

The first build compiles the dependencies and can take a few minutes. Later
builds reuse the cache in `target/`.

## Create your first project

In the IDE, use **File > New Project…** (`Shift+Cmd+N`), pick a template and
a folder, and click **Create**.

Or from a terminal, with the same result:

```sh
cargo run -p gbade -- new ~/GBADE\ Projects/hello --template hello-world --name "Hello"
```

`gbade new` copies an example, renames the game and removes any build output
the template happened to contain.

## Run it

Press **Cmd+R**, or click the **Run** button. GBADE builds, starts the emulator
and shows the game on the right side of the window. The bottom panel prints the
game's `printf` output in the **Console** tab.

Useful things to try straight away:

- Change the text in `src/main.c`, press **Cmd+R** again and watch the game
  restart with your change.
- Click in the left margin of a source line to set a breakpoint, then run.
  The game stops there and the right side shows the CPU and memory.
- Open **Help > Learn GBA programming (guides)** to read the rest of these guides
  without leaving the app.

## Build without running

```sh
cargo run -p gbade -- build path/to/project
```

The ROM lands in `build/<game-name>.gba` inside the project. You can copy that
file to a flashcart or load it in any GBA emulator; it carries a valid
cartridge header.

## Run without a window

`gbade run` builds, runs headless and prints the game's console output. It is
handy for tests and for checking a change from a terminal:

```sh
cargo run -p gbade -- run path/to/project --frames 120
cargo run -p gbade -- run path/to/project --frames 300 --press a@60
cargo run -p gbade -- run path/to/project --screenshot shot.png
```

- `--frames N` how many frames to emulate (default 120, about two seconds).
- `--press KEY@FRAME` holds a GBA button for five frames, starting at that
  frame. `KEY` is one of `a b start select up down left right l r`. Repeat the
  option to press several buttons.
- `--screenshot FILE.png` saves the last frame.

The exit code is 0 on success, 1 when the build fails, 2 for bad arguments and
3 when the game panics or the emulator stops for another reason.

## Make a distributable app

```sh
cargo xtask bundle   # -> target/bundle/GBADE.app
cargo xtask dmg      # -> target/GBADE-0.1.0.dmg
```

`bundle` does a release build and copies the executable, compiler, SDK,
examples, docs and license files into `GBADE.app`. The DMG is about 80 MB;
almost all of it is the compiler.

### Installing from the DMG

Open the DMG and drag **GBADE** onto **Applications**. A DMG you built
yourself opens straight away.

A DMG that was **downloaded** (from GitHub, a chat app, a browser) is marked
by macOS as "from the internet". GBADE is signed ad hoc, not with a paid
Apple Developer ID, so macOS refuses to start it and may say the app "is
damaged" or "cannot be opened". After dragging it to Applications, remove
the mark once:

```sh
xattr -dr com.apple.quarantine /Applications/GBADE.app
```

This also covers the compiler inside the app, which macOS would otherwise
block when GBADE starts it. Signing with a Developer ID and notarising the
app would make this step unnecessary; that needs an Apple Developer account. `dmg` wraps the app in a
drag-to-Applications disk image. `cargo xtask smoke` then runs the bundled
command line on every example as a sanity check.

## Where to go next

- [The IDE](02-the-ide.md) for every panel, button and shortcut.
- [Project Anatomy](03-project-anatomy.md) for the folder layout and
  `gbade.toml`.
- [C on the GBA](04-c-on-the-gba.md) for the language and memory rules that
  matter on this console.
