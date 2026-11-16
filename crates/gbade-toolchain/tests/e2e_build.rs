//! The rules of the build pipeline, with the real compiler: what gets
//! recompiled when, how project settings reach clang, and the errors a
//! project can run into before the compiler even starts.

mod common;
use common::*;
use gbade_toolchain::{BuildOptions, Project, Toolchain, build, rom};
use std::path::Path;
use std::time::Duration;

/// Rewrite a file with a new modification time. File systems store times
/// with some granularity (and the build compares them), so wait a little.
fn edit(path: &Path, text: &str) {
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(path, text).unwrap();
}

fn write(p: &Project, rel: &str, text: &str) {
    let path = p.root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn try_build(p: &Project) -> gbade_toolchain::BuildOutput {
    build(p, &Toolchain::locate().unwrap(), &test_options())
}

const MAIN_WITH_HEADER: &str = "#include <gba.h>
#include <stdio.h>
#include \"shapes.h\"
int other(void);
int main(void) {
    printf(\"area=%d other=%d\\n\", area(), other());
    while (1) vsync();
}
";

#[test]
fn editing_a_header_recompiles_only_the_files_that_include_it() {
    let (_dir, p) = project_with_main(MAIN_WITH_HEADER);
    write(&p, "include/shapes.h", "static inline int area(void) { return 6 * 7; }\n");
    write(&p, "src/other.c", "int other(void) { return 5; }\n");
    let first = build_ok(&p);
    assert_eq!(first.compiled, 3, "main.c, other.c and the generated assets.c");
    assert_eq!(build_ok(&p).compiled, 0);

    edit(&p.root.join("include/shapes.h"), "static inline int area(void) { return 6 * 8; }\n");
    let out = build_ok(&p);
    assert_eq!(out.compiled, 1, "only main.c includes shapes.h:\n{}", out.log);
    let mut run = Run::new(&out);
    run.frames(5);
    assert!(run.printed("area=48 other=5"), "{:?}", run.game_output());
}

#[test]
fn changing_build_settings_recompiles_everything() {
    let (_dir, mut p) = project_with_main(MAIN_WITH_HEADER);
    write(&p, "include/shapes.h", "static inline int area(void) { return 1; }\n");
    write(&p, "src/other.c", "int other(void) { return 2; }\n");
    build_ok(&p);
    assert_eq!(build_ok(&p).compiled, 0);

    p.config.build.optimization = "Os".into();
    assert_eq!(build_ok(&p).compiled, 3, "new flags: every file again");
    assert_eq!(build_ok(&p).compiled, 0);

    // `force` (Rebuild All) recompiles even when nothing changed.
    let tc = Toolchain::locate().unwrap();
    let forced = build(&p, &tc, &BuildOptions { force: true, ..test_options() });
    assert!(forced.success, "{}", forced.log);
    assert_eq!(forced.compiled, 3);
}

#[test]
fn defines_and_extra_flags_reach_the_compiler() {
    let (_dir, mut p) = project_with_main(
        "#include <gba.h>\n#include <stdio.h>\n\
         int main(void) { printf(\"level=%d mode=%s\\n\", LEVEL, MODE); while (1) vsync(); }\n",
    );
    p.config.build.defines = vec!["LEVEL=7".into()];
    p.config.build.cflags = vec!["-DMODE=\"hard\"".into()];
    p.save_config().unwrap();
    // Read back from gbade.toml, as the IDE and the CLI do.
    let p = Project::open(&p.root).unwrap();
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(5);
    assert!(run.printed("level=7 mode=hard"), "{:?}", run.game_output());
}

#[test]
fn werror_turns_warnings_into_a_failed_build() {
    let src = "#include <gba.h>\nint main(void) { int unused = 3; while (1) vsync(); }\n";
    let (_dir, mut p) = project_with_main(src);
    let out = build_ok(&p);
    assert!(out.warnings().any(|w| w.message.contains("unused")), "{:#?}", out.diagnostics);

    p.config.build.werror = true;
    let out = try_build(&p);
    assert!(!out.success);
    assert!(out.errors().any(|e| e.message.contains("unused")), "{:#?}", out.diagnostics);
}

#[test]
fn an_unknown_optimization_level_falls_back_to_o2() {
    let (_dir, mut p) = project_with_main("#include <gba.h>\nint main(void) { while (1) vsync(); }\n");
    p.config.build.optimization = "O9".into();
    let out = build_ok(&p);
    assert!(out.log.contains("unknown optimization 'O9', using O2"), "{}", out.log);
}

#[test]
fn a_project_without_sources_is_explained() {
    let (_dir, p) = project_with_main("");
    std::fs::remove_file(p.src_dir().join("main.c")).unwrap();
    let out = try_build(&p);
    assert!(!out.success);
    assert!(out.errors().any(|e| e.message.contains("no source files")), "{:#?}", out.diagnostics);
}

#[test]
fn assembly_files_are_assembled_and_linked() {
    let (_dir, p) = project_with_main(
        "#include <gba.h>\n#include <stdio.h>\nint asm_triple(int x);\n\
         int main(void) { printf(\"triple=%d\\n\", asm_triple(14)); while (1) vsync(); }\n",
    );
    write(
        &p,
        "src/triple.s",
        "    .syntax unified\n    .thumb\n    .global asm_triple\n    .type asm_triple, %function\n    .thumb_func\n\
         asm_triple:\n    adds r1, r0, r0\n    adds r0, r0, r1\n    bx lr\n",
    );
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(5);
    assert!(run.printed("triple=42"), "{:?}", run.game_output());
}

#[test]
fn files_with_the_same_name_in_different_folders_do_not_collide() {
    let (_dir, p) = project_with_main(
        "#include <gba.h>\n#include <stdio.h>\nint enemy_speed(void);\nint player_speed(void);\n\
         int main(void) { printf(\"%d %d\\n\", enemy_speed(), player_speed()); while (1) vsync(); }\n",
    );
    write(&p, "src/enemy/util.c", "int enemy_speed(void) { return 3; }\n");
    write(&p, "src/player/util.c", "int player_speed(void) { return 4; }\n");
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(5);
    assert!(run.printed("3 4"), "{:?}", run.game_output());
    let objs: Vec<_> = std::fs::read_dir(p.build_dir().join("obj")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(objs.iter().any(|n| n.to_string_lossy().contains("enemy__util")), "{objs:?}");
    assert!(objs.iter().any(|n| n.to_string_lossy().contains("player__util")), "{objs:?}");
}

#[test]
fn a_moved_function_leaves_no_stale_object_behind() {
    // Move `speed` from a.c to b.c: if a.c's old object were still linked,
    // the linker would report a duplicate symbol.
    let (_dir, p) = project_with_main(
        "#include <gba.h>\n#include <stdio.h>\nint speed(void);\n\
         int main(void) { printf(\"speed=%d\\n\", speed()); while (1) vsync(); }\n",
    );
    write(&p, "src/a.c", "int speed(void) { return 1; }\n");
    build_ok(&p);
    std::fs::remove_file(p.src_dir().join("a.c")).unwrap();
    write(&p, "src/b.c", "int speed(void) { return 2; }\n");
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(5);
    assert!(run.printed("speed=2"), "{:?}", run.game_output());
}

#[test]
fn a_fixed_error_builds_again() {
    let (_dir, p) = project_with_main("#include <gba.h>\nint main(void) { while (1) vsync() }\n");
    write(&p, "src/ok.c", "int fine(void) { return 1; }\n");
    let out = try_build(&p);
    assert!(!out.success);
    // The failed file has no object, so it is compiled again once fixed,
    // even though the error was the only change.
    assert!(!p.build_dir().join("obj/src__main.c.o").exists());
    edit(&p.src_dir().join("main.c"), "#include <gba.h>\nint main(void) { while (1) vsync(); }\n");
    let out = build_ok(&p);
    assert_eq!(out.compiled, 1, "only main.c: ok.c was compiled the first time\n{}", out.log);
}

#[test]
fn a_broken_image_stops_the_build_with_its_file_name() {
    let (_dir, p) = project_with_main("#include <gba.h>\nint main(void) { while (1) vsync(); }\n");
    write(&p, "assets/hero.png", "this is not a PNG");
    let out = try_build(&p);
    assert!(!out.success);
    let err = out.errors().next().expect("an error");
    assert!(err.message.starts_with("asset"), "{err:?}");
    assert!(err.file.as_ref().unwrap().ends_with("hero.png"), "{err:?}");
}

#[test]
fn the_rom_header_comes_from_gbade_toml() {
    let (_dir, mut p) = project_with_main("#include <gba.h>\nint main(void) { while (1) vsync(); }\n");
    p.config.game.title = "Space Quest".into();
    p.config.game.code = "bspq".into();
    p.config.game.maker = "zz".into();
    p.config.game.version = 3;
    let out = build_ok(&p);
    let data = std::fs::read(out.rom_path.as_ref().unwrap()).unwrap();
    assert_eq!(&data[0xA0..0xAC], b"SPACE QUEST\0");
    assert_eq!(&data[0xAC..0xB0], b"BSPQ");
    assert_eq!(&data[0xB0..0xB2], b"ZZ");
    assert_eq!(data[0xBC], 3);
    assert_eq!(rom::validate_header(&data), Vec::<String>::new(), "real hardware would boot it");
    assert_eq!(out.rom_path.unwrap().file_name().unwrap(), "test.gba", "named after the game");
}

#[test]
fn the_runtime_library_is_compiled_once_per_cache() {
    let cache = tempfile::tempdir().unwrap();
    let opts = BuildOptions { cache_dir: Some(cache.path().to_path_buf()), ..Default::default() };
    let tc = Toolchain::locate().unwrap();
    let (_d1, p1) = project_with_main("#include <gba.h>\nint main(void) { while (1) vsync(); }\n");
    let (_d2, p2) = project_with_main("#include <gba.h>\nint main(void) { for (;;) vsync(); }\n");
    let first = build(&p1, &tc, &opts);
    assert!(first.success);
    assert!(first.log.contains("Compiled the GBADE runtime library"), "{}", first.log);
    let second = build(&p2, &tc, &opts);
    assert!(second.success);
    assert!(!second.log.contains("Compiled the GBADE runtime library"), "another project reuses the cache");
    let dirs: Vec<_> = std::fs::read_dir(cache.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(dirs.len(), 1, "one runtime, no temporary folders left: {dirs:?}");
}

#[test]
fn parallel_builds_share_the_runtime_cache_safely() {
    // The IDE and a terminal may build at the same time with a cold cache.
    let cache = tempfile::tempdir().unwrap();
    let opts = BuildOptions { cache_dir: Some(cache.path().to_path_buf()), ..Default::default() };
    let tc = Toolchain::locate().unwrap();
    let projects: Vec<_> =
        (0..4).map(|_| project_with_main("#include <gba.h>\nint main(void) { while (1) vsync(); }\n")).collect();
    std::thread::scope(|s| {
        for (_, p) in &projects {
            let (tc, opts) = (&tc, &opts);
            s.spawn(move || {
                let out = build(p, tc, opts);
                assert!(out.success, "{}", out.log);
            });
        }
    });
    let dirs: Vec<_> = std::fs::read_dir(cache.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
}
