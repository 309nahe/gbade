# Third-Party Licenses

This folder holds the license texts for the third-party components shipped
with GBADE. `cargo xtask bundle` copies it into the app, so the texts travel
with every build.

GBADE's own code is under MIT OR Apache-2.0: see `LICENSE-MIT` and
`LICENSE-APACHE` at the repository root (copied here as `GBADE-LICENSE-MIT`
and `GBADE-LICENSE-APACHE` in the app bundle).

| File | Applies to |
|---|---|
| `MPL-2.0.txt` | mGBA 0.10.5, the agb-gbafix Nintendo logo bytes |
| `LLVM.txt` | LLVM clang and lld (Apache-2.0 with LLVM exceptions). compiler-rt's own `LICENSE.TXT`, including its legacy UIUC/MIT text, is at `sdk/runtime/compiler-rt/LICENSE.TXT`. |
| `Apache-2.0.txt` | the Apache part of LLVM's license and of the Rust crates listed in `RUST-CRATES.md` |
| `MIT.txt` | printf by Marco Paland, and the MIT-licensed Rust crates |
| `font8x8.txt` | the 8x8 text font, public domain |
| `Ubuntu-Font-1.0.txt` | Ubuntu Light, embedded by eframe's default fonts |
| `SIL-OFL-1.1.txt` | Noto Emoji, embedded by eframe's default fonts |
| `Hack-font.txt` | Hack, embedded by eframe's default fonts |
| `emoji-icon-font-MIT.txt` | emoji-icon-font, embedded by eframe's default fonts |
| `RUST-CRATES.md` | the exact list of Rust crates and their license expressions |

The human-readable credit list is `docs/CREDITS.md`, also shown from
**Help > About GBADE**.
