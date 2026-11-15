//! Builds mGBA (third_party/mgba, a git submodule) as a static library with
//! CMake, then compiles our small C shim against it.
//!
//! mGBA is configured with LIBMGBA_ONLY: no Qt/SDL frontends, no optional
//! dependencies (zlib, libpng, sqlite...), only the GBA core plus the
//! debugger infrastructure that GBADE uses for breakpoints.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // dunce: no `\\?\` prefix on Windows, which CMake does not understand.
    let mgba = dunce::canonicalize(manifest.join("../../third_party/mgba")).expect(
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

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    let build_dir = dst.join("build");
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    // Visual Studio generators are multi-config: the library lands in a
    // folder named after the configuration (build\Release\mgba.lib).
    println!("cargo:rustc-link-search=native={}", build_dir.join("Release").display());
    println!("cargo:rustc-link-lib=static=mgba");
    match target_os.as_str() {
        // mGBA looks up its "portable" config path through CoreFoundation.
        "macos" => println!("cargo:rustc-link-lib=framework=CoreFoundation"),
        // What mGBA's CMakeLists links on Windows (sockets, path helpers),
        // plus the shell and COM functions its config code uses to find
        // %APPDATA% (SHGetKnownFolderPath, CoTaskMemFree).
        "windows" => {
            for lib in ["ws2_32", "shlwapi", "shell32", "ole32"] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        _ => {}
    }

    let mut shim = cc::Build::new();
    shim.file(manifest.join("shim/gbade_shim.c"))
        .include(mgba.join("include"))
        .include(build_dir.join("include"))
        .include(manifest.join("shim"))
        .warnings(true);
    if msvc {
        // mGBA is a static library here: without BUILD_STATIC its headers
        // declare every function __declspec(dllimport) for MSVC.
        shim.define("BUILD_STATIC", None)
            .define("NOMINMAX", None)
            .define("WIN32_LEAN_AND_MEAN", None)
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .flag("/std:c11");
    } else {
        // gnu11, not c11: mGBA's headers use POSIX names (PATH_MAX...) that
        // glibc hides in strict ISO C mode. macOS shows them either way.
        shim.flag_if_supported("-std=gnu11");
    }
    shim.compile("gbade_shim");

    println!("cargo:rerun-if-changed=shim/gbade_shim.c");
    println!("cargo:rerun-if-changed=shim/gbade_shim.h");
    println!("cargo:rerun-if-changed=build.rs");
}
