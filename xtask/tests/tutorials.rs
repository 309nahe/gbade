//! Checks on the tutorials (`tutorials/`): every lesson has the same shape,
//! comes with a complete project, renders in the Tutorials panel, and only
//! points readers to real community resources.

use std::path::{Path, PathBuf};

fn tutorials_dir() -> PathBuf {
    gbade_toolchain::paths::canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../tutorials"))
}

/// The tutorial folders, in order.
fn tutorials() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(tutorials_dir())
        .expect("tutorials/ exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Chapters of Tonc (community edition, https://gbadev.net/tonc/), from its
/// table of contents (github.com/gbadev-org/tonc, content/SUMMARY.md).
const TONC_CHAPTERS: &[&str] = &[
    "foreword", "intro", "hardware", "setup", "first", "video", "bitmaps", "keys", "objbg", "regobj",
    "regbg", "affine", "affobj", "affbg", "gfx", "dma", "timers", "interrupts", "swi", "sndsqr",
    "text", "mode7", "mode7ex", "tte", "asm", "lab", "numbers", "fixed", "matrix", "makefile",
    "edmake", "refs", "log",
];

/// Chapters of Game Programming Patterns by Robert Nystrom
/// (github.com/munificent/game-programming-patterns, book/*.markdown).
const GPP_CHAPTERS: &[&str] = &[
    "game-loop", "state", "spatial-partition", "update-method", "double-buffer", "component",
    "observer", "command", "event-queue", "object-pool",
];

/// Every external link in `text`.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else { break };
        out.push(rest[..end].trim().to_string());
        rest = &rest[end + 1..];
    }
    out
}

#[test]
fn there_are_about_ten_numbered_tutorials() {
    let all = tutorials();
    assert!(all.len() >= 10, "{all:?}");
    for (i, dir) in all.iter().enumerate() {
        let name = dir.file_name().unwrap().to_string_lossy();
        let expected = format!("{:02}-", i + 1);
        assert!(name.starts_with(&expected), "{name}: folders are numbered 01-, 02-... without gaps");
        assert!(
            name[3..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{name}: lowercase words separated by dashes"
        );
    }
}

#[test]
fn every_tutorial_has_a_lesson_and_a_complete_project() {
    for dir in tutorials() {
        for f in ["README.md", "gbade.toml", "src/main.c"] {
            assert!(dir.join(f).is_file(), "{} has no {f}", dir.display());
        }
        assert!(!dir.join("build").exists(), "{}: build output must not be committed", dir.display());
        let toml = read(&dir.join("gbade.toml"));
        assert!(toml.contains("name = \"") && toml.contains("werror = false"), "{}", dir.display());
    }
}

#[test]
fn lessons_follow_the_same_outline() {
    for (i, dir) in tutorials().iter().enumerate() {
        let text = read(&dir.join("README.md"));
        let first = text.lines().next().unwrap_or("");
        assert!(first.starts_with(&format!("# {}. ", i + 1)), "{}: first line is \"# {}. Title\"", dir.display(), i + 1);
        assert_eq!(text.lines().filter(|l| l.starts_with("# ")).count(), 1, "{}: one title", dir.display());
        for section in ["## What you will learn", "## Run the example", "## Play with it", "## Go further"] {
            assert!(text.contains(&format!("\n{section}\n")), "{} has no \"{section}\" section", dir.display());
        }
        assert!(text.contains("**▶ Try this example**"), "{}: tell the reader which button to press", dir.display());
        // Small: a lesson fits in a few screens.
        let words = text.split_whitespace().count();
        assert!(words < 900, "{}: {words} words, keep tutorials short", dir.display());
        // The example says which tutorial it belongs to.
        let main = read(&dir.join("src/main.c"));
        assert!(main.starts_with(&format!("// Tutorial {}: ", i + 1)), "{}: main.c header", dir.display());
    }
}

#[test]
fn lessons_render_in_the_panel() {
    for dir in tutorials() {
        let text = read(&dir.join("README.md"));
        let fences = text.lines().filter(|l| l.trim_start().starts_with("```")).count();
        assert_eq!(fences % 2, 0, "{}: unbalanced code fence", dir.display());
        assert!(!text.contains("!["), "{}: images are not shown by the panel", dir.display());
        let lower = text.to_ascii_lowercase();
        for tag in ["<div", "<table", "<img", "<br", "<span", "<!--", "<p>"] {
            assert!(!lower.contains(tag), "{}: HTML ({tag}) is not rendered", dir.display());
        }
    }
}

#[test]
fn links_only_point_to_known_community_resources() {
    for dir in tutorials() {
        let text = read(&dir.join("README.md"));
        let go_further = text.split("## Go further").nth(1).unwrap_or("");
        assert!(go_further.contains("https://"), "{}: \"Go further\" links to a community resource", dir.display());
        for link in links(&text) {
            if let Some(page) = link.strip_prefix("https://gbadev.net/tonc/") {
                let chapter = page.strip_suffix(".html").unwrap_or_else(|| panic!("{link}: Tonc pages end in .html"));
                assert!(TONC_CHAPTERS.contains(&chapter), "{}: {link} is not a Tonc chapter", dir.display());
            } else if let Some(page) = link.strip_prefix("https://gameprogrammingpatterns.com/") {
                let chapter = page.strip_suffix(".html").unwrap_or("");
                assert!(GPP_CHAPTERS.contains(&chapter), "{}: {link} is not a Game Programming Patterns chapter", dir.display());
            } else {
                assert!(
                    link == "https://stuij.github.io/deku-sound-tutorial/",
                    "{}: {link} is not one of the community resources these checks know; add it to this test after checking it",
                    dir.display()
                );
            }
        }
    }
}
