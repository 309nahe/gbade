//! Finding the bundled compiler (clang), linker (ld.lld) and the GBADE SDK.
//!
//! GBADE ships its own copy of LLVM's clang and lld inside the app bundle, so
//! users never install a toolchain. This module looks for them in, in order:
//!
//! 1. `$GBADE_TOOLCHAIN_DIR` (a directory with `bin/clang` and `bin/ld.lld`)
//!    and `$GBADE_SDK_DIR` (the `sdk/` directory): explicit overrides.
//! 2. Inside the macOS app bundle: `GBADE.app/Contents/Resources/toolchain`
//!    and `.../Resources/sdk`.
//! 3. A development checkout: `<repo>/.cache/llvm` (downloaded by
//!    `cargo xtask fetch-llvm`) and `<repo>/sdk`.

use std::path::{Path, PathBuf};

/// Paths to everything needed to build a GBA game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    /// The clang compiler driver.
    pub clang: PathBuf,
    /// The LLVM ELF linker.
    pub lld: PathBuf,
    /// The GBADE SDK directory (headers, runtime, library sources).
    pub sdk: PathBuf,
}

/// Why the toolchain could not be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainError(pub String);

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ToolchainError {}

impl Toolchain {
    /// Build a toolchain from an LLVM directory (containing `bin/`) and an SDK
    /// directory, checking that the needed files exist.
    pub fn from_dirs(llvm_dir: &Path, sdk_dir: &Path) -> Result<Toolchain, ToolchainError> {
        let clang = llvm_dir.join("bin").join("clang");
        let lld = llvm_dir.join("bin").join("ld.lld");
        for (p, what) in [(&clang, "clang"), (&lld, "ld.lld")] {
            if !p.exists() {
                return Err(ToolchainError(format!("{what} not found at {}", p.display())));
            }
        }
        if !sdk_dir.join("include").join("gba.h").exists() {
            return Err(ToolchainError(format!(
                "GBADE SDK not found at {} (missing include/gba.h)",
                sdk_dir.display()
            )));
        }
        Ok(Toolchain { clang, lld, sdk: sdk_dir.to_path_buf() })
    }

    /// Find the toolchain automatically (see module docs for the search order).
    pub fn locate() -> Result<Toolchain, ToolchainError> {
        // 1. Environment overrides.
        let env_llvm = std::env::var_os("GBADE_TOOLCHAIN_DIR").map(PathBuf::from);
        let env_sdk = std::env::var_os("GBADE_SDK_DIR").map(PathBuf::from);

        let mut llvm_candidates: Vec<PathBuf> = Vec::new();
        let mut sdk_candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = env_llvm {
            llvm_candidates.push(p);
        }
        if let Some(p) = env_sdk {
            sdk_candidates.push(p);
        }

        // 2. App bundle: <exe>/../Resources/{toolchain,sdk}
        if let Ok(exe) = std::env::current_exe() {
            let exe = exe.canonicalize().unwrap_or(exe);
            if let Some(macos_dir) = exe.parent() {
                let res = macos_dir.join("..").join("Resources");
                llvm_candidates.push(res.join("toolchain"));
                sdk_candidates.push(res.join("sdk"));
            }
            // 3. Dev checkout: walk up from the executable.
            for dir in exe.ancestors() {
                if dir.join("sdk").join("include").join("gba.h").exists() {
                    llvm_candidates.push(dir.join(".cache").join("llvm"));
                    sdk_candidates.push(dir.join("sdk"));
                    break;
                }
            }
        }
        // 3b. Dev checkout: walk up from this crate's source (tests, cargo run).
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        for dir in manifest.ancestors() {
            if dir.join("sdk").join("include").join("gba.h").exists() {
                llvm_candidates.push(dir.join(".cache").join("llvm"));
                sdk_candidates.push(dir.join("sdk"));
                break;
            }
        }

        let llvm = llvm_candidates.iter().find(|p| p.join("bin").join("clang").exists());
        let sdk = sdk_candidates.iter().find(|p| p.join("include").join("gba.h").exists());
        match (llvm, sdk) {
            (Some(l), Some(s)) => Toolchain::from_dirs(
                &l.canonicalize().unwrap_or_else(|_| l.clone()),
                &s.canonicalize().unwrap_or_else(|_| s.clone()),
            ),
            (None, _) => Err(ToolchainError(format!(
                "Could not find the bundled compiler (clang). Looked in:\n{}\n\
                 If you are developing GBADE, run `cargo xtask fetch-llvm`.",
                llvm_candidates.iter().map(|p| format!("  - {}", p.display())).collect::<Vec<_>>().join("\n")
            ))),
            (_, None) => Err(ToolchainError(format!(
                "Could not find the GBADE SDK. Looked in:\n{}",
                sdk_candidates.iter().map(|p| format!("  - {}", p.display())).collect::<Vec<_>>().join("\n")
            ))),
        }
    }

    /// Directory with GBADE's C headers (`gba.h`, `stdio.h`...).
    pub fn include_dir(&self) -> PathBuf {
        self.sdk.join("include")
    }

    /// The GBA linker script.
    pub fn linker_script(&self) -> PathBuf {
        self.sdk.join("runtime").join("gba.ld")
    }

    /// `clang --version`, first line.
    pub fn clang_version(&self) -> Option<String> {
        let out = std::process::Command::new(&self.clang).arg("--version").output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().next().map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_finds_dev_checkout() {
        // In the repository, `cargo xtask fetch-llvm` must have been run.
        let tc = Toolchain::locate().expect("toolchain should be found in the dev checkout");
        assert!(tc.clang.exists());
        assert!(tc.lld.exists());
        assert!(tc.include_dir().join("gba.h").exists());
        assert!(tc.linker_script().exists());
        let v = tc.clang_version().unwrap();
        assert!(v.contains("clang"), "unexpected version line: {v}");
    }

    #[test]
    fn from_dirs_reports_missing_clang() {
        let dir = tempfile::tempdir().unwrap();
        let err = Toolchain::from_dirs(dir.path(), dir.path()).unwrap_err();
        assert!(err.0.contains("clang not found"));
    }
}
