# Command Line Reference

The `gbade` executable is both the IDE and a command-line tool. With no
arguments it opens the window; with a command it does the work and exits.

```text
gbade                         open the IDE
gbade build [DIR]             build the project in DIR (default: current folder)
gbade run [DIR] [OPTIONS]     build, run headless, print the console
gbade new DIR [OPTIONS]       create a project from an example
gbade examples                list the templates
gbade --version               print the version
gbade --help                  print this summary
```

`DIR` is the project folder, or any folder inside it: GBADE walks up until it
finds `gbade.toml`.

The command line is the same program as the app. From an installed app, run
it as `/Applications/GBADE.app/Contents/MacOS/gbade`, or add an alias:

```sh
alias gbade=/Applications/GBADE.app/Contents/MacOS/gbade
```

From a checkout, `cargo run -p gbade -- <command>` does the same.

## gbade build

```sh
gbade build
gbade build ~/GBADE\ Projects/pong
```

Builds the project and prints errors and warnings to standard error. On
success it reports the ROM path, its size and the memory used:

```text
built /path/build/pong.gba (24.1 KB, IWRAM 1.9 KB, EWRAM 12.0 KB) in 0.42s
```

## gbade run

Builds, then runs the game without a window and prints everything the game
writes with `printf`.

| Option | Meaning |
|---|---|
| `--frames N` | number of frames to emulate (default 120, about two seconds) |
| `--screenshot FILE.png` | save the last frame |
| `--press KEY@FRAME` | hold a button for 5 frames from `FRAME` |

```sh
gbade run . --frames 300 --press a@60 --screenshot shot.png
gbade run . --frames 600 --press start@30 --press a@90
```

`KEY` is one of `a`, `b`, `start`, `select`, `up`, `down`, `left`, `right`,
`l`, `r`, case-insensitive. Repeat `--press` to hold several buttons at once.

The cartridge save is loaded from `build/<name>.sav` before the run and
written back afterwards, so headless runs share progress with the IDE.

## gbade new

```sh
gbade new my-game
gbade new my-game --template sprites --name "My Game"
```

Copies an example into a new folder and sets the game name. `--template` takes
the folder name of an example, as listed by `gbade examples`; it defaults to
`hello-world`. `--name` defaults to the folder name.

The command refuses to overwrite an existing project and removes any `build/`
folder copied from the template.

## gbade examples

Lists the templates with their short descriptions, in reading order.

## Environment variables

| Variable | Used by | Meaning |
|---|---|---|
| `GBADE_TOOLCHAIN_DIR` | build | folder with `bin/clang` and `bin/ld.lld` |
| `GBADE_SDK_DIR` | build | the `sdk/` folder |
| `GBADE_CACHE_DIR` | build | compiled runtime cache (default `~/Library/Caches/gbade` on macOS, `~/.cache/gbade` elsewhere) |
| `GBADE_RESOURCES_DIR` | new, examples, docs | folder containing `examples/` and `docs/` |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | build error, missing project, or the emulator failed to start |
| 2 | bad arguments or unknown command |
| 3 | the game panicked or emulation stopped unexpectedly |

Exit code 3 is what makes `gbade run` usable in a test: a panic in the game
fails the command.

## Developer tasks: cargo xtask

These build and package GBADE itself, and are run from the repository root.

| Command | What it does |
|---|---|
| `cargo xtask fetch-llvm` | download clang, lld and compiler headers into `.cache/llvm` |
| `cargo xtask icon` | regenerate `crates/gbade/assets/icon.png` |
| `cargo xtask bundle` | build `target/bundle/GBADE.app` with toolchain, SDK, examples, docs and licenses |
| `cargo xtask dmg` | bundle, then create `target/GBADE-<version>.dmg` |
| `cargo xtask smoke` | run the bundled CLI on every example |
| `cargo xtask gen-assets` | regenerate the pixel-art PNGs used by the examples |

The LLVM version fetched is the macOS arm64 release. `bundle` and `dmg` also
need `iconutil`, `hdiutil` and `codesign` from Xcode's command line tools.
