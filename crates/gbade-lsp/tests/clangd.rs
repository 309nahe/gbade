//! Talk to the real bundled clangd about a real GBADE project.

use gbade_lsp::{Event, LspClient, Pos};
use gbade_toolchain::{Project, Severity, Toolchain, builder::write_compile_commands};
use std::time::Duration;

const MAIN: &str = r#"#include <gba.h>
int main(void) {
    text_init();
    while (1) {
        vsync();
        key_
    }
}
"#;

fn setup() -> Option<(tempfile::TempDir, Project, LspClient, std::path::PathBuf)> {
    let tc = Toolchain::locate().expect("toolchain");
    let Some(clangd) = tc.clangd.clone() else {
        eprintln!("clangd not found: run `cargo xtask fetch-llvm`");
        return None;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("gbade.toml"), "[game]\nname = \"Lsp\"\n").unwrap();
    let main = dir.path().join("src/main.c");
    std::fs::write(&main, MAIN).unwrap();
    let p = Project::open(dir.path()).unwrap();
    write_compile_commands(&p, &tc).unwrap();
    let main = main.canonicalize().unwrap();
    let client = LspClient::start(&clangd, &p.root, &p.build_dir()).expect("clangd starts");
    Some((dir, p, client, main))
}

#[test]
fn live_diagnostics_completion_hover_definition() {
    let Some((_dir, _p, mut lsp, main)) = setup() else { return };
    lsp.did_open(&main, MAIN);

    // 1. Live diagnostics: line 6 ("key_") is an error, everything else is fine:
    //    in particular <gba.h> and the GBA target flags are understood.
    let events = lsp.wait_for(Duration::from_secs(30), |e| matches!(e, Event::Diagnostics(..)));
    let diags = events
        .iter()
        .find_map(|e| match e { Event::Diagnostics(p, d) if *p == main => Some(d.clone()), _ => None })
        .expect("diagnostics for main.c");
    let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(!errors.is_empty(), "{diags:#?}");
    assert!(errors.iter().all(|d| d.line == 6 || d.line == 7), "only the unfinished line is wrong: {errors:#?}");
    assert!(!diags.iter().any(|d| d.message.contains("gba.h")), "SDK headers must resolve: {diags:#?}");

    // 2. Completion after "key_" proposes the SDK's input functions.
    let id = lsp.completion(&main, Pos { line: 5, character: 12 });
    let events = lsp.wait_for(Duration::from_secs(30), |e| matches!(e, Event::Completion { id: i, .. } if *i == id));
    let items = events.iter().find_map(|e| match e { Event::Completion { items, .. } => Some(items.clone()), _ => None }).unwrap();
    let names: Vec<&str> = items.iter().map(|i| i.insert.as_str()).collect();
    assert!(names.contains(&"key_hit") && names.contains(&"key_poll"), "{names:?}");

    // 3. Fix the code: diagnostics update to "no errors".
    let fixed = MAIN.replace("        key_\n", "        key_poll();\n");
    lsp.did_change(&main, &fixed);
    let events = lsp.wait_for(Duration::from_secs(30), |e| {
        matches!(e, Event::Diagnostics(p, d) if *p == main && !d.iter().any(|d| d.severity == Severity::Error))
    });
    assert!(
        events.iter().any(|e| matches!(e, Event::Diagnostics(_, d) if d.iter().all(|d| d.severity != Severity::Error))),
        "{events:#?}"
    );

    // 4. Hover on vsync shows its documentation from gba/video.h.
    let id = lsp.hover(&main, Pos { line: 4, character: 10 });
    let events = lsp.wait_for(Duration::from_secs(30), |e| matches!(e, Event::Hover { id: i, .. } if *i == id));
    let text = events.iter().find_map(|e| match e { Event::Hover { text, .. } => Some(text.clone()), _ => None }).unwrap();
    assert!(text.contains("vsync"), "{text}");
    assert!(text.to_lowercase().contains("vblank"), "the header comment is shown: {text}");

    // 5. Go to definition of vsync lands in the SDK header.
    let id = lsp.definition(&main, Pos { line: 4, character: 10 });
    let events = lsp.wait_for(Duration::from_secs(30), |e| matches!(e, Event::Definition { id: i, .. } if *i == id));
    let target = events.iter().find_map(|e| match e { Event::Definition { target, .. } => target.clone(), _ => None });
    let (file, pos) = target.expect("a definition");
    assert!(file.ends_with("gba/video.h"), "{file:?}");
    let line = std::fs::read_to_string(&file).unwrap().lines().nth(pos.line as usize).unwrap().to_string();
    assert!(line.contains("vsync"), "{line}");

    lsp.shutdown();
}

#[test]
fn asset_header_resolves_before_the_first_build() {
    let Some((dir, _p, mut lsp, _)) = setup() else { return };
    let other = dir.path().join("src/other.c");
    let text = "#include \"assets.h\"\n#include <gba.h>\nvoid f(void) { vsync(); }\n";
    std::fs::write(&other, text).unwrap();
    let other = other.canonicalize().unwrap();
    lsp.did_open(&other, text);
    let events = lsp.wait_for(Duration::from_secs(30), |e| matches!(e, Event::Diagnostics(p, _) if *p == other));
    let diags = events.iter().find_map(|e| match e { Event::Diagnostics(p, d) if *p == other => Some(d.clone()), _ => None }).unwrap();
    assert!(diags.iter().all(|d| d.severity != Severity::Error), "{diags:#?}");
}
