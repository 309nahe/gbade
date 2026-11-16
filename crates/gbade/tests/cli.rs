//! The command line, driven as a user (or a script) would: the real `gbade`
//! program in a separate process, checking exit codes, what goes to
//! standard output (only the game's own output) and standard error, and the
//! files it writes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The `gbade` program, with a runtime cache inside `target/` so tests never
/// touch the user's cache.
fn gbade(args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_gbade"));
    c.args(args);
    let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gbade-test-cache");
    c.env("GBADE_CACHE_DIR", cache);
    c
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(cmd: &mut Command) -> Run {
    let Output { status, stdout, stderr } = cmd.output().expect("gbade runs");
    Run { code: status.code(), stdout: String::from_utf8_lossy(&stdout).into(), stderr: String::from_utf8_lossy(&stderr).into() }
}

fn ok(args: &[&str]) -> Run {
    let r = run(&mut gbade(args));
    assert_eq!(r.code, Some(0), "gbade {args:?}\nstdout:\n{}\nstderr:\n{}", r.stdout, r.stderr);
    r
}

/// A new project from a template, in a fresh temporary folder.
fn new_project(template: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("game");
    ok(&["new", dir.to_str().unwrap(), "--template", template]);
    (tmp, dir)
}

/// A project whose main.c is `src`.
fn project_with_main(src: &str) -> (tempfile::TempDir, PathBuf) {
    let (tmp, dir) = new_project("hello-world");
    std::fs::write(dir.join("src/main.c"), src).unwrap();
    (tmp, dir)
}

#[test]
fn version_help_and_unknown_commands() {
    let r = ok(&["--version"]);
    assert_eq!(r.stdout.trim(), format!("gbade {}", env!("CARGO_PKG_VERSION")));
    let r = ok(&["help"]);
    assert!(r.stdout.contains("gbade build [DIR]") && r.stdout.contains("gbade info"), "{}", r.stdout);
    let r = run(&mut gbade(&["frobnicate"]));
    assert_eq!(r.code, Some(2));
    assert!(r.stderr.contains("unknown command `frobnicate`"));
    assert!(r.stdout.is_empty(), "errors go to stderr");
}

#[test]
fn examples_are_listed_simplest_first() {
    let r = ok(&["examples"]);
    let names: Vec<&str> = r.stdout.lines().map(|l| l.split_whitespace().next().unwrap()).collect();
    for name in ["hello-world", "drawing", "sprites", "scrolling", "pong"] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
    assert_eq!(names[0], "hello-world");
    assert!(r.stdout.lines().all(|l| l.split_whitespace().count() > 1), "every example has a description");
}

#[test]
fn new_copies_a_template_and_names_the_game() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("space-game");
    let r = ok(&["new", dir.to_str().unwrap(), "--template", "sprites", "--name", "Space: The Game!"]);
    assert!(r.stdout.contains("created Space: The Game!"), "{}", r.stdout);
    let toml = std::fs::read_to_string(dir.join("gbade.toml")).unwrap();
    assert!(toml.contains("name = \"Space: The Game!\""), "{toml}");
    assert!(toml.contains("title = \"SPACE THE GA\""), "12 ROM-safe characters: {toml}");
    assert!(dir.join("src/main.c").exists() && dir.join("assets").is_dir());
    assert!(!dir.join("build").exists(), "no build output copied from the template");

    // Without --name, the folder name is used.
    let other = tmp.path().join("my-pong");
    ok(&["new", other.to_str().unwrap(), "--template", "pong"]);
    assert!(std::fs::read_to_string(other.join("gbade.toml")).unwrap().contains("name = \"my-pong\""));

    // Existing projects are never overwritten.
    let r = run(&mut gbade(&["new", dir.to_str().unwrap()]));
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("already contains a GBADE project"), "{}", r.stderr);
}

#[test]
fn new_explains_bad_arguments() {
    let r = run(&mut gbade(&["new"]));
    assert_eq!(r.code, Some(2));
    assert!(r.stderr.contains("usage: gbade new DIR"));
    let tmp = tempfile::tempdir().unwrap();
    let r = run(&mut gbade(&["new", tmp.path().join("x").to_str().unwrap(), "--template", "tetris"]));
    assert_eq!(r.code, Some(2));
    assert!(r.stderr.contains("unknown template `tetris`") && r.stderr.contains("hello-world"), "{}", r.stderr);
    assert!(!tmp.path().join("x").exists());
}

#[test]
fn build_finds_the_project_from_any_folder_inside_it() {
    let (_tmp, dir) = new_project("hello-world");
    let r = ok(&["build", dir.join("src").to_str().unwrap()]);
    assert!(r.stderr.contains("built ") && r.stderr.contains("game.gba"), "{}", r.stderr);
    assert!(r.stdout.is_empty(), "build messages go to stderr: {}", r.stdout);
    assert!(dir.join("build/game.gba").exists());
    // From the current directory, without an argument.
    ok_in(&dir.join("src"), &["build"]);
}

fn ok_in(cwd: &Path, args: &[&str]) -> Run {
    let r = run(gbade(args).current_dir(cwd));
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    r
}

#[test]
fn build_errors_exit_with_1_and_show_location_and_hint() {
    let (_tmp, dir) = project_with_main("#include <gba.h>\nint main(void) {\n    int lives = 3\n    while (1) vsync();\n}\n");
    let r = run(&mut gbade(&["build", dir.to_str().unwrap()]));
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("main.c:3:18: error: expected ';'"), "{}", r.stderr);
    assert!(r.stderr.contains("hint:"), "{}", r.stderr);
    assert!(r.stderr.contains("build failed"));

    let tmp = tempfile::tempdir().unwrap();
    let r = run(&mut gbade(&["build", tmp.path().to_str().unwrap()]));
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("no gbade.toml found"), "{}", r.stderr);
}

#[test]
fn run_prints_only_the_game_output_on_stdout() {
    let (tmp, dir) = new_project("hello-world");
    let shot = tmp.path().join("shot.png");
    let r = ok(&["run", dir.to_str().unwrap(), "--frames", "20", "--screenshot", shot.to_str().unwrap()]);
    assert_eq!(r.stdout.trim(), "Hello from the GBA! The game has started.");
    assert!(r.stderr.contains("screenshot saved"));
    let img = image::open(&shot).unwrap();
    assert_eq!((img.width(), img.height()), (240, 160));
}

#[test]
fn run_presses_buttons_at_the_given_frames() {
    let (_tmp, dir) = new_project("hello-world");
    let r = ok(&["run", dir.to_str().unwrap(), "--frames", "60", "--press", "a@10", "--press", "A@30"]);
    assert!(r.stdout.contains("A was pressed 1 time(s)"), "{}", r.stdout);
    assert!(r.stdout.contains("A was pressed 2 time(s)"), "{}", r.stdout);
    // Without presses, nothing is pressed.
    let r = ok(&["run", dir.to_str().unwrap(), "--frames", "60"]);
    assert!(!r.stdout.contains("pressed"), "{}", r.stdout);
}

#[test]
fn run_rejects_bad_options_with_2() {
    let (_tmp, dir) = new_project("hello-world");
    for args in [vec!["--press", "jump@3"], vec!["--press", "a@soon"], vec!["--speed", "2"]] {
        let mut all = vec!["run", dir.to_str().unwrap()];
        all.extend(&args);
        let r = run(&mut gbade(&all));
        assert_eq!(r.code, Some(2), "{args:?}: {}", r.stderr);
    }
}

#[test]
fn a_game_that_panics_exits_with_3() {
    let (_tmp, dir) = project_with_main(
        "#include <gba.h>\n#include <assert.h>\nint main(void) {\n    int lives = 0;\n    assert(lives > 0);\n    while (1) vsync();\n}\n",
    );
    let r = run(&mut gbade(&["run", dir.to_str().unwrap(), "--frames", "30"]));
    assert_eq!(r.code, Some(3), "stdout:\n{}\nstderr:\n{}", r.stdout, r.stderr);
    assert!(format!("{}{}", r.stdout, r.stderr).contains("lives > 0"));
}

#[test]
fn the_cartridge_save_persists_between_runs() {
    // Count the runs in save memory: each run reads, adds one, writes back.
    let (_tmp, dir) = project_with_main(
        "#include <gba.h>\n#include <stdio.h>\n\
         int main(void) {\n\
             u8 runs = 0;\n\
             save_read(0, &runs, 1);\n\
             if (runs == 0xFF) runs = 0;\n\
             runs++;\n\
             save_write(0, &runs, 1);\n\
             printf(\"run %d\\n\", runs);\n\
             while (1) vsync();\n\
         }\n",
    );
    let first = ok(&["run", dir.to_str().unwrap(), "--frames", "10"]);
    assert_eq!(first.stdout.trim(), "run 1");
    assert!(dir.join("build/game.sav").exists());
    let second = ok(&["run", dir.to_str().unwrap(), "--frames", "10"]);
    assert_eq!(second.stdout.trim(), "run 2");
}

#[test]
fn info_shows_the_compiler_in_use() {
    let r = ok(&["info"]);
    for key in ["program:", "resources:", "compiler:", "linker:", "sdk:", "examples:", "docs:", "cache:"] {
        assert!(r.stdout.contains(key), "{key} missing:\n{}", r.stdout);
    }
    assert!(r.stdout.contains("clang version"), "{}", r.stdout);
    assert!(r.stdout.contains("target/gbade-test-cache") || r.stdout.contains("target\\gbade-test-cache"), "GBADE_CACHE_DIR is honoured");
}
