//! Platform conventions in one place: where per-user files go, how the
//! bundled programs are named, how they are started, and how paths are
//! normalised.
//!
//! | | macOS | Linux | Windows |
//! |---|---|---|---|
//! | [`home_dir`] | `$HOME` | `$HOME` | `%USERPROFILE%` |
//! | [`cache_dir`] | `~/Library/Caches/gbade` | `~/.cache/gbade` | `%LOCALAPPDATA%\gbade\cache` |
//! | [`data_dir`] | `~/Library/Application Support/gbade` | `~/.local/share/gbade` | `%LOCALAPPDATA%\gbade` |
//! | [`exe`]`("clang")` | `clang` | `clang` | `clang.exe` |
//!
//! `$GBADE_CACHE_DIR` and `$GBADE_DATA_DIR` override the first two
//! everywhere (tests and the smoke test use them).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The user's home folder (`%USERPROFILE%` on Windows), or the temporary
/// folder if there is none.
pub fn home_dir() -> PathBuf {
    #[allow(deprecated)] // un-deprecated in Rust 1.87, and correct on Windows since 1.85
    std::env::home_dir().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(std::env::temp_dir)
}

/// `%LOCALAPPDATA%` on Windows: per-user, per-machine data that is not
/// roamed between computers (the right place for a compiler cache).
#[cfg(windows)]
fn local_app_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join("AppData").join("Local"))
}

/// Where GBADE caches the compiled runtime library.
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GBADE_CACHE_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(windows)]
    {
        local_app_data().join("gbade").join("cache")
    }
    #[cfg(not(windows))]
    {
        if cfg!(target_os = "macos") {
            home_dir().join("Library").join("Caches").join("gbade")
        } else {
            home_dir().join(".cache").join("gbade")
        }
    }
}

/// Where per-user data lives (the single-file build unpacks itself here).
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GBADE_DATA_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(windows)]
    {
        local_app_data().join("gbade")
    }
    #[cfg(not(windows))]
    {
        if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(d).join("gbade");
        }
        if cfg!(target_os = "macos") {
            home_dir().join("Library").join("Application Support").join("gbade")
        } else {
            home_dir().join(".local").join("share").join("gbade")
        }
    }
}

/// The file name of a program on this platform: `clang` becomes `clang.exe`
/// on Windows and stays `clang` elsewhere.
pub fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// `path.canonicalize()`, without Windows' `\\?\C:\...` "verbatim" prefix
/// (which would end up in error messages, the editor's tabs and clang's
/// command line). Falls back to `path` itself if it does not exist.
pub fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A command for one of the bundled tools (clang, ld.lld). On Windows the
/// tool gets no console window of its own: without this, every file compiled
/// from the IDE would flash a black window on screen.
pub fn tool_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// `std::fs::rename`, retried for a moment on Windows: antivirus scanners
/// and the search indexer briefly open files that were just written, and
/// Windows refuses to move a folder while any file in it is open.
pub fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    let attempts = if cfg!(windows) { 20 } else { 1 };
    let mut last = None;
    for i in 0..attempts {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            // The destination exists: retrying will not help, the caller
            // decides what to do (usually: another process won the race).
            Err(e) if to.exists() => return Err(e),
            Err(e) => last = Some(e),
        }
        if i + 1 < attempts {
            std::thread::sleep(Duration::from_millis(50 * (i as u64 + 1)));
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("rename failed")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_names_follow_the_platform() {
        if cfg!(windows) {
            assert_eq!(exe("clang"), "clang.exe");
            assert_eq!(exe("ld.lld"), "ld.lld.exe");
        } else {
            assert_eq!(exe("clang"), "clang");
            assert_eq!(exe("ld.lld"), "ld.lld");
        }
    }

    #[test]
    fn canonical_has_no_verbatim_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let p = canonical(dir.path());
        assert!(p.is_absolute());
        assert!(!p.to_string_lossy().starts_with(r"\\?\"), "{}", p.display());
        // A missing path is returned unchanged.
        let missing = dir.path().join("missing");
        assert_eq!(canonical(&missing), missing);
    }

    #[test]
    fn per_user_dirs_are_absolute() {
        assert!(home_dir().is_absolute());
        for d in [cache_dir(), data_dir()] {
            assert!(d.is_absolute(), "{}", d.display());
        }
    }

    #[test]
    fn rename_moves_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("f"), "x").unwrap();
        rename(&a, &b).unwrap();
        assert_eq!(std::fs::read_to_string(b.join("f")).unwrap(), "x");
        // Onto an existing folder: an error, returned at once.
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("g"), "y").unwrap();
        std::fs::write(b.join("keep"), "z").unwrap();
        assert!(rename(&a, &b).is_err());
    }
}
