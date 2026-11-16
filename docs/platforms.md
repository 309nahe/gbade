# Platforms and Packaging

GBADE ships on three platforms from one codebase: a **DMG** on macOS, a
**single executable file** on Linux and a **portable ZIP** on Windows. This
page explains what is inside each, how the bundled compiler is found on each
platform, and what changes (very little) between them.

## One resources folder, three ways to ship it

Whatever the platform, GBADE needs the same folder of resources next to the
program:

```text
resources/
├── toolchain/
│   ├── bin/clang            LLVM 23 compiler, built for this platform
│   ├── bin/ld.lld           LLVM linker
│   ├── bin/clangd           LLVM C language server (editor assistance)
│   └── lib/clang/23/include compiler headers (stdint.h, stddef.h...)
├── sdk/                     gba.h, startup code, linker script, libc
├── examples/                project templates
├── tutorials/               hands-on lessons, each with a project to run
├── docs/                    these guides
└── licenses/
```

The only platform-specific files are the three programs in `toolchain/bin`
(`clang.exe`, `ld.lld.exe` and `clangd.exe` on Windows). The SDK, the
examples and the docs are identical everywhere, and so is the code in your
games: clang always compiles for the GBA, whatever computer it runs on.

| | macOS | Linux | Windows |
|---|---|---|---|
| What you download | `GBADE-<version>.dmg` | `gbade-<version>-linux-x86_64` | `GBADE-<version>-windows-x86_64.zip` |
| Size | about 95 MB | about 155 MB (Linux clang is larger) | about 100 MB |
| The program | `GBADE.app/Contents/MacOS/gbade` | the file itself | `gbade.exe` (and `gbade.com`) |
| The resources | `GBADE.app/Contents/Resources/` | packed inside the file, unpacked on first run | the `resources` folder next to `gbade.exe` |
| Built with | `cargo xtask dmg` | `cargo xtask linux` | `cargo xtask windows` |
| Compiler inside | LLVM `macOS-ARM64` release | LLVM `Linux-X64` release | LLVM `x86_64-pc-windows-msvc` release |

## macOS: the app bundle

A macOS app is a folder that looks like a file. `cargo xtask bundle` fills
`GBADE.app/Contents/Resources/` with the tree above, writes `Info.plist` and
the icon, and signs everything ad hoc. `cargo xtask dmg` wraps the app in a
disk image with a shortcut to Applications.

## Linux: one self-extracting file

Linux has no universal "app folder", so GBADE uses a different trick: the
resources are **appended to the executable**.

```text
gbade-0.3.0-linux-x86_64
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
chmod +x gbade-0.3.0-linux-x86_64
mv gbade-0.3.0-linux-x86_64 ~/.local/bin/gbade
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
| zlib | git support (libgit2) and the bundled compiler; installed everywhere |
| X11 or Wayland, libxkbcommon | the window, loaded at runtime |
| OpenGL (Mesa or a GPU driver) | drawing the window |
| an XDG desktop portal | the Open Project and Export file dialogs |

There is no GTK, Qt or Electron dependency, and nothing to install with a
package manager.

## Windows: a portable ZIP

Windows programs usually live in a folder with their files next to them, so
GBADE ships as exactly that: a ZIP to extract anywhere, with no installer
and nothing written outside the folder until GBADE runs.

```text
GBADE-0.3.0/
├── gbade.exe      the IDE, and the command line
├── gbade.com      the command line's console companion (see below)
├── README.txt
└── resources/     the folder described at the top of this page
```

`gbade.exe` finds `resources` next to itself. Keep the folder together: a
copy of `gbade.exe` on its own cannot find its compiler.

### Installing on Windows

1. Extract the ZIP (right-click, **Extract All…**) wherever you like, for
   example in your user folder or `C:\Tools`.
2. Double-click `gbade.exe`. Windows may say "Windows protected your PC",
   because GBADE is not signed with a code-signing certificate yet: click
   **More info**, then **Run anyway**.
3. Optional: run `.\gbade install-desktop` in that folder to add GBADE to the
   Start menu. Run it again if you move the folder.

### gbade.exe and gbade.com

A Windows program is either a console program or a windowed one, decided
when it is built. `gbade.exe` is windowed, so opening the IDE shows no
console window. But terminals do not wait for windowed programs: the prompt
would come back before `gbade build` finished, and the exit code would be
lost.

`gbade.com` solves that, as Visual Studio's `devenv.com` does. It is a tiny
console program that starts `gbade.exe` with the same arguments, waits for
it and returns its exit code; `gbade.exe` attaches to the terminal's console
for its output and Ctrl+C. cmd and PowerShell try `.com` before `.exe`, so
typing `gbade build my-game` in the GBADE folder (or with the folder in your
`PATH`) just works. In Git Bash, type `./gbade.com`.

### What the Windows version needs from the system

Windows 10 or 11 on x86-64, nothing else. The bundled `clang.exe`,
`ld.lld.exe` and `clangd.exe` are the official LLVM release programs, linked
statically: they use only system DLLs, and no Visual C++ runtime needs to be
installed. libgit2 (for the Git panel) is built into `gbade.exe`. The window
is drawn with OpenGL, and sound goes through WASAPI.

### Where files go on Windows

| What | Where |
|---|---|
| New projects (default) | `%USERPROFILE%\GBADE Projects` |
| Compiled runtime cache | `%LOCALAPPDATA%\gbade\cache` |
| Settings (window, recent projects) | `%APPDATA%\gbade\data` |
| Start menu shortcut (optional) | `%APPDATA%\Microsoft\Windows\Start Menu\Programs\GBADE.lnk` |

Projects are not put in `Documents` because that folder is often synced by
OneDrive: build folders change at every build, and sync clients lock files
while uploading them. To remove GBADE, delete its folder, then these folders
if you want.

## How the toolchain adapts

The code that finds the compiler is the same on every platform
(`Toolchain::locate` in `crates/gbade-toolchain/src/toolchain.rs`). It tries,
in order:

1. `$GBADE_TOOLCHAIN_DIR` and `$GBADE_SDK_DIR`, if set.
2. **The packaged resources** (`gbade_toolchain::bundle::resources_dir`):
   - on macOS, `Contents/Resources` next to the executable inside the app;
   - on Windows (or any unpacked copy), the `resources` folder next to the
     executable;
   - otherwise, the payload appended to the executable, unpacked on demand.
3. A development checkout: `.cache/llvm` and `sdk/` in the repository.

The first one that contains `bin/clang` (`bin\clang.exe` on Windows) and
`include/gba.h` wins; `gbade info` shows which. Everything after that is
platform-independent: the same clang flags
(`--target=arm-none-eabi -mcpu=arm7tdmi -mthumb`), the same linker script and
the same runtime. clang is a cross-compiler on every system: it runs on the
host (an Apple silicon Mac or an x86-64 PC) and always produces code for the
GBA's ARM7TDMI.

The platform conventions themselves (folders, program names, paths) live in
one module, `crates/gbade-toolchain/src/paths.rs`. On Windows it also makes
sure that:

- paths never carry the `\\?\` prefix that Windows' own path resolution
  adds, so error messages, editor tabs and clang's command line show normal
  `C:\...` paths;
- clang and ld.lld start without a console window, so building from the IDE
  does not flash a window per file;
- moving a freshly written folder into place is retried for a moment,
  because antivirus scanners briefly hold new files open.

The compiled runtime cache also follows platform conventions:
`~/Library/Caches/gbade` on macOS, `~/.cache/gbade` on Linux and
`%LOCALAPPDATA%\gbade\cache` on Windows (or `$GBADE_CACHE_DIR`).

### Where the right compiler comes from

`cargo xtask fetch-llvm` downloads the official LLVM release **for the
machine it runs on** and keeps only clang, lld, clangd and the headers:

| Build machine | LLVM release archive |
|---|---|
| macOS on Apple silicon | `LLVM-23.1.5-macOS-ARM64.tar.xz` |
| Linux x86-64 | `LLVM-23.1.5-Linux-X64.tar.xz` |
| Linux ARM64 | `LLVM-23.1.5-Linux-ARM64.tar.xz` |
| Windows x86-64 | `clang+llvm-23.1.5-x86_64-pc-windows-msvc.tar.xz` |
| Windows ARM64 | `clang+llvm-23.1.5-aarch64-pc-windows-msvc.tar.xz` |

The archive is extracted by xtask itself (in Rust), so no `tar` or `xz`
program is needed. So a package is always built on the platform it is for:
the DMG on a Mac, the Linux file on Linux, the ZIP on Windows. Continuous
integration does all three, on a macOS runner, an Ubuntu 22.04 runner and a
Windows runner. Only x86-64 packages are built for Linux and Windows today;
the ARM64 archives are there for building GBADE from source on those
machines.

## What else differs

Very little, and all of it is in the user interface:

| | macOS | Linux | Windows |
|---|---|---|---|
| Shortcuts | shown as ⌘R, ⇧⌘N | shown as Ctrl+R, Ctrl+Shift+N | shown as Ctrl+R, Ctrl+Shift+N |
| Open a folder | `open` (Finder) | `xdg-open` (your file manager) | Explorer |
| Window system | Cocoa | X11 or Wayland | Win32 |
| Sound | Core Audio | ALSA (or PipeWire/PulseAudio through it) | WASAPI |
| Security prompt | Gatekeeper, see [Getting Started](guide/01-getting-started.md) | none; just `chmod +x` | SmartScreen: **More info**, **Run anyway** |
| Command line | the program inside the app | the file itself | `gbade` (runs `gbade.com`) |

The shortcuts themselves are the same keys: egui maps its "command" modifier
to ⌘ on macOS and Ctrl elsewhere. Labels go through one small helper
(`crates/gbade/src/platform.rs`).

## Building the packages yourself

```sh
cargo xtask fetch-llvm    # once per machine
cargo xtask dmg           # on macOS   -> target/GBADE-<version>.dmg
cargo xtask linux         # on Linux   -> target/gbade-<version>-linux-x86_64
cargo xtask windows       # on Windows -> target/GBADE-<version>-windows-x86_64.zip
cargo xtask smoke         # run the package on every example
```

On Linux, building needs the ALSA headers (`sudo apt install libasound2-dev
pkg-config cmake` on Debian and Ubuntu). On Windows, it needs the Visual
Studio Build Tools (**Desktop development with C++**: the MSVC compiler and
the Windows SDK, whose resource compiler embeds the icon), CMake and Git.
Running `cargo xtask linux` on a Mac also works and produces a macOS single
file: handy to test the unpacking code without a Linux machine (`cargo xtask
smoke single`).

`cargo xtask smoke` tests the package like a user would: it runs it from
outside the repository (on Windows, after unzipping it into a temporary
folder, and through `gbade.com`), checks with `gbade info` that it uses its
own compiler rather than the checkout's, then creates, builds and runs every
example.
