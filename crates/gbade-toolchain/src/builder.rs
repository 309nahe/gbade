//! The build pipeline: assets -> compile -> link -> ROM.
//!
//! 1. PNG files in `assets/` become `build/gen/assets.{c,h}`.
//! 2. The GBADE runtime (startup code, libc, the gba.h library, compiler
//!    helpers) is compiled ONCE and cached in a per-user cache directory.
//! 3. Your `.c`/`.s` files are compiled in parallel. Only files that changed
//!    (or whose headers changed) are recompiled.
//! 4. Everything is linked with `ld.lld` using `sdk/runtime/gba.ld`.
//! 5. The ELF is converted into a `.gba` ROM with a valid header.

use crate::assets;
use crate::debuginfo::DebugInfo;
use crate::diagnostics::{self, Diagnostic, Severity};
use crate::project::{Project, SourceKind};
use crate::rom::{self, HeaderInfo};
use crate::toolchain::Toolchain;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// Options for a build.
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Where to cache the compiled runtime. Default: `~/Library/Caches/gbade`
    /// on macOS (or `$GBADE_CACHE_DIR`).
    pub cache_dir: Option<PathBuf>,
    /// Number of parallel compiler processes (0 = number of CPU cores).
    pub jobs: usize,
    /// Ignore up-to-date checks and recompile everything.
    pub force: bool,
}

/// How much of each memory the game uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryUsage {
    pub rom: u32,
    pub iwram: u32,
    pub ewram: u32,
}

impl MemoryUsage {
    pub const ROM_TOTAL: u32 = 32 * 1024 * 1024;
    /// IWRAM is 32 KB, but the stacks live at the top.
    pub const IWRAM_TOTAL: u32 = 32 * 1024;
    pub const EWRAM_TOTAL: u32 = 256 * 1024;
}

/// Everything a build produced.
#[derive(Debug, Clone, Default)]
pub struct BuildOutput {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    /// Human-readable log of what happened (shown in the IDE's Build panel).
    pub log: String,
    pub rom_path: Option<PathBuf>,
    pub elf_path: Option<PathBuf>,
    pub memory: Option<MemoryUsage>,
    pub duration: Duration,
    /// Number of project files actually compiled (0 = all up to date).
    pub compiled: usize,
}

impl BuildOutput {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning)
    }
    fn fail(mut self, start: Instant, msg: impl Into<String>) -> BuildOutput {
        let d = Diagnostic::error(msg);
        self.log.push_str(&format!("{}\n", d.summary()));
        self.diagnostics.push(d);
        self.success = false;
        self.duration = start.elapsed();
        self
    }
}

const ARCH_FLAGS: &[&str] = &["--target=arm-none-eabi", "-mcpu=arm7tdmi", "-mfloat-abi=soft"];
const CODEGEN_FLAGS: &[&str] = &[
    "-mthumb",
    "-g",
    "-ffunction-sections",
    "-fdata-sections",
    "-fno-common",
    "-fno-exceptions",
    "-fno-unwind-tables",
    "-fno-asynchronous-unwind-tables",
    "-fno-color-diagnostics",
    "-fdiagnostics-absolute-paths",
    "-fmessage-length=0",
    "-nostdlibinc",
    "-D__GBA__",
    "-DGBADE=1",
];

/// The default cache directory.
pub fn default_cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GBADE_CACHE_DIR") {
        return PathBuf::from(d);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            return p.join("Library").join("Caches").join("gbade");
        }
        return p.join(".cache").join("gbade");
    }
    std::env::temp_dir().join("gbade-cache")
}

/// Build a project.
pub fn build(project: &Project, tc: &Toolchain, opts: &BuildOptions) -> BuildOutput {
    let start = Instant::now();
    let mut out = BuildOutput::default();
    let build_dir = project.build_dir();
    let obj_dir = build_dir.join("obj");
    let gen_dir = build_dir.join("gen");
    for d in [&build_dir, &obj_dir, &gen_dir] {
        if let Err(e) = std::fs::create_dir_all(d) {
            return out.fail(start, format!("cannot create {}: {e}", d.display()));
        }
    }
    out.log.push_str(&format!("Building {} ...\n", project.config.game.name));

    // ---- 1. Assets ---------------------------------------------------------
    let asset_files = project.assets();
    let converted = assets::convert_all(&project.assets_dir(), &asset_files);
    for e in &converted.errors {
        let mut d = Diagnostic::error(format!("asset {}", e.message));
        d.file = Some(e.file.clone());
        out.log.push_str(&format!("{}\n", d.summary()));
        out.diagnostics.push(d);
    }
    if !converted.images.is_empty() {
        out.log.push_str(&format!("Converted {} image(s) from assets/\n", converted.images.len()));
    }
    write_if_changed(&gen_dir.join("assets.h"), &converted.header);
    write_if_changed(&gen_dir.join("assets.c"), &converted.source);
    if !converted.errors.is_empty() {
        out.duration = start.elapsed();
        return out;
    }

    // ---- 2. Runtime --------------------------------------------------------
    let cache_dir = opts.cache_dir.clone().unwrap_or_else(default_cache_dir);
    let runtime = match ensure_runtime(tc, &cache_dir, opts) {
        Ok((objs, built)) => {
            if built {
                out.log.push_str("Compiled the GBADE runtime library (cached for next time)\n");
            }
            objs
        }
        Err(log) => {
            out.log.push_str(&log);
            let mut diags = diagnostics::parse_clang(&log);
            if diags.is_empty() {
                diags.push(Diagnostic::error("failed to compile the GBADE runtime library"));
            }
            out.diagnostics.extend(diags);
            out.duration = start.elapsed();
            return out;
        }
    };

    // ---- 3. Compile the project ---------------------------------------------
    let mut cflags: Vec<String> = ARCH_FLAGS.iter().chain(CODEGEN_FLAGS).map(|s| s.to_string()).collect();
    let opt = match project.config.build.optimization.as_str() {
        o @ ("O0" | "O1" | "O2" | "O3" | "Os" | "Oz" | "Og") => format!("-{o}"),
        other => {
            out.log.push_str(&format!("warning: unknown optimization '{other}', using O2\n"));
            "-O2".into()
        }
    };
    cflags.push(opt);
    cflags.push("-Wall".into());
    if project.config.build.werror {
        cflags.push("-Werror".into());
    }
    for inc in [tc.include_dir()] {
        cflags.push("-isystem".into());
        cflags.push(inc.display().to_string());
    }
    for inc in [project.include_dir(), project.src_dir(), gen_dir.clone()] {
        cflags.push(format!("-I{}", inc.display()));
    }
    for d in &project.config.build.defines {
        cflags.push(format!("-D{d}"));
    }
    cflags.extend(project.config.build.cflags.iter().cloned());

    // If the flags changed since last build, everything must be recompiled.
    let flags_file = obj_dir.join("flags.txt");
    let flags_text = cflags.join("\n");
    let flags_changed = std::fs::read_to_string(&flags_file).map(|t| t != flags_text).unwrap_or(true);

    let mut jobs: Vec<CompileJob> = Vec::new();
    let mut objects: Vec<PathBuf> = Vec::new();
    let mut sources = project.sources();
    sources.push((gen_dir.join("assets.c"), SourceKind::C));
    if project.sources().is_empty() {
        return out.fail(start, "no source files: add a .c file in the src/ folder (for example src/main.c)");
    }
    for (src, kind) in sources {
        let rel = src
            .strip_prefix(&project.root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| PathBuf::from(src.file_name().unwrap()));
        let flat = rel.to_string_lossy().replace(['/', '\\'], "__");
        let obj = obj_dir.join(format!("{flat}.o"));
        let dep = obj_dir.join(format!("{flat}.d"));
        objects.push(obj.clone());
        if opts.force || flags_changed || !up_to_date(&obj, &dep) {
            jobs.push(CompileJob { src, obj, dep, kind, display: rel.display().to_string() });
        }
    }

    out.compiled = jobs.len();
    let results = run_jobs(tc, &jobs, &cflags, opts.jobs);
    let mut failed = false;
    for (job, ok, text) in results {
        out.log.push_str(&format!("Compiling {}\n", job.display));
        if !text.trim().is_empty() {
            out.log.push_str(&text);
            if !text.ends_with('\n') {
                out.log.push('\n');
            }
        }
        out.diagnostics.extend(diagnostics::parse_clang(&text));
        if !ok {
            failed = true;
            let _ = std::fs::remove_file(&job.obj);
        }
    }
    if failed {
        if !out.errors().any(|_| true) {
            out.diagnostics.push(Diagnostic::error("compilation failed (see the build log)"));
        }
        out.log.push_str("Build FAILED.\n");
        out.duration = start.elapsed();
        return out;
    }
    let _ = std::fs::write(&flags_file, &flags_text);

    // ---- 4. Link -----------------------------------------------------------
    let elf = project.elf_path();
    let map = build_dir.join(format!("{}.map", project.slug()));
    let mut link = Command::new(&tc.lld);
    link.arg("-T").arg(tc.linker_script()).arg("--gc-sections").arg("-Map").arg(&map).arg("-o").arg(&elf);
    link.args(&runtime).args(&objects);
    out.log.push_str("Linking\n");
    match link.output() {
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            if !text.trim().is_empty() {
                out.log.push_str(&text);
            }
            let diags = diagnostics::parse_lld(&text);
            let has_err = diags.iter().any(|d| d.severity == Severity::Error);
            out.diagnostics.extend(diags);
            if !o.status.success() {
                if !has_err {
                    out.diagnostics.push(Diagnostic::error("linking failed (see the build log)"));
                }
                out.log.push_str("Build FAILED.\n");
                out.duration = start.elapsed();
                return out;
            }
        }
        Err(e) => return out.fail(start, format!("cannot run the linker {}: {e}", tc.lld.display())),
    }

    // ---- 5. ROM ------------------------------------------------------------
    let elf_data = match std::fs::read(&elf) {
        Ok(d) => d,
        Err(e) => return out.fail(start, format!("cannot read {}: {e}", elf.display())),
    };
    let header = HeaderInfo {
        title: project.config.game.title.clone(),
        game_code: project.config.game.code.clone(),
        maker_code: project.config.game.maker.clone(),
        version: project.config.game.version,
    };
    let rom_data = match rom::elf_to_rom(&elf_data, &header) {
        Ok(r) => r,
        Err(e) => return out.fail(start, e.0),
    };
    let rom_path = project.rom_path();
    if let Err(e) = std::fs::write(&rom_path, &rom_data) {
        return out.fail(start, format!("cannot write {}: {e}", rom_path.display()));
    }

    let mem = match DebugInfo::from_elf(&elf_data) {
        Ok(info) => MemoryUsage {
            rom: rom_data.len() as u32,
            iwram: info.section_size(".iwram") + info.section_size(".bss"),
            ewram: info.section_size(".ewram") + info.section_size(".sbss"),
        },
        Err(_) => MemoryUsage { rom: rom_data.len() as u32, ..Default::default() },
    };
    out.log.push_str(&format!(
        "ROM {} ({}), IWRAM {} / 32 KB, EWRAM {} / 256 KB\n",
        rom_path.file_name().unwrap().to_string_lossy(),
        human_size(mem.rom),
        human_size(mem.iwram),
        human_size(mem.ewram)
    ));
    out.memory = Some(mem);
    out.rom_path = Some(rom_path);
    out.elf_path = Some(elf);
    out.success = true;
    out.duration = start.elapsed();
    let warnings = out.warnings().count();
    out.log.push_str(&format!(
        "Build succeeded in {:.2}s{}\n",
        out.duration.as_secs_f32(),
        if warnings > 0 { format!(" with {warnings} warning(s)") } else { String::new() }
    ));
    out
}

/// "1.5 KB", "12 bytes"...
pub fn human_size(n: u32) -> String {
    if n < 1024 {
        format!("{n} bytes")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    }
}

fn write_if_changed(path: &Path, content: &str) {
    if std::fs::read_to_string(path).map(|c| c != content).unwrap_or(true) {
        let _ = std::fs::write(path, content);
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// An object is up to date if it is newer than every file listed in its
/// dependency file (written by clang's -MMD).
fn up_to_date(obj: &Path, dep: &Path) -> bool {
    let (Some(obj_time), Ok(deps)) = (mtime(obj), std::fs::read_to_string(dep)) else {
        return false;
    };
    for file in parse_depfile(&deps) {
        match mtime(Path::new(&file)) {
            Some(t) if t <= obj_time => {}
            _ => return false,
        }
    }
    true
}

/// Parse a Makefile-style dependency file: "obj.o: a.c b.h \\\n c.h".
pub fn parse_depfile(text: &str) -> Vec<String> {
    let joined = text.replace("\\\n", " ");
    let Some((_, deps)) = joined.split_once(": ") else { return Vec::new() };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = deps.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&' ') => {
                cur.push(' ');
                chars.next();
            }
            ' ' | '\n' | '\t' | '\r' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Debug, Clone)]
struct CompileJob {
    src: PathBuf,
    obj: PathBuf,
    dep: PathBuf,
    kind: SourceKind,
    display: String,
}

fn compile_one(tc: &Toolchain, job: &CompileJob, flags: &[String]) -> (bool, String) {
    let mut cmd = Command::new(&tc.clang);
    cmd.args(flags);
    if job.kind == SourceKind::Asm {
        cmd.arg("-Wno-unused-command-line-argument");
    }
    cmd.arg("-MMD").arg("-MF").arg(&job.dep).arg("-c").arg(&job.src).arg("-o").arg(&job.obj);
    match cmd.output() {
        Ok(o) => (
            o.status.success(),
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
        ),
        Err(e) => (false, format!("clang: error: cannot run {}: {e}\n", tc.clang.display())),
    }
}

/// Run compile jobs on a pool of threads. Results keep the job order.
fn run_jobs(tc: &Toolchain, jobs: &[CompileJob], flags: &[String], n: usize) -> Vec<(CompileJob, bool, String)> {
    let n = if n == 0 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) } else { n };
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<Option<(bool, String)>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..n.min(jobs.len()).max(1) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut g = next.lock().unwrap();
                        let i = *g;
                        *g += 1;
                        i
                    };
                    if i >= jobs.len() {
                        break;
                    }
                    let r = compile_one(tc, &jobs[i], flags);
                    results.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    jobs.iter()
        .cloned()
        .zip(results)
        .map(|(j, r)| {
            let (ok, t) = r.unwrap_or((false, "not compiled".into()));
            (j, ok, t)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// A tiny, dependency-free 64-bit FNV-1a hash (only used for cache keys).
fn fnv1a(h: &mut u64, data: &[u8]) {
    for &b in data {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

/// All runtime source files with the extra flags they need.
fn runtime_sources(sdk: &Path) -> Vec<(PathBuf, Vec<&'static str>)> {
    let mut v = Vec::new();
    let rt = sdk.join("runtime");
    // crt0 must come first: it contains the ROM header at 0x08000000.
    for f in ["crt0.s", "irq.s", "aeabi_divmod.s"] {
        v.push((rt.join(f), vec!["-Wno-unused-command-line-argument"]));
    }
    v.push((rt.join("aeabi_glue.c"), vec!["-ffreestanding", "-fno-builtin"]));
    v.push((rt.join("printf").join("printf.c"), vec!["-ffreestanding", "-fno-builtin", "-w"]));
    let mut push_dir = |dir: PathBuf, extra: Vec<&'static str>| {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "c")).collect())
            .unwrap_or_default();
        files.sort();
        for f in files {
            v.push((f, extra.clone()));
        }
    };
    push_dir(rt.join("compiler-rt"), vec!["-ffreestanding", "-fno-builtin", "-w"]);
    push_dir(sdk.join("libc"), vec!["-ffreestanding", "-fno-builtin", "-Wall"]);
    push_dir(sdk.join("lib"), vec!["-Wall"]);
    v
}

fn hash_dir(h: &mut u64, dir: &Path) {
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            hash_dir(h, &p);
        } else if let Ok(data) = std::fs::read(&p) {
            fnv1a(h, p.file_name().unwrap().as_encoded_bytes());
            fnv1a(h, &data);
        }
    }
}

/// Compile the runtime into the cache if needed. Returns the object files
/// (crt0 first) and whether they were just built.
pub fn ensure_runtime(tc: &Toolchain, cache_dir: &Path, opts: &BuildOptions) -> Result<(Vec<PathBuf>, bool), String> {
    let sources = runtime_sources(&tc.sdk);
    let mut h: u64 = 0xcbf29ce484222325;
    for d in ["runtime", "lib", "libc", "include"] {
        hash_dir(&mut h, &tc.sdk.join(d));
    }
    fnv1a(&mut h, tc.clang.as_os_str().as_encoded_bytes());
    if let Ok(m) = std::fs::metadata(&tc.clang) {
        fnv1a(&mut h, &m.len().to_le_bytes());
    }
    fnv1a(&mut h, ARCH_FLAGS.join(" ").as_bytes());
    fnv1a(&mut h, CODEGEN_FLAGS.join(" ").as_bytes());
    let dir = cache_dir.join(format!("runtime-{h:016x}"));
    let objects: Vec<PathBuf> = sources
        .iter()
        .enumerate()
        .map(|(i, (src, _))| dir.join(format!("{i:03}_{}.o", src.file_name().unwrap().to_string_lossy())))
        .collect();
    if dir.join("ok").exists() && !opts.force {
        return Ok((objects, false));
    }

    // Build into a temporary directory, then move it in place, so that two
    // GBADE builds running at the same time never see half-built files.
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("cannot create {}: {e}", cache_dir.display()))?;
    let tmp = cache_dir.join(format!("tmp-{}-{:x}", std::process::id(), h ^ now_nanos()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let mut base: Vec<String> = ARCH_FLAGS.iter().chain(CODEGEN_FLAGS).map(|s| s.to_string()).collect();
    base.push("-O2".into());
    base.push("-isystem".into());
    base.push(tc.include_dir().display().to_string());

    let next = Mutex::new(0usize);
    let errors: Mutex<String> = Mutex::new(String::new());
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut g = next.lock().unwrap();
                        let i = *g;
                        *g += 1;
                        i
                    };
                    if i >= sources.len() {
                        break;
                    }
                    let (src, extra) = &sources[i];
                    let obj = tmp.join(objects[i].file_name().unwrap());
                    let o = Command::new(&tc.clang).args(&base).args(extra).arg("-c").arg(src).arg("-o").arg(&obj).output();
                    match o {
                        Ok(o) if o.status.success() => {}
                        Ok(o) => {
                            errors.lock().unwrap().push_str(&String::from_utf8_lossy(&o.stderr));
                        }
                        Err(e) => errors.lock().unwrap().push_str(&format!("clang: error: cannot run clang: {e}\n")),
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(errors);
    }
    std::fs::write(tmp.join("ok"), "").map_err(|e| e.to_string())?;
    if dir.exists() && opts.force {
        let _ = std::fs::remove_dir_all(&dir);
    }
    if std::fs::rename(&tmp, &dir).is_err() {
        // Someone else finished first: use theirs.
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join("ok").exists() {
            return Err(format!("could not install the runtime into {}", dir.display()));
        }
    }
    Ok((objects, true))
}

fn now_nanos() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depfile_parsing() {
        let d = "build/obj/a.o: /p/src/a.c /p/src/my\\ header.h \\\n  /sdk/include/gba.h\n";
        assert_eq!(parse_depfile(d), vec!["/p/src/a.c", "/p/src/my header.h", "/sdk/include/gba.h"]);
        assert!(parse_depfile("garbage").is_empty());
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(12), "12 bytes");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.00 MB");
    }

    #[test]
    fn runtime_crt0_first() {
        let tc = Toolchain::locate().unwrap();
        let s = runtime_sources(&tc.sdk);
        assert!(s[0].0.ends_with("crt0.s"));
        assert!(s.len() > 50);
    }
}
