//! End-to-end tests: build the example projects with the real toolchain and
//! run them in the real emulator.

mod common;
use common::*;
use gbade_emu::Keys;

#[test]
fn hello_world_builds_and_prints() {
    let (_dir, p) = copy_project(&repo_root().join("examples/hello-world"));
    let out = build_ok(&p);
    assert!(out.warnings().next().is_none(), "unexpected warnings: {:#?}", out.diagnostics);
    let mem = out.memory.unwrap();
    assert!(mem.rom > 1000 && mem.rom < 200_000, "rom size {}", mem.rom);

    let mut run = Run::new(&out);
    run.frames(30);
    assert!(run.printed("Hello from the GBA!"), "console: {:?}", run.game_output());

    // Backdrop colour RGB8(20, 30, 60) in the top-right corner (no text there).
    let (r, g, b) = run.emu.pixel(239, 159);
    assert!(r < 40 && g < 40 && b > 40 && b < 80, "backdrop is {r},{g},{b}");
    // Some white text pixels on the first text row.
    let white = (0..240).flat_map(|x| (0..8).map(move |y| (x, y))).filter(|&(x, y)| run.emu.pixel(x, y) == (255, 255, 255)).count();
    assert!(white > 50, "expected white text pixels, got {white}");

    run.press(Keys::A, 3);
    assert!(run.printed("A was pressed 1 time(s)"), "console: {:?}", run.game_output());
    run.press(Keys::A, 3);
    assert!(run.printed("A was pressed 2 time(s)"));
}

#[test]
fn incremental_build_skips_unchanged_files() {
    let (_dir, p) = copy_project(&repo_root().join("examples/hello-world"));
    let first = build_ok(&p);
    assert!(first.compiled >= 1);
    let second = build_ok(&p);
    assert_eq!(second.compiled, 0, "nothing changed, nothing should be recompiled");
    // Touch main.c (new mtime) -> recompiled.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let main = p.src_dir().join("main.c");
    let text = std::fs::read_to_string(&main).unwrap();
    std::fs::write(&main, text + "\n// edit\n").unwrap();
    let third = build_ok(&p);
    assert_eq!(third.compiled, 1);
}
