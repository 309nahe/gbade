# Third-Party Licenses

This folder holds the license texts for the third-party components shipped
with GBADE. `cargo xtask bundle` copies it into the app, so the texts travel
with every build.

| File | Applies to |
|---|---|
| `MPL-2.0.txt` | mGBA 0.10.5, the agb-gbafix Nintendo logo bytes |
| `LLVM.txt` | LLVM clang, lld and compiler-rt (Apache-2.0 with LLVM exceptions) |
| `Apache-2.0.txt` | the Apache part of LLVM's license and of the Rust crates above |
| `MIT.txt` | printf by Marco Paland, and the MIT-licensed Rust crates |
| `font8x8.txt` | the 8x8 text font, public domain |
| `RUST-CRATES.md` | the exact list of Rust crates and their license expressions |

The human-readable credit list is `docs/CREDITS.md`, also shown from
**Help > About GBADE**.
