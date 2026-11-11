//! GBADE projects: a folder with a `gbade.toml` file.
//!
//! ```text
//! my-game/
//! ├── gbade.toml        project settings (name, ROM title...)
//! ├── src/              your C (.c) and assembly (.s) files, any depth
//! │   └── main.c
//! ├── include/          optional: your own headers (added to the include path)
//! ├── assets/           optional: PNG images, converted to C automatically
//! └── build/            created by GBADE: objects, the .elf and the .gba ROM
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Name of the project file.
pub const PROJECT_FILE: &str = "gbade.toml";

/// Contents of `gbade.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub game: GameConfig,
    pub build: BuildConfig,
}

/// The `[game]` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    /// Human-readable name, also used for the ROM file name.
    pub name: String,
    /// Title stored in the ROM header (max 12 characters, uppercase ASCII).
    pub title: String,
    /// 4-character game code stored in the ROM header.
    pub code: String,
    /// 2-character maker code.
    pub maker: String,
    /// Software version byte.
    pub version: u8,
}

/// The `[build]` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    /// Optimisation level: "O0", "O1", "O2", "O3", "Os" or "Oz".
    pub optimization: String,
    /// Extra flags passed to clang for every C file, e.g. ["-Wextra"].
    pub cflags: Vec<String>,
    /// Extra preprocessor definitions, e.g. ["DEBUG=1"].
    pub defines: Vec<String>,
    /// Treat warnings as errors.
    pub werror: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            name: "My Game".into(),
            title: "MYGAME".into(),
            code: "GBDE".into(),
            maker: "01".into(),
            version: 0,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        BuildConfig { optimization: "O2".into(), cflags: Vec::new(), defines: Vec::new(), werror: false }
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig { game: GameConfig::default(), build: BuildConfig::default() }
    }
}

/// An error about the project itself (missing file, bad config...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectError(pub String);

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProjectError {}

/// A loaded project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub config: ProjectConfig,
}

/// Kind of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    C,
    Asm,
}

impl Project {
    /// Open the project in `dir` (which must contain `gbade.toml`).
    pub fn open(dir: &Path) -> Result<Project, ProjectError> {
        let file = dir.join(PROJECT_FILE);
        let text = std::fs::read_to_string(&file).map_err(|e| {
            ProjectError(format!("Cannot read {}: {e}. Is this a GBADE project folder?", file.display()))
        })?;
        let config: ProjectConfig =
            toml::from_str(&text).map_err(|e| ProjectError(format!("Error in {}:\n{e}", file.display())))?;
        let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        Ok(Project { root, config })
    }

    /// Find the project containing `path` (walking up parent directories).
    pub fn find(path: &Path) -> Option<PathBuf> {
        path.ancestors().find(|d| d.join(PROJECT_FILE).exists()).map(Path::to_path_buf)
    }

    /// Write `gbade.toml`.
    pub fn save_config(&self) -> std::io::Result<()> {
        let text = toml::to_string_pretty(&self.config).expect("config serializes");
        std::fs::write(self.root.join(PROJECT_FILE), text)
    }

    pub fn src_dir(&self) -> PathBuf {
        self.root.join("src")
    }
    pub fn include_dir(&self) -> PathBuf {
        self.root.join("include")
    }
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn build_dir(&self) -> PathBuf {
        self.root.join("build")
    }

    /// File-system friendly version of the game name ("My Game" -> "my-game").
    pub fn slug(&self) -> String {
        slugify(&self.config.game.name)
    }

    /// Path of the ROM produced by a build.
    pub fn rom_path(&self) -> PathBuf {
        self.build_dir().join(format!("{}.gba", self.slug()))
    }
    pub fn elf_path(&self) -> PathBuf {
        self.build_dir().join(format!("{}.elf", self.slug()))
    }
    /// Where GBADE keeps the game's save memory between runs.
    pub fn save_path(&self) -> PathBuf {
        self.build_dir().join(format!("{}.sav", self.slug()))
    }

    /// All source files under `src/`, sorted for reproducible builds.
    pub fn sources(&self) -> Vec<(PathBuf, SourceKind)> {
        let mut out = Vec::new();
        collect_files(&self.src_dir(), &mut |p| {
            let kind = match p.extension().and_then(|e| e.to_str()) {
                Some("c") => Some(SourceKind::C),
                Some("s") | Some("S") => Some(SourceKind::Asm),
                _ => None,
            };
            if let Some(k) = kind {
                out.push((p.to_path_buf(), k));
            }
        });
        out.sort();
        out
    }

    /// All PNG files under `assets/`.
    pub fn assets(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_files(&self.assets_dir(), &mut |p| {
            if p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("png")) == Some(true) {
                out.push(p.to_path_buf());
            }
        });
        out.sort();
        out
    }

    /// Create a new project from a template directory (copied recursively)
    /// and set its name. Fails if `dest` already contains a project.
    pub fn create_from_template(template: &Path, dest: &Path, name: &str) -> Result<Project, ProjectError> {
        if dest.join(PROJECT_FILE).exists() {
            return Err(ProjectError(format!("{} already contains a GBADE project", dest.display())));
        }
        copy_dir(template, dest).map_err(|e| ProjectError(format!("Could not copy template: {e}")))?;
        // Never copy a template's build output.
        let _ = std::fs::remove_dir_all(dest.join("build"));
        let mut project = if dest.join(PROJECT_FILE).exists() {
            Project::open(dest)?
        } else {
            Project { root: dest.canonicalize().unwrap_or(dest.to_path_buf()), config: ProjectConfig::default() }
        };
        project.config.game.name = name.to_string();
        project.config.game.title = header_title(name);
        project.save_config().map_err(|e| ProjectError(format!("Could not write gbade.toml: {e}")))?;
        Ok(project)
    }
}

/// "My Cool Game!" -> "my-cool-game"
pub fn slugify(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "game".into() } else { s }
}

/// "My Cool Game!" -> "MY COOL GAME" (12 chars max, header-safe ASCII).
pub fn header_title(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .map(|c| c.to_ascii_uppercase())
        .take(12)
        .collect::<String>()
        .trim()
        .to_string()
}

fn collect_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_files(&p, f);
        } else {
            f(&p);
        }
    }
}

/// Recursively copy a directory.
pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        if name == "build" || name.to_string_lossy().starts_with('.') {
            continue;
        }
        let to = dst.join(&name);
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_and_title() {
        assert_eq!(slugify("My Cool Game!"), "my-cool-game");
        assert_eq!(slugify("  "), "game");
        assert_eq!(slugify("Pong 2"), "pong-2");
        assert_eq!(header_title("My Cool Game!"), "MY COOL GAME");
        assert_eq!(header_title("a very long game name"), "A VERY LONG");
    }

    #[test]
    fn config_defaults_and_roundtrip() {
        let c: ProjectConfig = toml::from_str("[game]\nname = \"X\"\n").unwrap();
        assert_eq!(c.game.name, "X");
        assert_eq!(c.build.optimization, "O2");
        let text = toml::to_string_pretty(&c).unwrap();
        let back: ProjectConfig = toml::from_str(&text).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn bad_config_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(PROJECT_FILE), "[game\nname=").unwrap();
        let err = Project::open(dir.path()).unwrap_err();
        assert!(err.0.contains("Error in"), "{}", err.0);
    }

    #[test]
    fn sources_and_assets_are_discovered() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(PROJECT_FILE), "").unwrap();
        std::fs::create_dir_all(root.join("src/sub")).unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("src/main.c"), "").unwrap();
        std::fs::write(root.join("src/sub/fast.s"), "").unwrap();
        std::fs::write(root.join("src/notes.txt"), "").unwrap();
        std::fs::write(root.join("src/.hidden.c"), "").unwrap();
        std::fs::write(root.join("assets/hero.png"), "").unwrap();
        let p = Project::open(root).unwrap();
        let srcs: Vec<_> = p.sources().into_iter().map(|(p, k)| (p.file_name().unwrap().to_owned(), k)).collect();
        assert_eq!(srcs.len(), 2);
        assert!(srcs.contains(&("main.c".into(), SourceKind::C)));
        assert!(srcs.contains(&("fast.s".into(), SourceKind::Asm)));
        assert_eq!(p.assets().len(), 1);
        assert_eq!(p.rom_path().file_name().unwrap(), "my-game.gba");
    }

    #[test]
    fn create_from_template_sets_name() {
        let tpl = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tpl.path().join("src")).unwrap();
        std::fs::create_dir_all(tpl.path().join("build")).unwrap();
        std::fs::write(tpl.path().join("src/main.c"), "int main(){}").unwrap();
        std::fs::write(tpl.path().join("build/junk.o"), "").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let target = dest.path().join("new");
        let p = Project::create_from_template(tpl.path(), &target, "Space Blaster").unwrap();
        assert_eq!(p.config.game.name, "Space Blaster");
        assert_eq!(p.config.game.title, "SPACE BLASTE");
        assert!(target.join("src/main.c").exists());
        assert!(!target.join("build").exists());
        // Creating again must fail instead of overwriting.
        assert!(Project::create_from_template(tpl.path(), &target, "Again").is_err());
    }
}
