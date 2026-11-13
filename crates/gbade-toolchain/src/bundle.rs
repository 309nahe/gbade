//! Where GBADE's packaged resources live, on every platform.
//!
//! GBADE needs a folder of resources next to the program:
//!
//! ```text
//! resources/
//! ├── toolchain/   bin/clang, bin/ld.lld, lib/clang/<N>/include
//! ├── sdk/         gba.h, crt0, linker script, libc...
//! ├── examples/    project templates
//! ├── docs/        guides shown in the Learn panel
//! └── licenses/
//! ```
//!
//! The tree is identical everywhere; only the way it ships differs:
//!
//! - **macOS**: inside the app bundle, `GBADE.app/Contents/Resources/`.
//! - **Linux**: appended to the `gbade` executable itself, as a zstd
//!   compressed tar archive followed by a small trailer. The first run unpacks
//!   it into `~/.local/share/gbade/<version>-<id>/` (or `$XDG_DATA_HOME`).
//!   So the whole IDE, compiler included, is one file you can download and run.
//! - **Development checkout**: no bundle; `Toolchain::locate` falls back to
//!   `.cache/llvm` and the repository's `sdk/`.
//!
//! Payload layout at the end of the executable:
//!
//! ```text
//! [ executable ][ tar.zst payload ][ payload length: u64 LE ][ id: u64 LE ][ b"GBADEPK1" ]
//! ```
//!
//! `id` is a hash of the payload, so a new build unpacks into a new folder and
//! an old extraction is never mixed with new files.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Marks the end of an executable that carries resources.
pub const MAGIC: &[u8; 8] = b"GBADEPK1";
const TRAILER_LEN: u64 = 24;

/// The resources folder of the running program, if it is a packaged build.
/// On Linux this unpacks the appended payload on first use (a few seconds).
/// The result is computed once per process.
pub fn resources_dir() -> Option<PathBuf> {
    static CACHE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    CACHE.get_or_init(find_resources).clone()
}

fn find_resources() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    // macOS app bundle: <app>/Contents/MacOS/gbade -> <app>/Contents/Resources
    if let Some(dir) = exe.parent() {
        let res = dir.join("..").join("Resources");
        if res.join("sdk").join("include").join("gba.h").exists() {
            return Some(res.canonicalize().unwrap_or(res));
        }
    }
    // Single-file build: resources appended to the executable.
    match extract_payload(&exe, &data_dir()) {
        Ok(Some(dir)) => Some(dir),
        Ok(None) => None,
        Err(e) => {
            eprintln!("gbade: could not unpack the bundled resources: {e}");
            None
        }
    }
}

/// Where single-file builds unpack their resources.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GBADE_DATA_DIR") {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(d).join("gbade");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("gbade")
    } else {
        home.join(".local").join("share").join("gbade")
    }
}

/// Read the trailer of `exe`: (payload offset, payload length, id).
pub fn read_trailer(exe: &Path) -> std::io::Result<Option<(u64, u64, u64)>> {
    let mut f = File::open(exe)?;
    let size = f.metadata()?.len();
    if size < TRAILER_LEN {
        return Ok(None);
    }
    f.seek(SeekFrom::Start(size - TRAILER_LEN))?;
    let mut t = [0u8; TRAILER_LEN as usize];
    f.read_exact(&mut t)?;
    if &t[16..24] != MAGIC {
        return Ok(None);
    }
    let len = u64::from_le_bytes(t[0..8].try_into().unwrap());
    let id = u64::from_le_bytes(t[8..16].try_into().unwrap());
    if len > size - TRAILER_LEN {
        return Err(std::io::Error::other("corrupt resource trailer"));
    }
    Ok(Some((size - TRAILER_LEN - len, len, id)))
}

/// Unpack the payload of `exe` into `<base>/<version>-<id>/` unless already
/// done. Returns `None` when the executable carries no payload.
pub fn extract_payload(exe: &Path, base: &Path) -> std::io::Result<Option<PathBuf>> {
    let Some((offset, len, id)) = read_trailer(exe)? else { return Ok(None) };
    let name = format!("{}-{id:016x}", env!("CARGO_PKG_VERSION"));
    let dest = base.join(&name);
    if dest.join(".complete").exists() {
        return Ok(Some(dest));
    }
    std::fs::create_dir_all(base)?;
    eprintln!("gbade: unpacking the bundled compiler and SDK to {} (first run only)…", dest.display());

    // Unpack into a private temporary folder, then rename it into place, so
    // two GBADE processes starting at once never see a half-written tree.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = base.join(format!(".unpack-{name}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = (|| {
        let mut f = File::open(exe)?;
        f.seek(SeekFrom::Start(offset))?;
        let decoder = zstd::stream::read::Decoder::new(f.take(len))?;
        let mut archive = tar::Archive::new(decoder);
        archive.set_preserve_permissions(true);
        archive.unpack(&tmp)?;
        File::create(tmp.join(".complete"))?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if std::fs::rename(&tmp, &dest).is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        if !dest.join(".complete").exists() {
            return Err(std::io::Error::other(format!("could not move resources into {}", dest.display())));
        }
    }
    // Remove extractions left by older versions (each is a few hundred MB).
    if let Ok(rd) = std::fs::read_dir(base) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n != name && !n.starts_with('.') && e.path().join(".complete").exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(Some(dest))
}

/// Build a single-file executable: copy `exe`, append `resources` (a folder)
/// as a tar.zst payload and the trailer. Used by `cargo xtask linux`.
pub fn write_bundle(exe: &Path, resources: &Path, out: &Path, level: i32) -> std::io::Result<u64> {
    // 1. Compress the resources into memory (a few hundred MB uncompressed,
    //    well under a hundred compressed).
    let mut payload = Vec::new();
    {
        let mut enc = zstd::stream::write::Encoder::new(&mut payload, level)?;
        enc.multithread(std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1))?;
        {
            let mut tar = tar::Builder::new(&mut enc);
            tar.follow_symlinks(true);
            tar.mode(tar::HeaderMode::Deterministic);
            tar.append_dir_all(".", resources)?;
            tar.finish()?;
        }
        enc.finish()?;
    }
    // 2. id = FNV-1a hash of the payload.
    let mut id: u64 = 0xcbf29ce484222325;
    for &b in &payload {
        id ^= b as u64;
        id = id.wrapping_mul(0x100000001b3);
    }
    // 3. executable + payload + trailer.
    let mut out_f = File::create(out)?;
    std::io::copy(&mut File::open(exe)?, &mut out_f)?;
    out_f.write_all(&payload)?;
    out_f.write_all(&(payload.len() as u64).to_le_bytes())?;
    out_f.write_all(&id.to_le_bytes())?;
    out_f.write_all(MAGIC)?;
    drop(out_f);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(payload.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_files_have_no_payload() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("prog");
        std::fs::write(&f, b"just a program, no trailer here").unwrap();
        assert_eq!(read_trailer(&f).unwrap(), None);
        assert_eq!(extract_payload(&f, &dir.path().join("data")).unwrap(), None);
    }

    #[test]
    fn bundle_roundtrip_preserves_files_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let res = dir.path().join("res");
        std::fs::create_dir_all(res.join("toolchain/bin")).unwrap();
        std::fs::create_dir_all(res.join("sdk/include")).unwrap();
        std::fs::write(res.join("sdk/include/gba.h"), "#pragma once\n").unwrap();
        std::fs::write(res.join("toolchain/bin/clang"), "#!/bin/sh\necho clang\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(res.join("toolchain/bin/clang"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let exe = dir.path().join("exe");
        std::fs::write(&exe, vec![0x7Fu8; 1000]).unwrap();
        let out = dir.path().join("bundle");
        write_bundle(&exe, &res, &out, 3).unwrap();

        // The executable part is untouched.
        let data = std::fs::read(&out).unwrap();
        assert_eq!(&data[..1000], &vec![0x7Fu8; 1000][..]);
        let (offset, _len, id) = read_trailer(&out).unwrap().unwrap();
        assert_eq!(offset, 1000);

        let base = dir.path().join("data");
        let dest = extract_payload(&out, &base).unwrap().unwrap();
        assert!(dest.ends_with(format!("{}-{id:016x}", env!("CARGO_PKG_VERSION"))));
        assert_eq!(std::fs::read_to_string(dest.join("sdk/include/gba.h")).unwrap(), "#pragma once\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dest.join("toolchain/bin/clang")).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "clang must stay executable, mode {mode:o}");
        }
        // Second call reuses the extraction.
        assert_eq!(extract_payload(&out, &base).unwrap().unwrap(), dest);
        assert!(std::fs::read_dir(&base).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".unpack")));
    }

    #[test]
    fn corrupt_trailer_is_an_error_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("bad");
        let mut data = vec![0u8; 10];
        data.extend_from_slice(&u64::MAX.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(MAGIC);
        std::fs::write(&f, data).unwrap();
        assert!(read_trailer(&f).is_err());
    }
}
