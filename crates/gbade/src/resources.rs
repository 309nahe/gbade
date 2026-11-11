//! Finding bundled resources: example projects and documentation.
//!
//! In the app bundle they live in `GBADE.app/Contents/Resources/{examples,docs}`;
//! in a development checkout, at the repository root.

use std::path::{Path, PathBuf};

fn candidates(name: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = std::env::var_os("GBADE_RESOURCES_DIR") {
        v.push(PathBuf::from(dir).join(name));
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe = exe.canonicalize().unwrap_or(exe);
        if let Some(dir) = exe.parent() {
            v.push(dir.join("..").join("Resources").join(name));
        }
        for dir in exe.ancestors() {
            if dir.join("sdk").join("include").join("gba.h").exists() {
                v.push(dir.join(name));
                break;
            }
        }
    }
    for dir in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        if dir.join("sdk").join("include").join("gba.h").exists() {
            v.push(dir.join(name));
            break;
        }
    }
    v
}

fn find(name: &str) -> Option<PathBuf> {
    candidates(name).into_iter().find(|p| p.is_dir()).map(|p| p.canonicalize().unwrap_or(p))
}

pub fn examples_dir() -> Option<PathBuf> {
    find("examples")
}

pub fn docs_dir() -> Option<PathBuf> {
    find("docs")
}

/// An example project that can be used as a template.
#[derive(Debug, Clone)]
pub struct Example {
    pub dir: PathBuf,
    pub name: String,
    /// First paragraph of the example's README.md, or of main.c's header.
    pub description: String,
    /// Sort key from the README "order:" line (lower first).
    pub order: u32,
}

/// List the bundled examples, simplest first.
pub fn examples() -> Vec<Example> {
    let Some(dir) = examples_dir() else { return Vec::new() };
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if !p.join("gbade.toml").exists() {
            continue;
        }
        let name = gbade_toolchain::Project::open(&p)
            .map(|pr| pr.config.game.name)
            .unwrap_or_else(|_| e.file_name().to_string_lossy().into_owned());
        let readme = std::fs::read_to_string(p.join("README.md")).unwrap_or_default();
        let (description, order) = parse_readme(&readme);
        out.push(Example { dir: p, name, description, order });
    }
    out.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(&b.name)));
    out
}

/// README format: a "# Title" line, an optional "order: N" line, then the
/// description paragraph.
fn parse_readme(text: &str) -> (String, u32) {
    let mut order = 100;
    let mut desc = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            if !desc.is_empty() {
                break;
            }
            continue;
        }
        if let Some(n) = t.strip_prefix("<!-- order:").and_then(|r| r.strip_suffix("-->")) {
            order = n.trim().parse().unwrap_or(100);
            continue;
        }
        if t.is_empty() {
            if !desc.is_empty() {
                break;
            }
            continue;
        }
        desc.push(t.to_string());
    }
    (desc.join(" "), order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readme_parsing() {
        let (d, o) = parse_readme("# Pong\n<!-- order: 3 -->\n\nA classic.\nTwo paddles.\n\nMore text.\n");
        assert_eq!(d, "A classic. Two paddles.");
        assert_eq!(o, 3);
    }

    #[test]
    fn examples_are_found_in_checkout() {
        let ex = examples();
        assert!(ex.iter().any(|e| e.name == "Hello World"), "{ex:?}");
        assert!(docs_dir().is_some());
    }
}
