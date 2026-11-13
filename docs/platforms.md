# Platforms and Packaging

GBADE ships on two platforms from one codebase: a **DMG** on macOS and a
**single executable file** on Linux. This page explains what is inside each,
how the bundled compiler is found on each platform, and what changes (very
little) between them.

## One resources folder, two ways to ship it

Whatever the platform, GBADE needs the same folder of resources next to the
program:

```text
resources/
├── toolchain/
│   ├── bin/clang            LLVM 23 compiler, built for this platform
│   ├── bin/ld.lld           LLVM linker
│   └── lib/clang/23/include compiler headers (stdint.h, stddef.h...)
├── sdk/                     gba.h, startup code, linker script, libc
├── examples/                project templates
├── docs/                    these guides
└── licenses/
```

The only platform-specific files are the two programs in `toolchain/bin`.
The SDK, the examples and the docs are identical everywhere, and so are the
games: a `.gba` ROM built on Linux is byte-for-byte the same as one built on
a Mac.

| | macOS | Linux |
|---|---|---|
| What you download | `GBADE-<version>.dmg` | `gbade-<version>-linux-x86_64` |
| Size | about 80 MB | about 135 MB (Linux clang is larger) |
| The program | `GBADE.app/Contents/MacOS/gbade` | the file itself |
| The resources | `GBADE.app/Contents/Resources/` | packed inside the file, unpacked on first run |
| Built with | `cargo xtask dmg` | `cargo xtask linux` |
| Compiler inside | LLVM `macOS-ARM64` release | LLVM `Linux-X64` release |

## macOS: the app bundle

A macOS app is a folder that looks like a file. `cargo xtask bundle` fills
`GBADE.app/Contents/Resources/` with the tree above, writes `Info.plist` and
the icon, and signs everything ad hoc. `cargo xtask dmg` wraps the app in a
disk image with a shortcut to Applications.

## Linux: one self-extracting file

Linux has no universal "app folder", so GBADE uses a different trick: the
resources are **appended to the executable**.

```text
gbade-0.1.0-linux-x86_64
┌────────────────────────┬──────────────────────────────┬─────────────┐
│ the gbade ELF program  │ resources folder, tar + zstd │ 24-byte tail │
└────────────────────────┴──────────────────────────────┴─────────────┘
```

Linux only loads the ELF part when it starts the program; the extra bytes at
the end are ignored. The 24-byte tail holds the payload size, an id (a hash
of the payload) and the marker `GBADEPK1`.

On the first run, GBADE reads its own file, finds the marker and unpacks the
resources into:

```text
~/.local/share/gbade/<version>-<id>/      ($XDG_DATA_HOME/gbade if set)
```

That takes a second or two and happens once per version. A newer GBADE has a
different id, unpacks into a new folder and deletes the old one. Unpacking
goes into a temporary folder that is renamed at the end, so two copies of
GBADE starting at the same time cannot see a half-written compiler.

### Installing on Linux

```sh
chmod +x gbade-0.1.0-linux-x86_64
mv gbade-0.1.0-linux-x86_64 ~/.local/bin/gbade
gbade install-desktop        # adds GBADE to your applications menu
gbade                        # or start it from a terminal
```

`gbade install-desktop` writes `~/.local/share/applications/gbade.desktop`
and the icon, pointing at wherever the file currently is. Run it again if you
move the file.

### What the Linux binary needs from the system

The file carries its compiler, emulator and SDK. From the system it only uses
what every desktop already has:

| Library | Used for |
|---|---|
| glibc 2.35 or newer | everything (Ubuntu 22.04, Debian 12, Fedora 36 and newer) |
| libasound2 (ALSA) | sound; PipeWire and PulseAudio provide it too |
| X11 or Wayland, libxkbcommon | the window, loaded at runtime |
| OpenGL (Mesa or a GPU driver) | drawing the window |
| an XDG desktop portal | the Open Project and Export file dialogs |

There is no GTK, Qt or Electron dependency, and nothing to install with a
package manager.

## How the toolchain adapts

The code that finds the compiler is the same on both platforms
(`Toolchain::locate` in `crates/gbade-toolchain/src/toolchain.rs`). It tries,
in order:

1. `$GBADE_TOOLCHAIN_DIR` and `$GBADE_SDK_DIR`, if set.
2. **The packaged resources** (`gbade_toolchain::bundle::resources_dir`):
   - on macOS, `Contents/Resources` next to the executable inside the app;
   - otherwise, the payload appended to the executable, unpacked on demand.
3. A development checkout: `.cache/llvm` and `sdk/` in the repository.

The first one that contains `bin/clang` and `include/gba.h` wins. Everything
after that is platform-independent: the same clang flags
(`--target=arm-none-eabi -mcpu=arm7tdmi -mthumb`), the same linker script and
the same runtime. clang is a cross-compiler on both systems: it runs on the
host (an Apple silicon Mac or an x86-64 PC) and always produces code for the
GBA's ARM7TDMI.

The compiled runtime cache also follows platform conventions:
`~/Library/Caches/gbade` on macOS and `~/.cache/gbade` on Linux (or
`$GBADE_CACHE_DIR`).

### Where the right compiler comes from

`cargo xtask fetch-llvm` downloads the official LLVM release **for the
machine it runs on** and keeps only clang, lld and the headers:

| Build machine | LLVM release archive |
|---|---|
| macOS on Apple silicon | `LLVM-23.1.5-macOS-ARM64.tar.xz` |
| Linux x86-64 | `LLVM-23.1.5-Linux-X64.tar.xz` |
| Linux ARM64 | `LLVM-23.1.5-Linux-ARM64.tar.xz` |

So a package is always built on the platform it is for: the DMG on a Mac,
the Linux file on Linux. Continuous integration does both, on a macOS runner
and an Ubuntu 22.04 runner.

## What else differs

Very little, and all of it is in the user interface:

| | macOS | Linux |
|---|---|---|
| Shortcuts | shown as ⌘R, ⇧⌘N | shown as Ctrl+R, Ctrl+Shift+N |
| Open a folder | `open` (Finder) | `xdg-open` (your file manager) |
| Window system | Cocoa | X11 or Wayland |
| Sound | Core Audio | ALSA (or PipeWire/PulseAudio through it) |
| Security prompt | Gatekeeper, see [Getting Started](guide/01-getting-started.md) | none; just `chmod +x` |

The shortcuts themselves are the same keys: egui maps its "command" modifier
to ⌘ on macOS and Ctrl on Linux. Labels go through one small helper
(`crates/gbade/src/platform.rs`).

## Building the packages yourself

```sh
cargo xtask fetch-llvm    # once per machine
cargo xtask dmg           # on macOS  -> target/GBADE-<version>.dmg
cargo xtask linux         # on Linux  -> target/gbade-<version>-linux-x86_64
cargo xtask smoke         # run the package on every example
```

On Linux, building needs the ALSA headers (`sudo apt install libasound2-dev
pkg-config cmake` on Debian and Ubuntu). Running `cargo xtask linux` on a Mac
also works and produces a macOS single file: handy to test the unpacking code
without a Linux machine (`cargo xtask smoke single`).
