//! Developer tasks for GBADE. Run with `cargo xtask <command>`:
//!
//! - `fetch-llvm`  download the official LLVM release and extract clang, lld
//!                 and the compiler headers into `.cache/llvm`
//! - `icon`        regenerate the app icon (`crates/gbade/assets/icon.png`)
//! - `bundle`      build `target/bundle/GBADE.app` (release build + toolchain
//!                 + SDK + examples + docs), ad-hoc signed
//! - `dmg`         bundle, then create `target/GBADE-<version>.dmg`
//! - `linux`       build the single-file Linux executable
//!                 `target/gbade-<version>-linux-<arch>` (IDE + compiler + SDK)
//! - `smoke`       run the bundled app's CLI on every example (sanity check)
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
        Some("smoke") => smoke(&args[1..]),
        Some("gen-assets") => gen_assets(),
        _ => {
            eprintln!("usage: cargo xtask <fetch-llvm|icon|bundle|dmg|linux|smoke|gen-assets>");
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

/// The official LLVM release archive for the machine we build on. GBADE
/// bundles the compiler of the platform it ships for, so this is also the
/// platform of the packaged app.
fn llvm_release_name() -> Result<String, String> {
    let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macOS-ARM64",
        ("linux", "x86_64") => "Linux-X64",
        ("linux", "aarch64") => "Linux-ARM64",
        (os, arch) => return Err(format!("no official LLVM {LLVM_VERSION} release for {os}/{arch}")),
    };
    Ok(format!("LLVM-{LLVM_VERSION}-{platform}"))
}

fn fetch_llvm() -> Result<(), String> {
    let cache = root().join(".cache");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let name = llvm_release_name()?;
    let archive = cache.join(format!("{name}.tar.xz"));
    let dest = cache.join("llvm");
    // Already extracted (e.g. restored from the CI cache): skip the 1.5 GB download.
    if dest.join("bin/clang").exists() {
        eprintln!("{} already extracted", dest.display());
        return Ok(());
    }
    if !archive.exists() {
        let url = format!("https://github.com/llvm/llvm-project/releases/download/llvmorg-{LLVM_VERSION}/{name}.tar.xz");
        let part = archive.with_extension("part");
        run(Command::new("curl").args(["-fL", "--progress-bar", "-o"]).arg(&part).arg(&url))?;
        std::fs::rename(&part, &archive).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let members = [
        format!("{name}/bin/clang-{LLVM_MAJOR}"),
        format!("{name}/bin/clang"),
        format!("{name}/bin/lld"),
        format!("{name}/bin/ld.lld"),
        format!("{name}/bin/llvm-objdump"),
        format!("{name}/lib/clang"),
    ];
    run(Command::new("tar").arg("-xJf").arg(&archive).arg("-C").arg(&dest).arg("--strip-components=1").args(&members))?;
    eprintln!("LLVM extracted to {}", dest.display());
    Ok(())
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
    Ok(())
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
    // right mode), and the compiler's own headers.
    let tc = res.join("toolchain");
    std::fs::create_dir_all(tc.join("bin")).map_err(|e| e.to_string())?;
    std::fs::copy(llvm.join(format!("bin/clang-{LLVM_MAJOR}")), tc.join("bin/clang")).map_err(|e| e.to_string())?;
    std::fs::copy(llvm.join("bin/lld"), tc.join("bin/ld.lld")).map_err(|e| e.to_string())?;
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
    copy_dir(&root.join("docs"), &res.join("docs"), &no_build)?;
    copy_dir(&root.join("licenses"), &res.join("licenses"), &no_build)?;
    for (src, dst) in [("LICENSE-MIT", "GBADE-LICENSE-MIT"), ("LICENSE-APACHE", "GBADE-LICENSE-APACHE")] {
        std::fs::copy(root.join(src), res.join("licenses").join(dst)).map_err(|e| format!("{src}: {e}"))?;
    }
    Ok(())
}

fn bundle() -> Result<PathBuf, String> {
    let root = root();
    let llvm = root.join(".cache/llvm");
    if !llvm.join("bin/clang").exists() {
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
    for bin in [tc.join("bin/clang"), tc.join("bin/ld.lld"), macos.join("gbade")] {
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
// smoke: run the bundled CLI on every example
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
    if !root.join(".cache/llvm/bin/clang").exists() {
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

/// Run the packaged GBADE on every example: create a project, build it with
/// the bundled compiler, run it headless and save a screenshot.
/// `cargo xtask smoke` tests the macOS app on macOS and the single file on
/// Linux; `cargo xtask smoke single` forces the single file.
fn smoke(args: &[String]) -> Result<(), String> {
    let root = root();
    let tmp = root.join("target/smoke");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let use_single = !cfg!(target_os = "macos") || args.iter().any(|a| a == "single");

    let mut env: Vec<(&str, PathBuf)> = vec![("GBADE_CACHE_DIR", tmp.join("cache"))];
    let exe = if use_single {
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
        exe
    } else {
        let exe = root.join("target/bundle/GBADE.app/Contents/MacOS/gbade");
        if !exe.exists() {
            return Err("bundle first: cargo xtask bundle".into());
        }
        exe
    };

    let mut names: Vec<String> = std::fs::read_dir(root.join("examples"))
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().join("gbade.toml").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in &names {
        let dir = tmp.join(name);
        let mut new = Command::new(&exe);
        new.arg("new").arg(&dir).args(["--template", name]);
        let mut runc = Command::new(&exe);
        runc.arg("run").arg(&dir).args(["--frames", "60", "--screenshot"]).arg(tmp.join(format!("{name}.png")));
        for c in [&mut new, &mut runc] {
            c.env_remove("GBADE_TOOLCHAIN_DIR").env_remove("GBADE_SDK_DIR").env_remove("GBADE_RESOURCES_DIR");
            for (k, v) in &env {
                c.env(k, v);
            }
        }
        run(&mut new)?;
        run(&mut runc)?;
    }
    if use_single {
        let data = env.iter().find(|(k, _)| *k == "GBADE_DATA_DIR").unwrap().1.clone();
        let unpacked = std::fs::read_dir(&data)
            .map_err(|e| format!("nothing was unpacked into {}: {e}", data.display()))?
            .flatten()
            .any(|e| e.path().join("toolchain/bin/clang").exists());
        if !unpacked {
            return Err(format!("the single file did not unpack its compiler into {}", data.display()));
        }
        let _ = std::fs::remove_dir_all(data.parent().unwrap());
    }
    eprintln!("smoke test passed on {} examples; screenshots in {}", names.len(), tmp.display());
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
