# Rust Dependencies

GBADE is built in Rust. The crates below are compiled into the application
(or into the build tooling). Versions are the ones pinned in `Cargo.lock` at
the time of writing; run `cargo tree` for the full transitive list.

The full license texts are in this folder:

- [MIT.txt](MIT.txt) the MIT License text.
- [Apache-2.0.txt](Apache-2.0.txt) the Apache License 2.0 text.

Each crate's source archive and repository carry its own copyright notices.
Those notices belong to the crate authors and contributors; they are not
reproduced here.

| Crate | Version | License | Repository |
|---|---|---|---|
| egui | 0.33.3 | MIT OR Apache-2.0 | https://github.com/emilk/egui |
| eframe | 0.33.3 | MIT OR Apache-2.0 | https://github.com/emilk/egui |
| cpal | 0.18.2 | Apache-2.0 | https://github.com/RustAudio/cpal |
| rfd | 0.17.2 | MIT | https://github.com/PolyMeilex/rfd |
| image | 0.25.10 | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| object | 0.41.0 | Apache-2.0 OR MIT | https://github.com/gimli-rs/object |
| gimli | 0.34.0 | MIT OR Apache-2.0 | https://github.com/gimli-rs/gimli |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| toml | 1.2.1 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |

"MIT OR Apache-2.0" means the crate may be used under either license; the
recipient chooses. Both texts are included so that either choice is covered.
"Apache-2.0 OR MIT" is the same choice written in the other order.

## Build-time dependencies

The bundling tasks (`xtask`) also use the `image` crate to regenerate the app
icon and the example pixel art. The compiler and linker shipped inside the app
are LLVM, not Rust crates; see [../docs/CREDITS.md](../docs/CREDITS.md).

## Other third-party components

| Component | Version | License | Text |
|---|---|---|---|
| mGBA | 0.10.5 | MPL-2.0 | [MPL-2.0.txt](MPL-2.0.txt) |
| LLVM (clang, lld, compiler-rt) | 23.1.5 | Apache-2.0 WITH LLVM-exception | [LLVM.txt](LLVM.txt) |
| printf by Marco Paland | bundled | MIT | [MIT.txt](MIT.txt) |
| font8x8 by Daniel Hepper | bundled | Public domain | [font8x8.txt](font8x8.txt) |
| agb-gbafix Nintendo logo | bundled | MPL-2.0 | [MPL-2.0.txt](MPL-2.0.txt) |
