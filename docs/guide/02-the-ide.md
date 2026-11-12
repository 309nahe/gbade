# The IDE

The GBADE window is built around one idea: you should never have to leave it
to see why your game does not work. The editor, the compiler, the emulator and
the debugger share the same screen.

This page describes what each part does. All keyboard shortcuts use Cmd
because GBADE currently ships as a macOS app.

## The main areas

From top to bottom and left to right:

- **Menu bar**: File, Game, View and Help.
- **Toolbar**: Run, Build, Pause/Continue, Step line, Frame, Reset, Stop, the
  emulation speed menu and the Learn button. On the right: how much ROM,
  IWRAM and EWRAM the last build uses (hover a bar for details).
- **File list (left)**: every file in `src/`, `include/` and `assets/`, plus
  `gbade.toml`. Folders can be expanded. **+ New file** creates a file,
  **Finder** opens the project folder; right-click a file to delete it or
  show it in Finder. Clicking a PNG opens it in your image viewer.
- **Editor (centre)**: tabs for open files, line numbers and syntax
  highlighting. The left margin is where breakpoints go.
- **Game screen (right)**: the emulator, or a "Press ▶ Run" hint when nothing
  is running. When the editor has the keyboard, the screen shows "Click here
  to play": click it to send the keys to the game.
- **Hardware inspector (right, under the game screen; View menu to hide)**:
  CPU, I/O registers, palettes, tiles, sprites and memory.
- **Bottom panel**: Problems, Console and Build log.
- **Status bar (very bottom)**: the last message, fps, frame number, the time
  the emulator takes per frame, the sound state and whether the compiler was
  found.
- **Learn panel**: opens in the centre when you choose a guide.

Drag the dividers to resize any of these. The last project reopens on the
next launch.

## Menus

### File

- **New Project…** (`Shift+Cmd+N`) create a project from an example.
- **Open Project…** (`Cmd+O`) open a folder that contains `gbade.toml`.
- **Open Recent** the last ten projects.
- **New File…** (`Cmd+N`) create a file inside the project.
- **Save All** (`Cmd+S`) save every modified tab.
- **Export ROM (.gba)…** copy the built ROM somewhere else.
- **Show Project in Finder** open the project folder.
- **Settings…** colours, font size, screen scale, volume, key mapping,
  projects folder and more.

### Game

- **Build and Run** (`Cmd+R`) build, then start or restart the game.
- **Build Only** (`Cmd+B`) build without running.
- **Pause / Continue** (`F5`).
- **Step One Line** (`F10`) run the next source line and pause.
- **Reset** (`Shift+Cmd+R`) restart the game from the beginning.
- **Stop** (`Cmd+.`) stop the emulator and free its audio device.
- **Save State** (`F2`) and **Load State** (`F4`) snapshot the whole machine.
- **Save Screenshot** write the current frame to a PNG.
- **Remove All Breakpoints** clear every breakpoint at once.

### View

- **Dark mode** switch the theme.
- **Hardware inspector** show or hide the right-hand panel.
- **Emulator warnings in Console** show mGBA's warnings about bad memory
  accesses and similar hardware misuse.
- **Bigger editor font** (`Cmd++`) and **Smaller editor font** (`Cmd+-`).

### Help

- **Learn GBA programming (guides)** opens this documentation.
- **Tonc tutorial (website)** the classic external GBA tutorial.
- **About GBADE & credits** version, the list of projects GBADE builds on,
  and a link to [Credits and licenses](../CREDITS.md).

## The bottom panel

### Problems

Every compiler and linker message, extracted from clang's output. Each row
shows the file, line and message, plus a short hint for common mistakes. Click
a row to jump to the line in the editor. The tab title counts errors and
warnings.

### Console

The game's own output: everything it prints with `printf`, `puts` or
`debug_log`, plus, when enabled, mGBA's warnings. Game output and emulator
output are labelled. **Clear** empties it and **Game output only** hides the
emulator's own messages.

Panic messages such as `GBADE:PANIC` are clickable and jump to the source
line, and so are `BREAKPOINT()` hits.

### Build log

The raw log of the last build: which files were compiled, the link command,
memory usage, and anything the compiler printed that could not be parsed into
a problem.

## The hardware inspector

The inspector reads the live emulator. It is the fastest way to answer
questions like "is my background enabled?" without adding print statements.

| Tab | Shows |
|---|---|
| CPU | registers, current mode, flags, disassembly, current source line |
| I/O | the important hardware registers with decoded bits and a one-line explanation |
| Palettes | background and sprite palette RAM as colour swatches |
| Tiles | VRAM tiles, by character block, in 4bpp or 8bpp |
| Sprites | the 128 OAM entries and their attributes |
| Memory | EWRAM, IWRAM, I/O, palette, VRAM, OAM, ROM and SRAM, with a hex viewer |

The CPU tab includes the flags, the current function name when debug
information is available, and a disassembly around the program counter. The
I/O tab decodes registers such as `DISPCNT`, `BG0CNT` and `KEYINPUT` into
readable fields, which is very useful while learning what a register really
controls.

## Breakpoints and stepping

Click in the gutter to the left of a line number to toggle a breakpoint; a red
dot appears. The next time the game reaches that line it pauses, the inspector
jumps to the CPU tab and the editor highlights the line.

While paused you can:

- **F10** step one line (or one instruction when the line has no debug
  information).
- **F5** continue.
- **F2** / **F4** save and restore a state.
- Watch registers and memory in the inspector; they update as you step.

Breakpoints live in the IDE, not in the ROM. They work because the compiler
records which address belongs to which source line, and the emulator can stop
at an address.

## The Learn panel

**Help > Learn GBA programming (guides)** (or the **Learn** button in the toolbar)
switches the centre of the window to these guides.

The left side lists every document found in `docs/guide/` and `docs/`, guides
first, each sorted by file name. The title shown is the file's first `# Title`
line. Links between documents work; external links open in your browser.

The renderer is intentionally small: headings, paragraphs, bullet and numbered
lists, code blocks, tables, quotes, inline code, bold, italic and links. It
does not show images or raw HTML.

## Settings worth knowing

- **Screen scale** `Fit` follows the panel; a fixed integer scale keeps
  pixels sharp.
- **Run on save** rebuilds and restarts the game whenever you save a file.
  Comfortable for small changes, but it restarts the game every time.
- **Key mapping** click a GBA button and press a key. The defaults are below.
- **Projects folder** where **New Project** suggests putting new projects
  (`~/GBADE Projects` unless changed).

Default keyboard mapping:

| GBA | Key | GBA | Key |
|---|---|---|---|
| A | X | D-pad | arrow keys |
| B | Z | L | A |
| Start | Enter | R | S |
| Select | Backspace | | |
