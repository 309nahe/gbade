# Credits and Licenses

GBADE is a small program standing on a lot of other people's work. This page
lists what is included, under which license, and where the full license texts
live. The same credit list appears in **Help > About GBADE**.

## Emulator

- **mGBA** by Vicki Pfau and contributors, version 0.10.5.
  The emulator core. Mozilla Public License 2.0.
  [mgba.io](https://mgba.io)
  Full text: [licenses/MPL-2.0.txt](../licenses/MPL-2.0.txt)

  mGBA is linked into GBADE as a submodule (`third_party/mgba`) and used
  through a small shim (`crates/gbade-emu/shim`). Its source is available in
  this repository and at the URL above.

## Compiler and linker

- **LLVM** clang and lld, version 23.1.5.
  Apache License 2.0 with LLVM Exceptions.
  [llvm.org](https://llvm.org)
  Full text: [licenses/LLVM.txt](../licenses/LLVM.txt) and
  [licenses/Apache-2.0.txt](../licenses/Apache-2.0.txt)

- **compiler-rt** builtins from LLVM, for integer division, 64-bit arithmetic
  and software floating point on the ARM7TDMI.
  Apache License 2.0 with LLVM Exceptions. The used sources live in
  `sdk/runtime/compiler-rt/` and are compiled into every game.

The compiler is not stored in this repository; `cargo xtask fetch-llvm`
downloads the official release and `cargo xtask bundle` copies the needed
binaries and headers into the app.

## C library and font

- **printf** by Marco Paland.
  MIT License.
  [github.com/mpaland/printf](https://github.com/mpaland/printf)
  Full text: [licenses/MIT.txt](../licenses/MIT.txt)

  Used for `printf`, `sprintf` and `snprintf` in the SDK
  (`sdk/runtime/printf/`).

- **font8x8** by Daniel Hepper, based on a public-domain 8x8 VGA font by
  Marcel Sondaar.
  Public domain.
  [github.com/dhepper/font8x8](https://github.com/dhepper/font8x8)
  Notice: [licenses/font8x8.txt](../licenses/font8x8.txt)

  Used as the built-in text font (`sdk/runtime/font/font8x8_basic.h`).

## Cartridge header

- **agb-gbafix** from the `agb` project.
  The Nintendo logo bytes used by the cartridge header.
  Mozilla Public License 2.0.
  [github.com/agbrs/agb](https://github.com/agbrs/agb)
  Full text: [licenses/MPL-2.0.txt](../licenses/MPL-2.0.txt)

  The bytes are reproduced in `crates/gbade-toolchain/src/nintendo_logo.rs`.

## User interface and support crates

GBADE is written in Rust. These crates are compiled into the application:

| Crate | Version | License | Used for |
|---|---|---|---|
| egui / eframe | 0.33.3 | MIT OR Apache-2.0 | the user interface |
| cpal | 0.18.2 | Apache-2.0 | audio output |
| rfd | 0.17.2 | MIT | native file dialogs |
| image | 0.25.10 | MIT OR Apache-2.0 | PNG decoding and screenshots |
| object | 0.41.0 | Apache-2.0 OR MIT | reading the linked ELF |
| gimli | 0.34.0 | MIT OR Apache-2.0 | debug information |
| serde | 1.0.229 | MIT OR Apache-2.0 | settings and project files |
| toml | 1.2.1 | MIT OR Apache-2.0 | reading `gbade.toml` |

Each crate's own repository and source carry its copyright notices. The full
license texts are [licenses/MIT.txt](../licenses/MIT.txt) and
[licenses/Apache-2.0.txt](../licenses/Apache-2.0.txt). Per-crate details and
copyright holders are listed in
[licenses/RUST-CRATES.md](../licenses/RUST-CRATES.md).

## Fonts in the user interface

eframe's `default_fonts` feature embeds **epaint_default_fonts 0.33.3**, which
ships four font files. They are used by the IDE's own interface, not by the
games you write (those use the 8x8 font8x8 bitmap font above).

- **Ubuntu Light** by Canonical Ltd. Ubuntu Font Licence 1.0.
  [licenses/Ubuntu-Font-1.0.txt](../licenses/Ubuntu-Font-1.0.txt)
- **Noto Emoji** by Google Inc. SIL Open Font License 1.1.
  [licenses/SIL-OFL-1.1.txt](../licenses/SIL-OFL-1.1.txt)
- **Hack** by the Source Foundry Authors, with DejaVu and Bitstream Vera
  components. MIT License and Bitstream Vera License.
  [licenses/Hack-font.txt](../licenses/Hack-font.txt)
- **emoji-icon-font** by John Slegers. MIT License.
  [licenses/emoji-icon-font-MIT.txt](../licenses/emoji-icon-font-MIT.txt)

## Reference sources

These works are not part of the binary; they taught the people who wrote it
how the hardware behaves, and the SDK follows their conventions.

- **Tonc** by Jasper Vijn, "The Optimized Nintendo Console" GBA tutorial.
  [gbadev.net/tonc](https://gbadev.net/tonc/)
  The register names and the tone of the explanations in `gba.h` follow Tonc.

- **GBATEK** by Martin Korth, "GBA, NDS and DSi Technical Reference".
  [problemkaputt.de/gbatek.htm](https://problemkaputt.de/gbatek.htm)
  The hardware details behind the memory map, display modes and registers.

## GBADE itself

GBADE's own source code (the Rust crates, the C SDK written for GBADE, the
examples and these docs) is licensed under your choice of the MIT License or
the Apache License 2.0, the usual choice for Rust projects. The texts are
`LICENSE-MIT` and `LICENSE-APACHE` at the root of the repository, and in
`Contents/Resources/licenses/` inside the app.

The third-party parts listed above keep their own licenses. In particular the
vendored files in `sdk/runtime/compiler-rt/`, `sdk/runtime/printf/` and
`sdk/runtime/font/`, and `crates/gbade-toolchain/src/nintendo_logo.rs`
(MPL-2.0), are not covered by GBADE's license.

Your games are yours. The SDK code that gets compiled into a ROM is under the
permissive licenses above (GBADE's MIT/Apache-2.0, compiler-rt's Apache-2.0
with the LLVM exception, which waives attribution for compiled code, printf's
MIT and font8x8's public domain), so you can distribute or sell your ROMs.
Keeping a credits line for printf (MIT) is the polite and safe thing to do.

## A note on trademarks

Game Boy and Game Boy Advance are trademarks of Nintendo. GBADE is an
independent development tool, not affiliated with or endorsed by Nintendo. The
cartridge header uses the Nintendo logo only because the hardware requires
those exact bytes in every cartridge.
