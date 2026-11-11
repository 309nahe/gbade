//! End-to-end tests of the GBADE runtime and SDK: each test compiles a small
//! C program with the real toolchain and checks its behaviour in mGBA.

mod common;
use common::*;
use gbade_emu::{Emulator, StopReason};
use gbade_toolchain::{DebugInfo, Severity, Toolchain, build};

/// Build `main_c`, run it for `frames` frames and return the console lines.
fn run_program(main_c: &str, frames: usize) -> Vec<String> {
    let (_dir, p) = project_with_main(main_c);
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(frames);
    run.game_output()
}

#[test]
fn integer_and_64bit_arithmetic() {
    let out = run_program(
        r#"
        #include <gba.h>
        #include <stdio.h>
        volatile int a = 1000, b = 7, neg = -1000;
        volatile long long big = 123456789012345LL, d = 1000;
        volatile unsigned long long ubig = 18446744073709551615ULL;
        int main(void) {
            printf("div %d mod %d\n", a / b, a % b);
            printf("neg %d %d\n", neg / b, neg % b);
            printf("ll %lld %lld\n", big / d, big % d);
            printf("ull %llu\n", ubig / 3);
            printf("mul %lld\n", big * 3);
            printf("shift %lld\n", big >> 7);
            while (1) vsync();
        }"#,
        5,
    );
    assert!(out.contains(&"div 142 mod 6".to_string()), "{out:?}");
    assert!(out.contains(&"neg -142 -6".to_string()), "{out:?}");
    assert!(out.contains(&"ll 123456789012 345".to_string()), "{out:?}");
    assert!(out.contains(&"ull 6148914691236517205".to_string()), "{out:?}");
    assert!(out.contains(&"mul 370370367037035".to_string()), "{out:?}");
    assert!(out.contains(&"shift 964506164158".to_string()), "{out:?}");
}

#[test]
fn soft_float_works() {
    let out = run_program(
        r#"
        #include <gba.h>
        #include <stdio.h>
        volatile float x = 1.5f, y = 2.25f;
        volatile double dx = 3.0;
        int main(void) {
            printf("f %.3f\n", (double)(x * y + 1.0f));
            printf("d %.4f\n", dx / 7.0);
            printf("cmp %d %d %d\n", x < y, x > y, x == 1.5f);
            printf("conv %d %d\n", (int)(y * 10.0f), (int)-2.75f);
            while (1) vsync();
        }"#,
        5,
    );
    assert!(out.contains(&"f 4.375".to_string()), "{out:?}");
    assert!(out.contains(&"d 0.4286".to_string()), "{out:?}");
    assert!(out.contains(&"cmp 1 0 1".to_string()), "{out:?}");
    assert!(out.contains(&"conv 22 -2".to_string()), "{out:?}");
}

#[test]
fn libc_strings_malloc_and_rand() {
    let out = run_program(
        r#"
        #include <gba.h>
        #include <stdio.h>
        #include <stdlib.h>
        #include <string.h>
        int cmp(const void *a, const void *b) { return *(const int *)a - *(const int *)b; }
        int main(void) {
            char buf[32];
            strcpy(buf, "hello");
            strcat(buf, " gba");
            printf("str '%s' %d %d\n", buf, (int)strlen(buf), strcmp(buf, "hello gba") == 0);
            int *p = malloc(100 * sizeof(int));
            int *q = malloc(50);
            for (int i = 0; i < 100; i++) p[i] = i * i;
            printf("heap %d %d %d\n", p[99], p != 0, q != 0 && q != p);
            printf("ewram %d\n", (unsigned)p >= 0x02000000 && (unsigned)p < 0x02040000);
            free(p); free(q);
            int *r = malloc(64);
            printf("reuse %d\n", r != 0);
            int arr[5] = {5, 3, 9, 1, 7};
            qsort(arr, 5, sizeof(int), cmp);
            printf("sorted %d %d %d %d %d\n", arr[0], arr[1], arr[2], arr[3], arr[4]);
            printf("atoi %d %ld\n", atoi("-123"), strtol("0x1F", 0, 0));
            rand_seed(42);
            int r1 = rand_range(1, 6);
            printf("dice %d\n", r1 >= 1 && r1 <= 6);
            char tmp[16]; snprintf(tmp, sizeof tmp, "%05d|%x", 42, 255);
            printf("fmt %s\n", tmp);
            while (1) vsync();
        }"#,
        5,
    );
    for want in ["str 'hello gba' 9 1", "heap 9801 1 1", "ewram 1", "reuse 1", "sorted 1 3 5 7 9", "atoi -123 31", "dice 1", "fmt 00042|ff"] {
        assert!(out.contains(&want.to_string()), "missing {want:?} in {out:?}");
    }
}

#[test]
fn iwram_code_data_and_ewram_sections() {
    let (_dir, p) = project_with_main(
        r#"
        #include <gba.h>
        #include <stdio.h>
        IWRAM_CODE int fast_sum(int n) { int s = 0; for (int i = 0; i < n; i++) s += i; return s; }
        EWRAM_DATA int table[4] = {10, 20, 30, 40};
        EWRAM_BSS int zeros[1000];
        int initialised = 77;
        int main(void) {
            printf("fast %d at %p\n", fast_sum(100), (void *)fast_sum);
            printf("table %d %d at %p\n", table[0], table[3], (void *)table);
            int z = 0; for (int i = 0; i < 1000; i++) z |= zeros[i];
            printf("zeros %d init %d\n", z, initialised);
            while (1) vsync();
        }"#,
    );
    let out = build_ok(&p);
    let mut run = Run::new(&out);
    run.frames(5);
    let o = run.game_output();
    assert!(o.iter().any(|l| l.starts_with("fast 4950 at 0x03")), "{o:?}");
    assert!(o.iter().any(|l| l.starts_with("table 10 40 at 0x02")), "{o:?}");
    assert!(o.contains(&"zeros 0 init 77".to_string()), "{o:?}");
    let mem = out.memory.unwrap();
    assert!(mem.ewram >= 4000, "ewram usage {}", mem.ewram);
}

#[test]
fn assert_failure_reports_file_and_line() {
    let out = run_program(
        r#"
        #include <gba.h>
        #include <assert.h>
        int main(void) {
            int lives = 0;
            assert(lives > 0);
            while (1) vsync();
        }"#,
        5,
    );
    let panic = out.iter().find(|l| l.contains("GBADE:PANIC")).expect("panic message");
    assert!(panic.contains("main.c:6"), "{panic}");
    assert!(panic.contains("lives > 0"), "{panic}");
}

#[test]
fn sram_save_survives_reload() {
    let src = r#"
        #include <gba.h>
        #include <stdio.h>
        typedef struct { u32 magic; int count; } Save;
        int main(void) {
            Save s;
            save_read(0, &s, sizeof s);
            if (s.magic != 0x47424144) { s.magic = 0x47424144; s.count = 0; }
            s.count++;
            save_write(0, &s, sizeof s);
            printf("boot %d\n", s.count);
            while (1) vsync();
        }"#;
    let (_dir, p) = project_with_main(src);
    let out = build_ok(&p);
    let rom = std::fs::read(out.rom_path.unwrap()).unwrap();

    let mut emu = Emulator::new();
    emu.load_rom(&rom, None).unwrap();
    for _ in 0..5 {
        emu.run_frame();
    }
    let logs = emu.take_logs();
    assert!(logs.iter().any(|l| l.message == "boot 1"), "{logs:?}");
    let save = emu.save_data();
    assert!(save.len() >= 8, "save size {}", save.len());

    let mut emu2 = Emulator::new();
    emu2.load_rom(&rom, Some(&save)).unwrap();
    for _ in 0..5 {
        emu2.run_frame();
    }
    let logs = emu2.take_logs();
    assert!(logs.iter().any(|l| l.message == "boot 2"), "{logs:?}");
}

#[test]
fn breakpoint_on_source_line_stops_there() {
    let src = "#include <gba.h>\n\
               #include <stdio.h>\n\
               volatile int counter;\n\
               __attribute__((noinline)) void tick(void) {\n\
                   counter++;\n\
               }\n\
               int main(void) {\n\
                   while (1) {\n\
                       vsync();\n\
                       tick();\n\
                   }\n\
               }\n";
    let (_dir, p) = project_with_main(src);
    let out = build_ok(&p);
    let elf = std::fs::read(out.elf_path.as_ref().unwrap()).unwrap();
    let info = DebugInfo::from_elf(&elf).unwrap();
    let main_c = p.src_dir().join("main.c");
    let (line, addrs) = info.addresses_for_line(&main_c, 5).expect("line 5 has code");
    assert_eq!(line, 5);
    assert!(!addrs.is_empty());
    // A blank/brace line snaps to the next line with code.
    let (snapped, _) = info.addresses_for_line(&main_c, 4).unwrap();
    assert!(snapped >= 4);

    let rom = std::fs::read(out.rom_path.as_ref().unwrap()).unwrap();
    let mut emu = Emulator::new();
    emu.load_rom(&rom, None).unwrap();
    for a in &addrs {
        assert!(emu.add_breakpoint(*a));
    }
    let mut stop = StopReason::FrameDone;
    for _ in 0..10 {
        stop = emu.run_frame();
        if stop != StopReason::FrameDone {
            break;
        }
    }
    let StopReason::Breakpoint(addr) = stop else { panic!("expected breakpoint, got {stop:?}") };
    assert!(addrs.contains(&addr));
    let cpu = emu.cpu_state();
    assert_eq!(cpu.pc(), addr, "PC should be at the breakpoint");
    let (file, l) = info.line_for_address(cpu.pc()).unwrap();
    assert!(file.ends_with("main.c"));
    assert_eq!(l, 5);
    assert_eq!(info.function_at(cpu.pc()).unwrap().name, "tick");
    let (text, _) = emu.disassemble(cpu.pc(), cpu.thumb());
    assert!(!text.is_empty());

    // Resume: we hit it again on the next frame.
    let again = emu.run_frame();
    assert!(matches!(again, StopReason::Breakpoint(_) | StopReason::FrameDone));
    emu.clear_breakpoints();
    for _ in 0..3 {
        assert_eq!(emu.run_frame(), StopReason::FrameDone);
    }
}

#[test]
fn compile_errors_are_reported_with_location_and_hint() {
    let (_dir, p) = project_with_main("#include <gba.h>\nint main(void) {\n    int x = 1\n    return y;\n}\n");
    let tc = Toolchain::locate().unwrap();
    let out = build(&p, &tc, &test_options());
    assert!(!out.success);
    let errs: Vec<_> = out.errors().collect();
    assert!(!errs.is_empty(), "{}", out.log);
    let e = errs.iter().find(|d| d.message.contains("expected ';'")).expect("missing-semicolon error");
    assert_eq!(e.line, 3);
    assert!(e.file.as_ref().unwrap().ends_with("main.c"));
    assert!(e.hint.is_some());
    assert!(out.rom_path.is_none());
}

#[test]
fn link_errors_point_at_the_caller() {
    let (_dir, p) = project_with_main("#include <gba.h>\nvoid draw_hero(void);\nint main(void) {\n    draw_hero();\n    while (1) vsync();\n}\n");
    let tc = Toolchain::locate().unwrap();
    let out = build(&p, &tc, &test_options());
    assert!(!out.success);
    let e = out.errors().next().expect("an error");
    assert_eq!(e.message, "undefined symbol: draw_hero");
    assert_eq!(e.line, 4, "{e:?}");
    assert!(e.hint.as_ref().unwrap().contains("draw_hero"));
}

#[test]
fn iwram_overflow_gets_a_friendly_hint() {
    let (_dir, p) = project_with_main("#include <gba.h>\nint huge[20000];\nint main(void) { huge[1] = 2; while (1) vsync(); }\n");
    let tc = Toolchain::locate().unwrap();
    let out = build(&p, &tc, &test_options());
    assert!(!out.success, "{}", out.log);
    assert!(out.errors().any(|d| d.hint.as_deref().is_some_and(|h| h.contains("EWRAM_BSS"))), "{:#?}", out.diagnostics);
}

#[test]
fn warnings_do_not_fail_the_build() {
    let (_dir, p) = project_with_main("#include <gba.h>\nint main(void) {\n    int unused;\n    while (1) vsync();\n}\n");
    let out = build_ok(&p);
    assert!(out.diagnostics.iter().any(|d| d.severity == Severity::Warning && d.line == 3), "{:#?}", out.diagnostics);
}

#[test]
fn crash_stops_emulator_with_reason() {
    let (_dir, p) = project_with_main(
        r#"
        #include <gba.h>
        typedef void (*fn)(void);
        int main(void) {
            u32 *ram = (u32 *)0x02000000;
            for (int i = 0; i < 16; i++) ram[i] = 0xE7F000F0;
            ((fn)0x02000000)();
            while (1) vsync();
        }"#,
    );
    let out = build_ok(&p);
    let rom = std::fs::read(out.rom_path.unwrap()).unwrap();
    let mut emu = Emulator::new();
    emu.load_rom(&rom, None).unwrap();
    let mut stopped = None;
    for _ in 0..10 {
        let r = emu.run_frame();
        if r != StopReason::FrameDone {
            stopped = Some(r);
            break;
        }
    }
    let logs = emu.take_logs();
    // mGBA either enters the debugger (illegal op) or logs a game error.
    assert!(
        matches!(stopped, Some(StopReason::IllegalInstruction(_)) | Some(StopReason::Other(_)))
            || logs.iter().any(|l| l.message.to_lowercase().contains("undefined") || l.message.to_lowercase().contains("illegal")),
        "stop={stopped:?} logs={logs:?}"
    );
}
