//! Developer tasks for GBADE. Run with `cargo xtask <command>`:
//!
//! - `fetch-llvm`  download the official LLVM release and extract clang, lld
//!                 and the compiler headers into `.cache/llvm`
//! - `icon`        regenerate the app icons (`crates/gbade/assets/icon.{png,ico}`)
//! - `bundle`      build `target/bundle/GBADE.app` (release build + toolchain
//!                 + SDK + examples + docs), ad-hoc signed
//! - `dmg`         bundle, then create `target/GBADE-<version>.dmg`
//! - `linux`       build the single-file Linux executable
//!                 `target/gbade-<version>-linux-<arch>` (IDE + compiler + SDK)
//! - `windows`     build the portable Windows ZIP
//!                 `target/GBADE-<version>-windows-<arch>.zip` (on Windows)
//! - `smoke`       run the packaged app's CLI on every example (sanity check)
//! - `gen-assets`  regenerate the pixel-art PNGs used by the examples

use std::path::{Path, PathBuf};
use std::process::Command;

const LLVM_VERSION: &str = "23.1.5";
const LLVM_MAJOR: &str = "23";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("fetch-llvm") => fetch_llvm(),
        Some("icon") => icon(),
        Some("bundle") => bundle().map(|_| ()),
        Some("dmg") => dmg(),
        Some("linux") => linux().map(|_| ()),
        Some("windows") => windows().map(|_| ()),
        Some("smoke") => smoke(&args[1..]),
        Some("gen-assets") => gen_assets(),
        _ => {
            eprintln!("usage: cargo xtask <fetch-llvm|icon|bundle|dmg|linux|windows|smoke|gen-assets>");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cmd: &mut Command) -> Result<(), String> {
    eprintln!("$ {cmd:?}");
    let status = cmd.status().map_err(|e| format!("cannot run {cmd:?}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("command failed: {cmd:?}")) }
}

// ---------------------------------------------------------------------------
// fetch-llvm
// ---------------------------------------------------------------------------

/// One official LLVM release archive, and what GBADE keeps from it.
struct LlvmRelease {
    /// The archive's top-level folder; the file is `<top>.tar.xz`.
    top: String,
    /// Files to extract, relative to `top` (plus everything in `lib/clang/`).
    files: Vec<String>,
}

/// The official LLVM release archive for the machine we build on. GBADE
/// bundles the compiler of the platform it ships for, so this is also the
/// platform of the packaged app.
fn llvm_release() -> Result<LlvmRelease, String> {
    let unix = |platform: &str| LlvmRelease {
        top: format!("LLVM-{LLVM_VERSION}-{platform}"),
        // `clang` and `ld.lld` are symbolic links to `clang-23` and `lld`.
        files: vec![
            format!("bin/clang-{LLVM_MAJOR}"),
            "bin/clang".into(),
            "bin/lld".into(),
            "bin/ld.lld".into(),
            "bin/clangd".into(),
        ],
    };
    // Windows archives use the older naming scheme and contain no links:
    // `ld.lld.exe` is a full copy of `lld.exe` (only the former is needed).
    let windows = |arch: &str| LlvmRelease {
        top: format!("clang+llvm-{LLVM_VERSION}-{arch}-pc-windows-msvc"),
        files: vec!["bin/clang.exe".into(), "bin/ld.lld.exe".into(), "bin/clangd.exe".into()],
    };
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => unix("macOS-ARM64"),
        ("linux", "x86_64") => unix("Linux-X64"),
        ("linux", "aarch64") => unix("Linux-ARM64"),
        ("windows", "x86_64") => windows("x86_64"),
        ("windows", "aarch64") => windows("aarch64"),
        (os, arch) => return Err(format!("no official LLVM {LLVM_VERSION} release for {os}/{arch}")),
    })
}

/// `<llvm>/bin/clang`, `clang.exe` on Windows.
fn llvm_clang(llvm: &Path) -> PathBuf {
    llvm.join("bin").join(gbade_toolchain::paths::exe("clang"))
}

fn fetch_llvm() -> Result<(), String> {
    let cache = root().join(".cache");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let release = llvm_release()?;
    let archive = cache.join(format!("{}.tar.xz", release.top));
    let dest = cache.join("llvm");
    // Already extracted (e.g. restored from the CI cache): skip the 1-2 GB
    // download. An extraction missing a file (clangd, the C language server
    // used by the editor, was added later) is redone.
    if release.files.iter().all(|f| dest.join(f).exists()) {
        eprintln!("{} already extracted", dest.display());
        return Ok(());
    }
    if !archive.exists() {
        let url = format!(
            "https://github.com/llvm/llvm-project/releases/download/llvmorg-{LLVM_VERSION}/{}.tar.xz",
            release.top
        );
        let part = archive.with_extension("part");
        // curl ships with macOS, every Linux distribution and Windows 10+.
        run(Command::new("curl").args(["-fL", "--progress-bar", "-o"]).arg(&part).arg(&url))?;
        std::fs::rename(&part, &archive).map_err(|e| e.to_string())?;
    }
    // Extract next to the destination, then rename: an interrupted
    // extraction never looks finished.
    let partial = cache.join("llvm.partial");
    let _ = std::fs::remove_dir_all(&partial);
    let _ = std::fs::remove_dir_all(&dest);
    eprintln!("extracting clang, lld and clangd from {} (a minute or two)…", archive.display());
    let n = extract_llvm(&archive, &release, &partial)?;
    gbade_toolchain::paths::rename(&partial, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    eprintln!("LLVM extracted to {} ({n} files)", dest.display());
    Ok(())
}

/// Extract `release.files` and `lib/clang/` from the `.tar.xz` archive into
/// `dest`, without the top-level folder. Done in Rust rather than with the
/// `tar` command, which on Windows cannot always read `.xz` files.
fn extract_llvm(archive: &Path, release: &LlvmRelease, dest: &Path) -> Result<usize, String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let xz = liblzma::read::XzDecoder::new_multi_decoder(std::io::BufReader::with_capacity(1 << 20, file));
    let mut tar = tar::Archive::new(xz);
    let mut found: Vec<&String> = Vec::new();
    let mut count = 0;
    let err = |e: std::io::Error| format!("{}: {e}", archive.display());
    for entry in tar.entries().map_err(err)? {
        let mut entry = entry.map_err(err)?;
        let path = entry.path().map_err(err)?.into_owned();
        let Ok(rel) = path.strip_prefix(&release.top) else { continue };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let file = release.files.iter().find(|f| **f == rel_str);
        if file.is_none() && !rel_str.starts_with("lib/clang/") {
            continue;
        }
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        // Regular files keep their permissions, links stay links.
        entry.unpack(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        found.extend(file);
        count += 1;
    }
    let missing: Vec<&String> = release.files.iter().filter(|f| !found.contains(f)).collect();
    if !missing.is_empty() {
        return Err(format!("{} does not contain {missing:?}", archive.display()));
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// icon
// ---------------------------------------------------------------------------

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Signed distance to a rounded rectangle centred at (cx, cy).
fn sd_round_rect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hw + r;
    let qy = (py - cy).abs() - hh + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

fn sd_circle(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r
}

fn blend(dst: &mut [f32; 4], src: [f32; 3], a: f32) {
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + src[i] * a;
    }
    dst[3] = dst[3] + a * (1.0 - dst[3]);
}

/// Draw the GBADE icon: a stylised handheld on a purple tile.
pub fn render_icon(size: u32) -> image::RgbaImage {
    let s = size as f32;
    let mut img = image::RgbaImage::new(size, size);
    let aa = 1.2 / (s / 1024.0).max(0.01);
    for y in 0..size {
        for x in 0..size {
            // Work in a 1024-unit space.
            let px = (x as f32 + 0.5) * 1024.0 / s;
            let py = (y as f32 + 0.5) * 1024.0 / s;
            let cov = |d: f32| 1.0 - smoothstep(-aa, aa, d);
            let mut c = [0.0f32, 0.0, 0.0, 0.0];
            // Background tile with gradient.
            let d_bg = sd_round_rect(px, py, 512.0, 512.0, 430.0, 430.0, 190.0);
            let t = py / 1024.0;
            let top = [128.0, 96.0, 255.0];
            let bot = [64.0, 36.0, 170.0];
            let bg = [top[0] + (bot[0] - top[0]) * t, top[1] + (bot[1] - top[1]) * t, top[2] + (bot[2] - top[2]) * t];
            blend(&mut c, bg, cov(d_bg));
            // Shadow under the console.
            let d_sh = sd_round_rect(px, py, 512.0, 548.0, 360.0, 190.0, 150.0);
            blend(&mut c, [30.0, 15.0, 80.0], 0.35 * (1.0 - smoothstep(-30.0, 30.0, d_sh)) * cov(d_bg));
            // Console body.
            let d_body = sd_round_rect(px, py, 512.0, 520.0, 360.0, 190.0, 150.0);
            blend(&mut c, [236.0, 236.0, 246.0], cov(d_body));
            // Screen bezel and screen.
            let d_bez = sd_round_rect(px, py, 512.0, 505.0, 175.0, 130.0, 22.0);
            blend(&mut c, [52.0, 54.0, 74.0], cov(d_bez));
            let d_scr = sd_round_rect(px, py, 512.0, 505.0, 140.0, 98.0, 8.0);
            blend(&mut c, [24.0, 30.0, 46.0], cov(d_scr));
            // Code brackets "< />" in green pixels on the screen.
            let green = [110.0, 230.0, 140.0];
            let pix = 16.0;
            let glyph = [
                "..X......X...X..",
                ".X......X.....X.",
                "X......X.......X",
                ".X....X.......X.",
                "..X..X.......X..",
            ];
            let gx0 = 512.0 - 8.0 * pix;
            let gy0 = 505.0 - 2.5 * pix;
            for (row, line) in glyph.iter().enumerate() {
                for (col, ch) in line.chars().enumerate() {
                    if ch == 'X' {
                        let cx = gx0 + col as f32 * pix + pix / 2.0;
                        let cy = gy0 + row as f32 * pix + pix / 2.0;
                        let d = sd_round_rect(px, py, cx, cy, pix / 2.0 - 1.0, pix / 2.0 - 1.0, 2.0);
                        blend(&mut c, green, cov(d));
                    }
                }
            }
            // D-pad.
            let dpad = [60.0, 60.0, 80.0];
            let d1 = sd_round_rect(px, py, 245.0, 520.0, 62.0, 20.0, 8.0);
            let d2 = sd_round_rect(px, py, 245.0, 520.0, 20.0, 62.0, 8.0);
            blend(&mut c, dpad, cov(d1.min(d2)));
            // A and B buttons.
            let btn = [124.0, 92.0, 255.0];
            blend(&mut c, btn, cov(sd_circle(px, py, 805.0, 492.0, 30.0)));
            blend(&mut c, btn, cov(sd_circle(px, py, 750.0, 552.0, 30.0)));
            // Shoulder hint lines.
            let d_l = sd_round_rect(px, py, 512.0, 342.0, 250.0, 6.0, 6.0);
            blend(&mut c, [200.0, 200.0, 215.0], cov(d_l) * cov(d_body));

            let a = c[3].clamp(0.0, 1.0);
            img.put_pixel(
                x,
                y,
                image::Rgba([c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, (a * 255.0).round() as u8]),
            );
        }
    }
    img
}

fn icon() -> Result<(), String> {
    let out = root().join("crates/gbade/assets");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    render_icon(512).save(out.join("icon.png")).map_err(|e| e.to_string())?;
    eprintln!("wrote {}", out.join("icon.png").display());
    write_ico(&out.join("icon.ico"))?;
    eprintln!("wrote {}", out.join("icon.ico").display());
    Ok(())
}

/// The Windows icon (embedded in gbade.exe by `crates/gbade/build.rs`):
/// every size Explorer and the taskbar ask for, each drawn at its own size
/// rather than scaled down, stored as PNG frames.
fn write_ico(dest: &Path) -> Result<(), String> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let mut frames = Vec::new();
    for size in [16u32, 20, 24, 32, 40, 48, 64, 96, 128, 256] {
        let img = render_icon(size);
        frames.push(IcoFrame::as_png(img.as_raw(), size, size, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?);
    }
    let file = std::fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    IcoEncoder::new(file).encode_images(&frames).map_err(|e| e.to_string())
}

fn make_icns(dest: &Path) -> Result<(), String> {
    let tmp = root().join("target/AppIcon.iconset");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    for (base, scale) in [(16, 1), (16, 2), (32, 1), (32, 2), (128, 1), (128, 2), (256, 1), (256, 2), (512, 1), (512, 2)] {
        let px = base * scale;
        let name = if scale == 1 { format!("icon_{base}x{base}.png") } else { format!("icon_{base}x{base}@2x.png") };
        render_icon(px).save(tmp.join(name)).map_err(|e| e.to_string())?;
    }
    run(Command::new("iconutil").arg("-c").arg("icns").arg(&tmp).arg("-o").arg(dest))
}

// ---------------------------------------------------------------------------
// bundle
// ---------------------------------------------------------------------------

fn copy_dir(src: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for e in std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        if skip(&p) {
            continue;
        }
        let to = dst.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &to, skip)?;
        } else {
            std::fs::copy(&p, &to).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    Ok(())
}

fn version() -> String {
    let toml = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    toml.lines()
        .find_map(|l| l.strip_prefix("version = \"").map(|r| r.trim_end_matches('"').to_string()))
        .unwrap_or_else(|| "0.0.0".into())
}

/// Compiler headers needed for freestanding ARM C.
fn wanted_header(name: &str) -> bool {
    name.starts_with("std")
        || name.starts_with("__std")
        || name.starts_with("arm_")
        || matches!(name, "limits.h" | "float.h" | "iso646.h" | "varargs.h" | "inttypes.h" | "__float_header_macro.h")
        || name.starts_with("__float")
        || name.starts_with("__limits")
}

/// Fill a resources folder (the layout described in `gbade_toolchain::bundle`):
/// the compiler for this platform, the SDK, examples, docs and licenses.
/// Shared by the macOS app bundle and the single-file Linux build.
fn stage_resources(res: &Path) -> Result<(), String> {
    let root = root();
    let llvm = root.join(".cache/llvm");
    std::fs::create_dir_all(res).map_err(|e| e.to_string())?;

    // Toolchain: clang, ld.lld (real files, named so the drivers pick the
    // right mode: `clang` is the GCC-style driver, `ld.lld` the ELF linker),
    // and the compiler's own headers.
    let tc = res.join("toolchain");
    std::fs::create_dir_all(tc.join("bin")).map_err(|e| e.to_string())?;
    // On macOS and Linux `clang` and `ld.lld` are links in the release:
    // copy the files they point to. Windows has plain `.exe` files.
    // clangd is the C language server behind live errors, completion and
    // hover in the editor.
    let sources = if cfg!(windows) {
        ["clang.exe".to_string(), "ld.lld.exe".into(), "clangd.exe".into()]
    } else {
        [format!("clang-{LLVM_MAJOR}"), "lld".into(), "clangd".into()]
    };
    for (src, dst) in sources.iter().zip(["clang", "ld.lld", "clangd"]) {
        let dst = tc.join("bin").join(gbade_toolchain::paths::exe(dst));
        std::fs::copy(llvm.join("bin").join(src), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    let hdr_src = llvm.join(format!("lib/clang/{LLVM_MAJOR}/include"));
    let hdr_dst = tc.join(format!("lib/clang/{LLVM_MAJOR}/include"));
    std::fs::create_dir_all(&hdr_dst).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(&hdr_src).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if e.path().is_file() && wanted_header(&name) {
            std::fs::copy(e.path(), hdr_dst.join(&name)).map_err(|e| e.to_string())?;
        }
    }

    let no_build = |p: &Path| {
        let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        n == "build" || n.starts_with('.')
    };
    copy_dir(&root.join("sdk"), &res.join("sdk"), &no_build)?;
    copy_dir(&root.join("examples"), &res.join("examples"), &no_build)?;
    copy_dir(&root.join("tutorials"), &res.join("tutorials"), &no_build)?;
    copy_dir(&root.join("docs"), &res.join("docs"), &no_build)?;
    copy_dir(&root.join("licenses"), &res.join("licenses"), &no_build)?;
    for (src, dst) in [("LICENSE-MIT", "GBADE-LICENSE-MIT"), ("LICENSE-APACHE", "GBADE-LICENSE-APACHE")] {
        std::fs::copy(root.join(src), res.join("licenses").join(dst)).map_err(|e| format!("{src}: {e}"))?;
    }
    Ok(())
}

fn bundle() -> Result<PathBuf, String> {
    let root = root();
    if !llvm_clang(&root.join(".cache/llvm")).exists() {
        fetch_llvm()?;
    }
    run(Command::new("cargo").current_dir(&root).args(["build", "--release", "-p", "gbade"]))?;

    let app = root.join("target/bundle/GBADE.app");
    let _ = std::fs::remove_dir_all(&app);
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let res = contents.join("Resources");
    for d in [&macos, &res] {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::copy(root.join("target/release/gbade"), macos.join("gbade")).map_err(|e| e.to_string())?;
    stage_resources(&res)?;
    make_icns(&res.join("AppIcon.icns"))?;
    let tc = res.join("toolchain");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>GBADE</string>
    <key>CFBundleDisplayName</key><string>GBADE</string>
    <key>CFBundleIdentifier</key><string>dev.gbade.ide</string>
    <key>CFBundleExecutable</key><string>gbade</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{v}</string>
    <key>CFBundleVersion</key><string>{v}</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>GBADE contributors. Includes mGBA (MPL-2.0) and LLVM (Apache-2.0 WITH LLVM-exception).</string>
</dict>
</plist>
"#,
        v = version()
    );
    std::fs::write(contents.join("Info.plist"), plist).map_err(|e| e.to_string())?;

    // Ad-hoc signature: inner executables first, then the app.
    for bin in [tc.join("bin/clang"), tc.join("bin/ld.lld"), tc.join("bin/clangd"), macos.join("gbade")] {
        run(Command::new("codesign").args(["--force", "--sign", "-", "--timestamp=none"]).arg(&bin))?;
    }
    run(Command::new("codesign").args(["--force", "--sign", "-", "--timestamp=none"]).arg(&app))?;
    eprintln!("bundle ready: {}", app.display());
    Ok(app)
}

fn dmg() -> Result<(), String> {
    let app = bundle()?;
    let root = root();
    let stage = root.join("target/dmg-stage");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    run(Command::new("cp").arg("-R").arg(&app).arg(&stage))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("/Applications", stage.join("Applications")).map_err(|e| e.to_string())?;
    let out = root.join(format!("target/GBADE-{}.dmg", version()));
    let _ = std::fs::remove_file(&out);
    run(Command::new("hdiutil")
        .args(["create", "-volname", "GBADE", "-fs", "HFS+", "-format", "ULMO", "-ov", "-srcfolder"])
        .arg(&stage)
        .arg(&out))?;
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    eprintln!("DMG ready: {} ({:.1} MB)", out.display(), size as f64 / 1e6);
    Ok(())
}

// ---------------------------------------------------------------------------
// linux: the single self-extracting file
// ---------------------------------------------------------------------------

fn single_file_path() -> PathBuf {
    let os = if cfg!(target_os = "macos") { "macos" } else { std::env::consts::OS };
    root().join(format!("target/gbade-{}-{os}-{}", version(), std::env::consts::ARCH))
}

/// Build the single-file executable for this machine: the release `gbade`
/// binary with the whole resources folder appended (see
/// `gbade_toolchain::bundle`). This is how GBADE ships on Linux; on macOS it
/// produces the same kind of file, which is handy to test the mechanism.
fn linux() -> Result<PathBuf, String> {
    let root = root();
    if !llvm_clang(&root.join(".cache/llvm")).exists() {
        fetch_llvm()?;
    }
    run(Command::new("cargo").current_dir(&root).args(["build", "--release", "-p", "gbade"]))?;
    let stage = root.join("target/single-file-stage");
    let _ = std::fs::remove_dir_all(&stage);
    stage_resources(&stage)?;
    let out = single_file_path();
    eprintln!("compressing resources (zstd -19, takes a minute)…");
    let payload = gbade_toolchain::bundle::write_bundle(&root.join("target/release/gbade"), &stage, &out, 19)
        .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    let total = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "single-file build ready: {} ({:.1} MB, of which {:.1} MB compressed resources)",
        out.display(),
        total as f64 / 1e6,
        payload as f64 / 1e6
    );
    Ok(out)
}

// ---------------------------------------------------------------------------
// windows: the portable ZIP
// ---------------------------------------------------------------------------

/// `target/GBADE-<version>-windows-<arch>.zip`.
fn windows_zip_path() -> PathBuf {
    root().join(format!("target/GBADE-{}-windows-{}.zip", version(), std::env::consts::ARCH))
}

/// The text file at the top of the ZIP, for people who open it before
/// reading any documentation. CRLF line endings for Notepad.
fn windows_readme() -> String {
    let v = version();
    format!(
        "GBADE {v} for Windows\n\
         =====================\n\
         \n\
         Make your own Game Boy Advance games: code editor, C compiler and GBA\n\
         emulator in one program. Nothing else to install.\n\
         \n\
         Start\n\
         -----\n\
         Double-click gbade.exe. Keep this folder together: gbade.exe needs the\n\
         \"resources\" folder next to it (the compiler, the SDK, the examples).\n\
         \n\
         Windows may show \"Windows protected your PC\" the first time, because\n\
         GBADE is not signed yet: click \"More info\", then \"Run anyway\".\n\
         \n\
         Optional: add GBADE to the Start menu by running, in this folder,\n\
         \n\
         \x20   .\\gbade install-desktop\n\
         \n\
         Command line\n\
         ------------\n\
         In cmd or PowerShell, in this folder (or add it to your PATH):\n\
         \n\
         \x20   .\\gbade new my-game --template sprites\n\
         \x20   .\\gbade run my-game --frames 300 --screenshot shot.png\n\
         \n\
         \"gbade\" runs gbade.com, the command-line companion of gbade.exe.\n\
         \n\
         More: resources\\docs\\platforms.md, or the Learn panel in the app.\n\
         License: MIT OR Apache-2.0; third-party licenses in resources\\licenses.\n"
    )
    .replace('\n', "\r\n")
}

/// Build the portable Windows ZIP:
///
/// ```text
/// GBADE-<version>/
///   gbade.exe      the IDE (and the command line, see gbade.com)
///   gbade.com      console companion: `gbade build` in a terminal
///   README.txt
///   resources/     toolchain (clang.exe, ld.lld.exe), sdk, examples, docs, licenses
/// ```
fn windows() -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("the Windows ZIP is built on Windows (it bundles the Windows clang.exe)".into());
    }
    let root = root();
    if !llvm_clang(&root.join(".cache/llvm")).exists() {
        fetch_llvm()?;
    }
    run(Command::new("cargo").current_dir(&root).args(["build", "--release", "-p", "gbade", "-p", "gbade-console"]))?;
    let top = format!("GBADE-{}", version());
    let stage_root = root.join("target/windows-stage");
    let _ = std::fs::remove_dir_all(&stage_root);
    let stage = stage_root.join(&top);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    for (src, dst) in [("gbade.exe", "gbade.exe"), ("gbade-console.exe", "gbade.com")] {
        std::fs::copy(root.join("target/release").join(src), stage.join(dst)).map_err(|e| format!("{src}: {e}"))?;
    }
    std::fs::write(stage.join("README.txt"), windows_readme()).map_err(|e| e.to_string())?;
    stage_resources(&stage.join("resources"))?;

    let out = windows_zip_path();
    eprintln!("compressing {} (a minute)…", out.display());
    write_zip(&stage_root, &out)?;
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    eprintln!("Windows ZIP ready: {} ({:.1} MB)", out.display(), size as f64 / 1e6);
    Ok(out)
}

/// Zip everything inside `dir` (paths in the archive are relative to it,
/// with `/` separators), dated now.
fn write_zip(dir: &Path, out: &Path) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
        entries.sort();
        for p in entries {
            out.push(p.clone());
            if p.is_dir() {
                walk(&p, out)?;
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    walk(dir, &mut paths).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (y, mo, d, h, mi, sec) = utc_now();
    let time = zip::DateTime::from_date_and_time(y, mo, d, h, mi, sec).unwrap_or_default();
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(time);
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let err = |e: zip::result::ZipError| format!("{}: {e}", out.display());
    for p in paths {
        let name = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
        if p.is_dir() {
            zip.add_directory(name, options).map_err(err)?;
        } else {
            zip.start_file(name, options).map_err(err)?;
            let mut f = std::fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            std::io::copy(&mut f, &mut zip).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    zip.finish().map_err(err)?;
    Ok(())
}

/// The current UTC date and time as (year, month, day, hour, minute, second).
fn utc_now() -> (u16, u8, u8, u8, u8, u8) {
    civil(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

/// Seconds since 1970 (UTC) as (year, month, day, hour, minute, second).
fn civil(secs: u64) -> (u16, u8, u8, u8, u8, u8) {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as u16;
    (year, month, day, (rem / 3600) as u8, (rem / 60 % 60) as u8, (rem % 60) as u8)
}

// ---------------------------------------------------------------------------
// smoke: run the packaged CLI on every example
// ---------------------------------------------------------------------------

/// Run a command and return its standard output, failing on a non-zero exit.
fn output(cmd: &mut Command) -> Result<String, String> {
    eprintln!("$ {cmd:?}");
    let o = cmd.output().map_err(|e| format!("cannot run {cmd:?}: {e}"))?;
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    eprint!("{stdout}{}", String::from_utf8_lossy(&o.stderr));
    if o.status.success() { Ok(stdout) } else { Err(format!("command failed ({}): {cmd:?}", o.status)) }
}

/// Run the packaged GBADE on every example: create a project, build it with
/// the bundled compiler, run it headless and save a screenshot. Before that,
/// check with `gbade info` that the package uses its own compiler.
///
/// `cargo xtask smoke` tests the package of the machine it runs on: the
/// macOS app on macOS, the ZIP on Windows (unzipped into a temporary folder
/// and driven through `gbade.com`, like a terminal would), the single file
/// on Linux. `cargo xtask smoke single` forces the single file.
fn smoke(args: &[String]) -> Result<(), String> {
    let root = root();
    let tmp = root.join("target/smoke");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let use_single = args.iter().any(|a| a == "single") || !(cfg!(target_os = "macos") || cfg!(windows));

    let mut env: Vec<(&str, PathBuf)> = vec![("GBADE_CACHE_DIR", tmp.join("cache"))];
    // Where the package's compiler must come from, and a folder to delete
    // at the end.
    let (exe, package_dir, outside) = if use_single {
        let single = single_file_path();
        if !single.exists() {
            return Err(format!("build it first: cargo xtask linux ({} missing)", single.display()));
        }
        // Run from outside the repository, with an empty data folder, so the
        // binary must unpack and use its own compiler.
        let outside = std::env::temp_dir().join(format!("gbade-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).map_err(|e| e.to_string())?;
        let exe = outside.join("gbade");
        std::fs::copy(&single, &exe).map_err(|e| e.to_string())?;
        env.push(("GBADE_DATA_DIR", outside.join("data")));
        (exe, outside.join("data"), Some(outside))
    } else if cfg!(windows) {
        let zip_path = windows_zip_path();
        if !zip_path.exists() {
            return Err(format!("build it first: cargo xtask windows ({} missing)", zip_path.display()));
        }
        // Unzip it like a user would, outside the repository.
        let outside = std::env::temp_dir().join(format!("gbade-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        zip::ZipArchive::new(file).and_then(|mut z| z.extract(&outside)).map_err(|e| format!("cannot unzip: {e}"))?;
        let dir = outside.join(format!("GBADE-{}", version()));
        for f in ["gbade.exe", "gbade.com", "README.txt", "resources/toolchain/bin/clang.exe"] {
            if !dir.join(f).exists() {
                return Err(format!("the ZIP has no {f}"));
            }
        }
        (dir.join("gbade.com"), dir.join("resources"), Some(outside))
    } else {
        let app = root.join("target/bundle/GBADE.app");
        let exe = app.join("Contents/MacOS/gbade");
        if !exe.exists() {
            return Err("bundle first: cargo xtask bundle".into());
        }
        (exe, app, None)
    };
    let command = |args: &[&std::ffi::OsStr]| {
        let mut c = Command::new(&exe);
        c.args(args);
        c.env_remove("GBADE_TOOLCHAIN_DIR").env_remove("GBADE_SDK_DIR").env_remove("GBADE_RESOURCES_DIR");
        for (k, v) in &env {
            c.env(k, v);
        }
        c
    };

    // The command line works: output and exit codes come back (on Windows
    // this goes through gbade.com).
    let version_out = output(&mut command(&["--version".as_ref()]))?;
    if !version_out.contains(&version()) {
        return Err(format!("`gbade --version` printed {version_out:?}"));
    }
    let missing = tmp.join("no-project-here");
    let status = command(&["build".as_ref(), missing.as_os_str()]).status().map_err(|e| e.to_string())?;
    if status.code() != Some(1) {
        return Err(format!("`gbade build` on a folder without a project should exit with 1, got {status}"));
    }

    // The package's own compiler is used, not the repository's.
    let info = output(&mut command(&["info".as_ref()]))?;
    let compiler = info
        .lines()
        .find_map(|l| l.strip_prefix("compiler:"))
        .map(|p| PathBuf::from(p.trim()))
        .ok_or_else(|| format!("`gbade info` shows no compiler:\n{info}"))?;
    let expected = gbade_toolchain::paths::canonical(&package_dir);
    if !compiler.starts_with(&expected) {
        return Err(format!("the package uses {} instead of its own compiler in {}", compiler.display(), expected.display()));
    }

    // Every example and every tutorial is a template: create each one from
    // the package (so a template missing from it fails), check it with the
    // bundled clangd, build and run it.
    let mut names: Vec<String> = Vec::new();
    for folder in ["examples", "tutorials"] {
        let mut found: Vec<String> = std::fs::read_dir(root.join(folder))
            .map_err(|e| e.to_string())?
            .flatten()
            .filter(|e| e.path().join("gbade.toml").exists())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        found.sort();
        names.extend(found);
    }
    for name in &names {
        let dir = tmp.join(name);
        let shot = tmp.join(format!("{name}.png"));
        run(&mut command(&["new".as_ref(), dir.as_os_str(), "--template".as_ref(), name.as_ref()]))?;
        // The bundled code assistant (clangd) must find the SDK: no errors.
        run(&mut command(&["check".as_ref(), dir.as_os_str()]))?;
        run(&mut command(&["run".as_ref(), dir.as_os_str(), "--frames".as_ref(), "60".as_ref(), "--screenshot".as_ref(), shot.as_os_str()]))?;
        if !shot.exists() {
            return Err(format!("no screenshot for {name}"));
        }
    }
    if use_single {
        let unpacked = std::fs::read_dir(&package_dir)
            .map_err(|e| format!("nothing was unpacked into {}: {e}", package_dir.display()))?
            .flatten()
            .any(|e| e.path().join("toolchain/bin/clang").exists());
        if !unpacked {
            return Err(format!("the single file did not unpack its compiler into {}", package_dir.display()));
        }
    }
    if let Some(outside) = outside {
        let _ = std::fs::remove_dir_all(outside);
    }
    eprintln!("smoke test passed on {} examples and tutorials; screenshots in {}", names.len(), tmp.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// gen-assets: pixel art for the examples (kept in code so it is reproducible)
// ---------------------------------------------------------------------------

const HERO_FRAMES: [[&str; 16]; 4] = [
    [
        "................", ".....KKKKKK.....", "....KHHHHHHK....", "...KHHHHHHHHK...",
        "...KHHSSSSSSK...", "...KHSSSSKSSK...", "...KSSSSSKSSK...", "....KSSSSSSSK...",
        ".....KKKKKKK....", "....KRRRRRRRK...", "...KRRRRRRRRRK..", "...KSKRRRRRKSK..",
        "....KBBBBBBBK...", "....KBBBKBBBK...", "....KBBK.KBBK...", "....KOOK.KOOK...",
    ],
    [
        "................", ".....KKKKKK.....", "....KHHHHHHK....", "...KHHHHHHHHK...",
        "...KHHSSSSSSK...", "...KHSSSSKSSK...", "...KSSSSSKSSK...", "....KSSSSSSSK...",
        ".....KKKKKKK....", "....KRRRRRRRK...", "...KRRRRRRRRRK..", "...KSKRRRRRKSK..",
        "....KBBBBBBBK...", "...KBBBK.KBBK...", "..KBBK....KBBK..", "..KOOK....KOOK..",
    ],
    [
        "................", "................", ".....KKKKKK.....", "....KHHHHHHK....",
        "...KHHHHHHHHK...", "...KHHSSSSSSK...", "...KHSSSSKSSK...", "...KSSSSSKSSK...",
        "....KSSSSSSSK...", ".....KKKKKKK....", "....KRRRRRRRK...", "...KSKRRRRRKSK..",
        "....KBBBBBBBK...", "....KBBBKBBK....", ".....KBBKBBK....", ".....KOOKOOK....",
    ],
    [
        "................", ".....KKKKKK.....", "....KHHHHHHK....", "...KHHHHHHHHK...",
        "...KHHSSSSSSK...", "...KHSSSSKSSK...", "...KSSSSSKSSK...", "....KSSSSSSSK...",
        ".....KKKKKKK....", "....KRRRRRRRK...", "...KRRRRRRRRRK..", "...KSKRRRRRKSK..",
        "....KBBBBBBBK...", "....KBBK.KBBBK..", "...KBBK...KBBK..", "...KOOK...KOOK..",
    ],
];

fn hero_color(c: char) -> [u8; 4] {
    match c {
        'K' => [24, 20, 37, 255],
        'H' => [124, 74, 36, 255],
        'S' => [255, 205, 160, 255],
        'R' => [222, 64, 64, 255],
        'B' => [64, 88, 208, 255],
        'O' => [92, 52, 24, 255],
        'W' => [255, 255, 255, 255],
        _ => [0, 0, 0, 0],
    }
}

fn gen_hero() -> image::RgbaImage {
    let mut img = image::RgbaImage::new(64, 16);
    for (f, frame) in HERO_FRAMES.iter().enumerate() {
        for (y, row) in frame.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                img.put_pixel((f * 16 + x) as u32, y as u32, image::Rgba(hero_color(c)));
            }
        }
    }
    img
}

/// A 512x256 side-scrolling world: sky, clouds, hills, grass and dirt.
/// Patterns repeat on tile boundaries so the tile set stays small.
fn gen_world() -> image::RgbaImage {
    let (w, h) = (512u32, 256u32);
    let sky = [[92, 148, 252], [110, 166, 255], [140, 190, 255]];
    let cloud = [250, 250, 255];
    let cloud_shade = [214, 226, 248];
    let hill = [[64, 168, 88], [48, 136, 72]];
    let grass = [[88, 200, 72], [56, 160, 56]];
    let dirt = [[150, 96, 52], [122, 74, 40]];
    let flower = [[255, 230, 80], [240, 90, 120]];
    let mut img = image::RgbaImage::new(w, h);
    // Hill height profile with a 128-pixel period.
    let hill_top = |x: u32| -> u32 {
        let t = (x % 128) as f32 / 128.0 * std::f32::consts::TAU;
        (150.0 - 28.0 * (0.5 - 0.5 * t.cos())) as u32 / 4 * 4
    };
    for y in 0..h {
        for x in 0..w {
            let mut c = sky[((y / 48) as usize).min(2)];
            // Clouds: rounded blobs at fixed places (aligned to 8 pixels).
            for &(cx, cy, r) in &[(64i32, 40i32, 14i32), (88, 36, 18), (112, 42, 12), (296, 56, 14), (320, 50, 18), (344, 58, 12), (440, 30, 12), (460, 26, 16)] {
                let dx = x as i32 - cx;
                let dy = y as i32 - cy;
                if dx * dx + dy * dy <= r * r && dy < r / 2 {
                    c = if dy > r / 4 { cloud_shade } else { cloud };
                }
            }
            if y >= hill_top(x) {
                c = hill[((x / 8 + y / 8) % 2) as usize];
            }
            if y >= 192 {
                c = if y < 200 {
                    grass[((x / 4) % 2) as usize]
                } else {
                    dirt[(((x / 8) + (y / 8)) % 2) as usize]
                };
                // A few flowers on the grass.
                if (184..192).contains(&y) && (x % 64 == 20 || x % 64 == 44) {
                    c = flower[((x / 64) % 2) as usize];
                }
            } else if (186..192).contains(&y) && (x % 64 == 20 || x % 64 == 44) {
                c = flower[((x / 64) % 2) as usize];
            }
            img.put_pixel(x, y, image::Rgba([c[0], c[1], c[2], 255]));
        }
    }
    img
}

fn gen_assets() -> Result<(), String> {
    let r = root();
    let targets = [
        (r.join("examples/sprites/assets/hero.16x16.png"), gen_hero()),
        (r.join("examples/scrolling/assets/hero.16x16.png"), gen_hero()),
        (r.join("examples/scrolling/assets/backgrounds/world.png"), gen_world()),
    ];
    for (path, img) in targets {
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        img.save(&path).map_err(|e| e.to_string())?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small `.tar.xz` shaped like an LLVM release.
    fn fake_release(dir: &Path, top: &str, files: &[(&str, &str)]) -> PathBuf {
        let path = dir.join(format!("{top}.tar.xz"));
        let xz = liblzma::write::XzEncoder::new(std::fs::File::create(&path).unwrap(), 1);
        let mut tar = tar::Builder::new(xz);
        for (name, content) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(content.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            tar.append_data(&mut h, format!("{top}/{name}"), content.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        path
    }

    #[test]
    fn extract_keeps_only_the_compiler_linker_and_headers() {
        let dir = tempfile::tempdir().unwrap();
        let top = "clang+llvm-0.0.0-x86_64-pc-windows-msvc";
        let archive = fake_release(
            dir.path(),
            top,
            &[
                ("bin/clang.exe", "clang"),
                ("bin/ld.lld.exe", "lld"),
                ("bin/lld.exe", "lld copy"),
                ("bin/clang-tidy.exe", "not needed"),
                ("lib/clang/23/include/stdint.h", "// stdint"),
                ("lib/libLLVM.a", "not needed"),
            ],
        );
        let release = LlvmRelease { top: top.into(), files: vec!["bin/clang.exe".into(), "bin/ld.lld.exe".into()] };
        let dest = dir.path().join("llvm");
        assert_eq!(extract_llvm(&archive, &release, &dest).unwrap(), 3);
        assert_eq!(std::fs::read_to_string(dest.join("bin/clang.exe")).unwrap(), "clang");
        assert_eq!(std::fs::read_to_string(dest.join("lib/clang/23/include/stdint.h")).unwrap(), "// stdint");
        assert!(!dest.join("bin/lld.exe").exists());
        assert!(!dest.join("bin/clang-tidy.exe").exists());
        assert!(!dest.join("lib/libLLVM.a").exists());
    }

    #[test]
    fn extract_reports_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let archive = fake_release(dir.path(), "LLVM-0-X", &[("bin/clang", "clang")]);
        let release = LlvmRelease { top: "LLVM-0-X".into(), files: vec!["bin/clang".into(), "bin/ld.lld".into()] };
        let err = extract_llvm(&archive, &release, &dir.path().join("llvm")).unwrap_err();
        assert!(err.contains("bin/ld.lld"), "{err}");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil(951_782_400), (2000, 2, 29, 0, 0, 0)); // leap day
        assert_eq!(civil(1_794_783_599), (2026, 11, 15, 22, 59, 59));
    }

    #[test]
    fn windows_readme_is_for_notepad() {
        let r = windows_readme();
        assert!(r.contains("\r\n") && !r.replace("\r\n", "").contains('\n'), "CRLF line endings only");
        assert!(r.contains("gbade.exe") && r.contains(".\\gbade install-desktop"));
        assert!(r.contains(&format!("GBADE {}", version())));
    }

    #[test]
    fn zip_keeps_the_folder_layout() {
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().join("stage");
        std::fs::create_dir_all(stage.join("GBADE-1/resources/sdk")).unwrap();
        std::fs::write(stage.join("GBADE-1/gbade.exe"), "exe").unwrap();
        std::fs::write(stage.join("GBADE-1/resources/sdk/gba.h"), "h").unwrap();
        let out = dir.path().join("out.zip");
        write_zip(&stage, &out).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["GBADE-1/", "GBADE-1/gbade.exe", "GBADE-1/resources/", "GBADE-1/resources/sdk/", "GBADE-1/resources/sdk/gba.h"]);
        let unzipped = dir.path().join("unzipped");
        z.extract(&unzipped).unwrap();
        assert_eq!(std::fs::read_to_string(unzipped.join("GBADE-1/resources/sdk/gba.h")).unwrap(), "h");
    }

    #[test]
    fn every_supported_platform_has_a_release() {
        // The machine running the tests is one of the supported platforms.
        let r = llvm_release().unwrap();
        assert!(r.files.iter().any(|f| f.starts_with("bin/clang")));
        assert!(r.files.iter().any(|f| f.starts_with("bin/ld.lld")));
    }
}
