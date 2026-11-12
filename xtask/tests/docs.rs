//! Checks that the shipped documentation stays usable by the IDE's Learn
//! panel.
//!
//! The panel is a deliberately small Markdown renderer: guides are listed by
//! file name, titled by their first `# ` line, and only a subset of Markdown
//! is understood. These tests catch the mistakes that would break the panel or
//! leave dangling links, without needing a compiler or an emulator.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap()
}

fn docs_dir() -> PathBuf {
    repo_root().join("docs")
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(markdown_files(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// First line that is not blank and not an HTML comment.
fn first_content_line(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("<!--"))
}

#[test]
fn guide_files_are_numbered_and_named_in_reading_order() {
    let dir = docs_dir().join("guide");
    assert!(
        dir.is_dir(),
        "docs/guide is missing: the Learn panel would be empty"
    );

    let guides: Vec<PathBuf> = markdown_files(&dir);
    assert!(
        guides.len() >= 5,
        "expected a real guide set, found {}",
        guides.len()
    );

    for (i, path) in guides.iter().enumerate() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let expected = format!("{:02}-", i + 1);
        assert!(
            name.starts_with(&expected) && name.ends_with(".md"),
            "guide {name} should be named {expected}....md so the panel sorts it correctly"
        );
        let stem = name.trim_end_matches(".md");
        assert!(
            stem.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "guide {name} should use lowercase words separated by dashes"
        );
    }
}

#[test]
fn every_document_has_a_title() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        let first = first_content_line(&text);
        assert!(
            first.is_some_and(|l| l.starts_with("# ") && l.len() > 2),
            "{} must start with a single '# Title' line; the Learn panel uses it as the file name",
            path.display()
        );
    }
}

#[test]
fn no_images_or_html_in_documents() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        assert!(
            !text.contains("!["),
            "{} uses Markdown image syntax, which the Learn panel cannot show",
            path.display()
        );
        // The renderer prints raw HTML tags literally; only check for real
        // element names so that inline code such as `<gba.h>` stays allowed.
        let lower = text.to_ascii_lowercase();
        for tag in [
            "<div", "<table", "<img", "<br", "<span", "<script", "<style", "<p>",
        ] {
            assert!(
                !lower.contains(tag),
                "{} contains the HTML tag {tag}, which the Learn panel does not render",
                path.display()
            );
        }
    }
}

#[test]
fn relative_links_resolve() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        let base = path.parent().unwrap();
        let mut rest = text.as_str();
        while let Some(start) = rest.find("](") {
            rest = &rest[start + 2..];
            let Some(end) = rest.find(')') else { break };
            let target = rest[..end].trim();
            rest = &rest[end + 1..];
            if target.is_empty()
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with('#')
                || target.starts_with("mailto:")
            {
                continue;
            }
            let target = target.split('#').next().unwrap();
            assert!(
                base.join(target).exists(),
                "{} links to {target}, which does not exist",
                path.display()
            );
        }
    }
}

#[test]
fn credits_cover_the_required_projects() {
    let credits = docs_dir().join("CREDITS.md");
    assert!(
        credits.is_file(),
        "docs/CREDITS.md is linked from the About window"
    );
    let text = read(&credits).to_ascii_lowercase();
    for name in [
        "mgba",
        "llvm",
        "printf",
        "font8x8",
        "agb-gbafix",
        "egui",
        "cpal",
        "rfd",
        "image",
        "object",
        "gimli",
        "serde",
        "toml",
        "tonc",
        "gbatek",
        "ubuntu font",
        "noto emoji",
        "hack",
        "emoji-icon-font",
    ] {
        assert!(text.contains(name), "CREDITS.md does not mention {name}");
    }
}

#[test]
fn licenses_folder_is_complete_enough_to_bundle() {
    let dir = repo_root().join("licenses");
    assert!(dir.is_dir(), "licenses/ is required by cargo xtask bundle");
    for file in [
        "MPL-2.0.txt",
        "Apache-2.0.txt",
        "MIT.txt",
        "LLVM.txt",
        "Ubuntu-Font-1.0.txt",
        "SIL-OFL-1.1.txt",
        "Hack-font.txt",
        "emoji-icon-font-MIT.txt",
    ] {
        let path = dir.join(file);
        assert!(path.is_file(), "licenses/{file} is missing");
        let text = read(&path);
        assert!(text.len() > 500, "licenses/{file} looks like a placeholder");
    }
}

#[test]
fn reference_pages_are_present() {
    for file in ["sdk-api.md", "cli.md", "assets.md", "troubleshooting.md"] {
        assert!(docs_dir().join(file).is_file(), "docs/{file} is missing");
    }
}

#[test]
fn code_fences_are_balanced() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        let fences = text
            .lines()
            .filter(|l| l.trim_start().starts_with("```"))
            .count();
        assert_eq!(
            fences % 2,
            0,
            "{} has an unclosed code fence",
            path.display()
        );
    }
}

#[test]
fn tables_have_a_separator_row() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        let mut in_code = false;
        let mut has_table = false;
        let mut has_separator = false;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                continue;
            }
            if t.starts_with('|') {
                has_table = true;
                let cells = t.trim_matches('|');
                if cells.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && t.contains('-') {
                    has_separator = true;
                }
            }
        }
        if has_table {
            assert!(
                has_separator,
                "{} has a table without a |---| separator row",
                path.display()
            );
        }
    }
}

#[test]
fn only_the_first_heading_is_level_one() {
    for path in markdown_files(&docs_dir()) {
        let text = read(&path);
        let mut in_code = false;
        let h1: Vec<&str> = text
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                if t.starts_with("```") {
                    in_code = !in_code;
                    return false;
                }
                !in_code && t.starts_with("# ")
            })
            .collect();
        assert_eq!(
            h1.len(),
            1,
            "{} should have exactly one '# ' heading, the document title",
            path.display()
        );
        assert!(
            first_content_line(&text).is_some_and(|l| l.starts_with("# ")),
            "{} must begin with its title",
            path.display()
        );
    }
}
