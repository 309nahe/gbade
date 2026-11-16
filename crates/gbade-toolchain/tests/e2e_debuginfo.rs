//! Debug information read from real builds: what the debugger, the
//! breakpoints and the inspectors rely on. A two-file program with code in
//! ROM (Thumb) and in IWRAM (ARM), variables in EWRAM, and a function the
//! linker removes.

mod common;
use common::*;
use gbade_emu::{Emulator, StopReason};
use gbade_toolchain::DebugInfo;
use std::path::Path;

const MAIN_C: &str = "#include <gba.h>
#include \"game.h\"
EWRAM_BSS int score;
EWRAM_BSS char big_buffer[10000];
volatile int values[8];
int main(void) {
    while (1) {
        vsync();
        score += draw(score & 7);
        score += fast_add(score, 1);
    }
}
";

/// Line numbers below refer to this file.
const GAME_C: &str = "#include <gba.h>
#include \"game.h\"
int draw(int n) {
    int total = 0;

    for (int i = 0; i < n; i++) {
        total += values[i];
    }
    return total;
}
IWRAM_CODE int fast_add(int a, int b) {
    return a + b;
}
int never_called(int x) {
    return x * 1234;
}
";

const GAME_H: &str = "extern volatile int values[8];\nint draw(int n);\nint fast_add(int a, int b);\nint never_called(int x);\n";

fn build_game() -> (tempfile::TempDir, gbade_toolchain::Project, gbade_toolchain::BuildOutput, DebugInfo) {
    let (dir, p) = project_with_main(MAIN_C);
    std::fs::write(p.src_dir().join("game.c"), GAME_C).unwrap();
    std::fs::write(p.src_dir().join("game.h"), GAME_H).unwrap();
    let out = build_ok(&p);
    let info = DebugInfo::from_elf(&std::fs::read(out.elf_path.as_ref().unwrap()).unwrap()).unwrap();
    (dir, p, out, info)
}

#[test]
fn symbols_have_the_right_memory_regions() {
    let (_dir, _p, out, info) = build_game();
    let main = info.symbol("main").expect("main");
    assert!(main.is_function);
    assert_eq!(main.address >> 24, 0x08, "ROM code");
    assert_eq!(main.address & 1, 0, "Thumb bit cleared from function addresses");
    assert!(main.size > 0);

    let fast = info.symbol("fast_add").expect("fast_add");
    assert!(fast.is_function);
    assert_eq!(fast.address >> 24, 0x03, "IWRAM_CODE runs from IWRAM");

    let score = info.symbol("score").expect("score");
    assert!(!score.is_function);
    assert_eq!(score.address >> 24, 0x02, "EWRAM_BSS lives in EWRAM");
    assert_eq!(score.size, 4);
    assert_eq!(info.symbol_at(score.address + 2).unwrap().name, "score");

    // Unused functions are removed by the linker (--gc-sections), and so
    // are their line-table rows.
    assert!(info.symbol("never_called").is_none());

    // The memory report counts the EWRAM buffer.
    assert!(out.memory.unwrap().ewram >= 10_000, "{:?}", out.memory);
}

#[test]
fn lines_point_into_the_right_file() {
    let (_dir, p, _out, info) = build_game();
    let game_c = p.src_dir().join("game.c");
    let draw = info.symbol("draw").unwrap().clone();

    let (file, _) = info.line_for_address(draw.address).expect("draw has line info");
    assert!(file.ends_with("game.c"), "{}", file.display());
    assert_eq!(info.function_at(draw.address + 2).unwrap().name, "draw");

    // Line 7 (`total += values[i];`) is inside draw. Projects build with
    // -O2 by default; the volatile reads keep the line in the code.
    let (line, addrs) = info.addresses_for_line(&game_c, 7).expect("line 7 has code");
    assert_eq!(line, 7);
    assert!(addrs.iter().all(|a| (draw.address..draw.address + draw.size).contains(a)), "{addrs:x?}");
    // Line 5 is blank: the breakpoint moves down to the `for`.
    let (snapped, _) = info.addresses_for_line(&game_c, 5).unwrap();
    assert_eq!(snapped, 6);
    // Lines of the removed function have no code any more.
    assert_eq!(info.addresses_for_line(&game_c, 15), None);
    // Both source files are known, and paths are absolute.
    assert!(info.files.iter().any(|f| f.ends_with("main.c")));
    assert!(info.files.iter().all(|f| f.is_absolute()), "{:?}", info.files);
}

/// Run until a breakpoint, at most a few frames.
fn run_to_breakpoint(emu: &mut Emulator) -> u32 {
    for _ in 0..10 {
        match emu.run_frame() {
            StopReason::FrameDone => continue,
            StopReason::Breakpoint(a) => return a,
            other => panic!("unexpected stop {other:?}"),
        }
    }
    panic!("no breakpoint hit");
}

#[test]
fn breakpoints_stop_in_another_file_and_in_iwram() {
    let (_dir, p, out, info) = build_game();
    let rom = std::fs::read(out.rom_path.as_ref().unwrap()).unwrap();
    let mut emu = Emulator::new();
    emu.load_rom(&rom, None).unwrap();

    // In game.c, Thumb code in ROM.
    let (_, addrs) = info.addresses_for_line(&p.src_dir().join("game.c"), 7).unwrap();
    for a in &addrs {
        emu.add_breakpoint(*a);
    }
    let pc = run_to_breakpoint(&mut emu);
    assert!(emu.cpu_state().thumb());
    let (file, line) = info.line_for_address(pc).unwrap();
    assert!(file.ends_with("game.c"));
    assert_eq!(line, 7);
    emu.clear_breakpoints();

    // In IWRAM: ARM code copied there by the startup code.
    let (_, addrs) = info.addresses_for_line(&p.src_dir().join("game.c"), 12).unwrap();
    assert!(addrs.iter().all(|a| a >> 24 == 0x03), "{addrs:x?}");
    for a in &addrs {
        emu.add_breakpoint(*a);
    }
    let pc = run_to_breakpoint(&mut emu);
    assert!(!emu.cpu_state().thumb(), "IWRAM_CODE is ARM code");
    assert_eq!(info.function_at(pc).unwrap().name, "fast_add");
    assert_eq!(info.line_for_address(pc).unwrap().1, 12);
}

#[test]
fn a_build_without_debug_lines_still_has_symbols() {
    // Stripping is not something GBADE does, but foreign ELFs may lack
    // DWARF: symbols must still work and lines are simply absent.
    let (_dir, _p, out, _info) = build_game();
    let elf = std::fs::read(out.elf_path.as_ref().unwrap()).unwrap();
    let info = DebugInfo::from_elf(&elf).unwrap();
    let no_lines = DebugInfo { rows: Vec::new(), files: Vec::new(), ..info };
    let main = no_lines.symbol("main").unwrap();
    assert_eq!(no_lines.line_for_address(main.address), None);
    assert_eq!(no_lines.addresses_for_line(Path::new("main.c"), 5), None);
    assert_eq!(no_lines.function_at(main.address).unwrap().name, "main");
}
