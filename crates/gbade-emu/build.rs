//! Builds mGBA (third_party/mgba, a git submodule) as a static library with
//! CMake, then compiles our small C shim against it.
//!
//! mGBA is configured with LIBMGBA_ONLY: no Qt/SDL frontends, no optional
//! dependencies (zlib, libpng, sqlite...), only the GBA core plus the
//! debugger infrastructure that GBADE uses for breakpoints.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let mgba = manifest.join("../../third_party/mgba").canonicalize().expect(
        "third_party/mgba is missing: run `git submodule update --init --recursive`",
    );
    if !mgba.join("CMakeLists.txt").exists() {
        panic!("third_party/mgba is empty: run `git submodule update --init --recursive`");
    }

    let dst = cmake::Config::new(&mgba)
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .define("LIBMGBA_ONLY", "ON")
        .define("M_CORE_GB", "OFF")
        .define("M_CORE_GBA", "ON")
        .define("USE_DEBUGGERS", "ON")
        .define("USE_GDB_STUB", "OFF")
        .define("USE_EDITLINE", "OFF")
        .define("ENABLE_SCRIPTING", "OFF")
        .define("USE_ELF", "OFF")
        .define("USE_LUA", "OFF")
        .define("SKIP_GIT", "ON")
        .define("CMAKE_OSX_DEPLOYMENT_TARGET", "12.0")
        .profile("Release")
        .build_target("mgba")
        .build();

    let build_dir = dst.join("build");
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!("cargo:rustc-link-lib=static=mgba");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // mGBA looks up its "portable" config path through CoreFoundation.
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }

    cc::Build::new()
        .file(manifest.join("shim/gbade_shim.c"))
        .include(mgba.join("include"))
        .include(build_dir.join("include"))
        .include(manifest.join("shim"))
        // gnu11, not c11: mGBA's headers use POSIX names (PATH_MAX...) that
        // glibc hides in strict ISO C mode. macOS shows them either way.
        .flag_if_supported("-std=gnu11")
        .warnings(true)
        .compile("gbade_shim");

    println!("cargo:rerun-if-changed=shim/gbade_shim.c");
    println!("cargo:rerun-if-changed=shim/gbade_shim.h");
    println!("cargo:rerun-if-changed=build.rs");
}
