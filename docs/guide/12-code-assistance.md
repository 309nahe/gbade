# Code Assistance

While you type, GBADE checks your code and helps you write it: errors appear
before you build, names complete themselves, and you can ask what any
function does. This is powered by **clangd**, the C "language server" from
the LLVM project, which ships inside GBADE next to the compiler.

## Errors while you type

Make a mistake and, about a quarter of a second after you stop typing, the
line turns red and the margin shows a marker, exactly like after a build:

```c
int presses = 0     // <- missing ';': red right away
```

Hover the line to read the message and, for common mistakes, a hint in plain
English. The **Problems** panel updates too. You do not need to build to see
these: GBADE checks the files you have open, with the same compiler settings
as a real build.

Some problems can only be found when the whole game is linked: a function
that is declared but defined nowhere, or memory that overflows. Those still
appear after a build, and stay listed until the next one.

## Completion

Type two letters of a name and a list of matching functions, macros,
variables and types appears under the cursor:

```text
key_h|
  key_held(u16 keys)        bool
  key_held_mask()           u16
  key_hit(u16 keys)         bool
```

| Key | Action |
|---|---|
| Up / Down | choose |
| Enter or Tab | insert the chosen name |
| Esc | close the list |
| Ctrl+Space | open the list yourself, anywhere |

Typing `.` or `->` after a struct variable lists its fields. The list knows
everything in `<gba.h>`, the C library, your own files and the generated
`assets.h`.

## What is this?

Rest the mouse on any name for a moment: a tooltip shows its declaration and
the comment written above it. For the SDK this is a mini manual, since every
function in `gba.h` is documented:

```text
void vsync(void)
Wait until the next VBlank. Uses the BIOS so the CPU sleeps...
```

## Go to definition

To see where something comes from, ⌘-click it (Ctrl-click on Linux), or put
the cursor on it and press **F12**. GBADE opens the file at the definition:
your own function, or the SDK header where a register or helper is defined.
SDK headers open **read-only**, so you cannot break the SDK by accident; read
them, they are written to be learned from.

## From the terminal

`gbade check` runs the same checks on every C file of a project, without
building, and prints the problems with their hints:

```sh
gbade check my-game
main.c:2:25: error: use of undeclared identifier 'y'
  hint: This name does not exist here. Check the spelling (C is case-sensitive)...
checked 1 file(s): 1 error(s), 0 warning(s)
```

It exits with code 1 when there are errors, so it works in scripts.

## How it works

clangd needs to know how each file is compiled: the GBA target, the SDK's
include folder, your defines. GBADE writes this into
`build/compile_commands.json` (the standard format every C tool understands),
from the very same list of flags the build uses, so the editor and the build
never disagree. It is refreshed when the project opens and at every build;
change `gbade.toml` and press Build to update it.

Because `compile_commands.json` is standard, other editors (VS Code, Neovim,
Zed, CLion) pick it up too if you open the project folder in them.

## When it does not work

- **No red lines, no completion**: the status bar says why when clangd could
  not start. Developers building GBADE from source need `cargo xtask
  fetch-llvm` to download clangd.
- **"file not found" for your own header**: put headers in `include/` or next
  to your `.c` files, then build once so `compile_commands.json` is fresh.
- To see what clangd does, start GBADE with `GBADE_CLANGD_LOG=/tmp/clangd.log`
  set: its detailed log is written to that file.
