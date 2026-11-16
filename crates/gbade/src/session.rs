//! A running game: the emulator plus everything GBADE knows about the ROM
//! (debug info, breakpoints, console output, why it is paused).

use gbade_emu::{Emulator, Keys, LogEntry, LogLevel, StopReason};
use gbade_toolchain::DebugInfo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The GBA refreshes the screen 59.7275 times per second.
pub const GBA_FPS: f64 = 16_777_216.0 / 280_896.0;

/// A line in the Console panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLine {
    pub level: LogLevel,
    pub from_game: bool,
    pub category: String,
    pub text: String,
    pub frame: u32,
}

/// Why the game is paused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseReason {
    /// The user pressed Pause.
    User,
    /// A breakpoint set in the editor margin was reached.
    Breakpoint,
    /// The game called BREAKPOINT().
    BreakpointMacro,
    /// The game called assert()/ASSERT()/debug_panic().
    Panic(String),
    /// The CPU hit an invalid instruction.
    Crash(String),
    /// Paused after a "step".
    Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseInfo {
    pub reason: PauseReason,
    pub pc: u32,
    /// Source location to show in the editor.
    pub location: Option<(PathBuf, u32)>,
    pub function: Option<String>,
}

impl PauseInfo {
    pub fn title(&self) -> String {
        match &self.reason {
            PauseReason::User => "Paused".into(),
            PauseReason::Breakpoint => "Stopped at a breakpoint".into(),
            PauseReason::BreakpointMacro => "BREAKPOINT() reached".into(),
            PauseReason::Panic(m) => format!("The game stopped: {m}"),
            PauseReason::Crash(m) => format!("The game crashed: {m}"),
            PauseReason::Step => "Stepped".into(),
        }
    }
}

pub struct Session {
    pub emu: Emulator,
    pub debug: Option<DebugInfo>,
    pub save_path: Option<PathBuf>,
    pub pause: Option<PauseInfo>,
    /// 1.0 = normal speed.
    pub speed: f64,
    accumulator: f64,
    pub console: Vec<ConsoleLine>,
    /// Number of console lines dropped because the console was full.
    pub console_dropped: usize,
    /// Breakpoint addresses currently installed, with their source lines.
    pub installed_breakpoints: Vec<(u32, PathBuf, u32)>,
    pub audio_buf: Vec<i16>,
    /// Emulation time of the last frame (for the performance display).
    pub last_frame_cost: Duration,
}

const CONSOLE_MAX: usize = 5000;

impl Session {
    /// Load a ROM (and its save file if it exists).
    pub fn new(rom: Vec<u8>, elf: Option<&[u8]>, save_path: Option<PathBuf>, audio_rate: u32) -> Result<Session, String> {
        let mut emu = Emulator::new();
        emu.set_audio_rate(audio_rate);
        let save = save_path.as_ref().and_then(|p| std::fs::read(p).ok());
        emu.load_rom(&rom, save.as_deref()).map_err(|e| e.to_string())?;
        let debug = elf.and_then(|e| DebugInfo::from_elf(e).ok());
        Ok(Session {
            emu,
            debug,
            save_path,
            pause: None,
            speed: 1.0,
            accumulator: 0.0,
            console: Vec::new(),
            console_dropped: 0,
            installed_breakpoints: Vec::new(),
            audio_buf: Vec::with_capacity(4096),
            last_frame_cost: Duration::ZERO,
        })
    }

    pub fn is_paused(&self) -> bool {
        self.pause.is_some()
    }

    pub fn pause_user(&mut self) {
        if self.pause.is_none() {
            self.pause = Some(self.pause_here(PauseReason::User));
        }
    }

    pub fn resume(&mut self) {
        self.pause = None;
        self.accumulator = 0.0;
    }

    pub fn reset(&mut self) {
        self.persist_save();
        self.emu.reset();
        self.pause = None;
        self.accumulator = 0.0;
        self.push_console(LogLevel::Info, false, "gbade", "Game reset.".into());
    }

    /// Write the cartridge save memory to disk (if the game has any).
    pub fn persist_save(&self) {
        if let Some(p) = &self.save_path {
            let data = self.emu.save_data();
            if !data.is_empty() {
                let _ = std::fs::write(p, data);
            }
        }
    }

    /// Install breakpoints from editor lines. Returns lines that have no code
    /// (so the editor can tell the user) — each mapped to the line actually used.
    pub fn set_breakpoints(&mut self, bps: &BTreeMap<PathBuf, BTreeSet<u32>>) -> Vec<(PathBuf, u32, Option<u32>)> {
        self.emu.clear_breakpoints();
        self.installed_breakpoints.clear();
        let mut report = Vec::new();
        let Some(debug) = &self.debug else { return report };
        for (file, lines) in bps {
            for &line in lines {
                match debug.addresses_for_line(file, line) {
                    Some((actual, addrs)) => {
                        for a in addrs {
                            if self.emu.add_breakpoint(a) {
                                self.installed_breakpoints.push((a, file.clone(), actual));
                            }
                        }
                        if actual != line {
                            report.push((file.clone(), line, Some(actual)));
                        }
                    }
                    None => report.push((file.clone(), line, None)),
                }
            }
        }
        report
    }

    /// Where the CPU is, as a PauseInfo.
    fn pause_here(&self, reason: PauseReason) -> PauseInfo {
        let pc = self.emu.cpu_state().pc();
        let (location, function) = match &self.debug {
            Some(d) => (
                d.line_for_address(pc).map(|(f, l)| (f.to_path_buf(), l)),
                d.function_at(pc).map(|s| s.name.clone()),
            ),
            None => (None, None),
        };
        PauseInfo { reason, pc, location, function }
    }

    fn push_console(&mut self, level: LogLevel, from_game: bool, category: &str, text: String) {
        if self.console.len() >= CONSOLE_MAX {
            self.console.drain(..CONSOLE_MAX / 10);
            self.console_dropped += CONSOLE_MAX / 10;
        }
        self.console.push(ConsoleLine {
            level,
            from_game,
            category: category.to_string(),
            text,
            frame: self.emu.frame_counter(),
        });
    }

    /// Turn emulator logs into console lines; detect GBADE markers.
    fn process_logs(&mut self, logs: Vec<LogEntry>) {
        for l in logs {
            let from_game = l.is_game_output();
            if from_game {
                if let Some(rest) = l.message.strip_prefix("\u{1}GBADE:PANIC ") {
                    let (loc, msg) = parse_marker(rest);
                    self.push_console(LogLevel::Error, true, &l.category, format!("PANIC: {rest}"));
                    let mut info = self.pause_here(PauseReason::Panic(msg));
                    if loc.is_some() {
                        info.location = loc;
                    }
                    self.pause = Some(info);
                    continue;
                }
                if let Some(rest) = l.message.strip_prefix("\u{1}GBADE:BREAK ") {
                    let (loc, _) = parse_marker(rest);
                    self.push_console(LogLevel::Info, true, &l.category, format!("BREAKPOINT() at {rest}"));
                    let mut info = self.pause_here(PauseReason::BreakpointMacro);
                    if loc.is_some() {
                        info.location = loc;
                    }
                    self.pause = Some(info);
                    continue;
                }
            }
            self.push_console(l.level, from_game, &l.category, l.message);
        }
    }

    /// Run one frame (used by the frame loop and by "Next frame").
    /// Returns false if execution stopped (breakpoint, crash...).
    pub fn run_one_frame(&mut self, keys: Keys) -> bool {
        self.emu.set_keys(keys);
        let t = std::time::Instant::now();
        let r = self.emu.run_frame();
        self.last_frame_cost = t.elapsed();
        self.audio_buf.clear();
        self.emu.drain_audio(&mut self.audio_buf);
        let logs = self.emu.take_logs();
        self.process_logs(logs);
        match r {
            StopReason::FrameDone => self.pause.is_none(),
            StopReason::Breakpoint(_) => {
                self.pause = Some(self.pause_here(PauseReason::Breakpoint));
                false
            }
            StopReason::IllegalInstruction(a) | StopReason::Other(a) => {
                let msg = format!(
                    "the CPU tried to run an invalid instruction at {a:#010x}. This usually means a bad \
                     function pointer, writing past the end of an array (corrupting the stack), or \
                     returning from main()."
                );
                self.push_console(LogLevel::Error, false, "gbade", msg.clone());
                self.pause = Some(self.pause_here(PauseReason::Crash(msg)));
                false
            }
            StopReason::Watchpoint(_) => {
                self.pause = Some(self.pause_here(PauseReason::Breakpoint));
                false
            }
        }
    }

    /// Advance emulation by real time `dt`. Returns how many frames ran and
    /// the audio they produced is left in `audio_out`.
    pub fn tick(&mut self, dt: Duration, keys: Keys, audio_out: &mut Vec<i16>) -> u32 {
        if self.pause.is_some() {
            return 0;
        }
        self.accumulator += dt.as_secs_f64() * self.speed * GBA_FPS;
        // Never try to catch up more than a few frames (e.g. after the window
        // was hidden), the game would just fast-forward.
        let max = (4.0 * self.speed).max(4.0);
        if self.accumulator > max {
            self.accumulator = max;
        }
        let mut ran = 0;
        while self.accumulator >= 1.0 {
            self.accumulator -= 1.0;
            let ok = self.run_one_frame(keys);
            ran += 1;
            if self.speed <= 1.0 {
                audio_out.extend_from_slice(&self.audio_buf);
            }
            if !ok {
                self.accumulator = 0.0;
                break;
            }
        }
        ran
    }

    /// Execute instructions until the source line changes (or a limit).
    pub fn step_line(&mut self) {
        let Some(debug) = &self.debug else {
            self.emu.step_instruction();
            self.pause = Some(self.pause_here(PauseReason::Step));
            return;
        };
        let start = debug.line_for_address(self.emu.cpu_state().pc()).map(|(f, l)| (f.to_path_buf(), l));
        for _ in 0..5_000_000 {
            self.emu.step_instruction();
            let pc = self.emu.cpu_state().pc();
            if let Some((f, l)) = debug.line_for_address(pc) {
                let here = Some((f.to_path_buf(), l));
                if here != start && debug.rows.iter().any(|r| r.address == pc && r.is_stmt) {
                    break;
                }
            }
        }
        let logs = self.emu.take_logs();
        self.process_logs(logs);
        self.pause = Some(self.pause_here(PauseReason::Step));
    }

    /// Run exactly one frame and stay paused.
    pub fn step_frame(&mut self, keys: Keys) {
        self.pause = None;
        if self.run_one_frame(keys) {
            self.pause = Some(self.pause_here(PauseReason::Step));
        }
    }

    pub fn clear_console(&mut self) {
        self.console.clear();
        self.console_dropped = 0;
    }

    #[allow(dead_code)]
    pub fn debug_files(&self) -> Vec<&Path> {
        self.debug.as_ref().map(|d| d.files.iter().map(PathBuf::as_path).collect()).unwrap_or_default()
    }
}

/// Parse "path/file.c:12: message" or "path/file.c:12".
pub fn parse_marker(s: &str) -> (Option<(PathBuf, u32)>, String) {
    // Find ":<digits>" followed by ':' or end.
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && (j == bytes.len() || bytes[j] == b':') {
                let file = PathBuf::from(&s[..i]);
                let line: u32 = s[i + 1..j].parse().unwrap_or(0);
                let msg = s.get(j + 1..).unwrap_or("").trim().to_string();
                return (Some((file, line)), msg);
            }
        }
        i += 1;
    }
    (None, s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_parsing() {
        let (loc, msg) = parse_marker("/p/src/main.c:6: assertion failed: lives > 0");
        assert_eq!(loc, Some((PathBuf::from("/p/src/main.c"), 6)));
        assert_eq!(msg, "assertion failed: lives > 0");
        let (loc, msg) = parse_marker("/p/src/game.c:120");
        assert_eq!(loc, Some((PathBuf::from("/p/src/game.c"), 120)));
        assert_eq!(msg, "");
        let (loc, _) = parse_marker("abort: abort() called");
        assert!(loc.is_none());
    }

    #[test]
    fn gba_fps_is_right() {
        assert!((GBA_FPS - 59.7275).abs() < 0.001);
    }

    #[test]
    fn markers_with_windows_paths() {
        let (loc, msg) = parse_marker("C:\\p\\src\\main.c:6: assertion failed: lives > 0");
        assert_eq!(loc, Some((PathBuf::from("C:\\p\\src\\main.c"), 6)));
        assert_eq!(msg, "assertion failed: lives > 0");
    }

    // ---- sessions on real games ---------------------------------------------

    use gbade_toolchain::{BuildOptions, Project, Toolchain};

    /// Build `main_c` and start a session on it (with a save file path).
    fn session(main_c: &str) -> (tempfile::TempDir, PathBuf, Session) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("gbade.toml"), "[game]\nname = \"Session\"\n").unwrap();
        std::fs::write(dir.path().join("src/main.c"), main_c).unwrap();
        let p = Project::open(dir.path()).unwrap();
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gbade-test-cache");
        let opts = BuildOptions { cache_dir: Some(cache), ..Default::default() };
        let out = gbade_toolchain::build(&p, &Toolchain::locate().unwrap(), &opts);
        assert!(out.success, "{}", out.log);
        let rom = std::fs::read(out.rom_path.unwrap()).unwrap();
        let elf = std::fs::read(out.elf_path.unwrap()).unwrap();
        let main = p.src_dir().join("main.c");
        let s = Session::new(rom, Some(&elf), Some(p.save_path()), 48_000).unwrap();
        (dir, main, s)
    }

    const COUNTER: &str = "#include <gba.h>
#include <stdio.h>
volatile int frames;
__attribute__((noinline)) void tick(void) {

    frames++;
}
int main(void) {
    printf(\"started\\n\");
    while (1) {
        vsync();
        tick();
    }
}
";

    /// One GBA frame of real time.
    const FRAME: Duration = Duration::from_nanos(16_742_706);

    #[test]
    fn real_time_runs_the_game_at_its_own_speed() {
        let (_dir, _main, mut s) = session(COUNTER);
        let mut audio = Vec::new();
        let ran: u32 = (0..60).map(|_| s.tick(FRAME, Keys::NONE, &mut audio)).sum();
        assert!((59..=60).contains(&ran), "{ran} frames in one second");
        assert!(!audio.is_empty(), "sound is produced at normal speed");

        // Twice as fast: twice as many frames, and no sound (it would be garbled).
        s.speed = 2.0;
        audio.clear();
        let ran: u32 = (0..60).map(|_| s.tick(FRAME, Keys::NONE, &mut audio)).sum();
        assert!((119..=120).contains(&ran), "{ran}");
        assert!(audio.is_empty());

        // After a long freeze (window hidden...), no fast-forward to catch up.
        s.speed = 1.0;
        assert!(s.tick(Duration::from_secs(5), Keys::NONE, &mut audio) <= 4);

        // Paused: time does not move the game.
        s.pause_user();
        assert_eq!(s.tick(Duration::from_secs(1), Keys::NONE, &mut audio), 0);
        assert_eq!(s.pause.as_ref().unwrap().title(), "Paused");
        s.resume();
        assert!(s.tick(FRAME * 2, Keys::NONE, &mut audio) >= 1);
    }

    #[test]
    fn game_output_reaches_the_console() {
        let (_dir, _main, mut s) = session(COUNTER);
        s.run_one_frame(Keys::NONE);
        let line = s.console.iter().find(|l| l.text == "started").expect("printf output");
        assert!(line.from_game);
        s.clear_console();
        assert!(s.console.is_empty());
    }

    #[test]
    fn the_console_keeps_the_newest_lines() {
        let (_dir, _main, mut s) = session(COUNTER);
        for i in 0..CONSOLE_MAX + 10 {
            s.push_console(LogLevel::Info, true, "gba.debug", format!("line {i}"));
        }
        assert!(s.console.len() <= CONSOLE_MAX);
        assert!(s.console_dropped > 0);
        assert_eq!(s.console.last().unwrap().text, format!("line {}", CONSOLE_MAX + 9));
        assert_eq!(s.console.len() + s.console_dropped, CONSOLE_MAX + 10, "nothing lost silently");
    }

    #[test]
    fn editor_breakpoints_stop_on_their_line_and_report_moved_ones() {
        let (_dir, main, mut s) = session(COUNTER);
        // Line 5 is blank: the breakpoint moves to line 6 (`frames++`).
        // Line 40 is past the end of the file: it cannot be installed.
        let bps: BTreeMap<PathBuf, BTreeSet<u32>> = [(main.clone(), [5, 40].into_iter().collect())].into_iter().collect();
        let report = s.set_breakpoints(&bps);
        assert!(report.contains(&(main.clone(), 5, Some(6))), "{report:?}");
        assert!(report.contains(&(main.clone(), 40, None)), "{report:?}");
        assert!(!s.installed_breakpoints.is_empty());

        let mut stopped = false;
        for _ in 0..10 {
            if !s.run_one_frame(Keys::NONE) {
                stopped = true;
                break;
            }
        }
        assert!(stopped, "the breakpoint stops the frame loop");
        let pause = s.pause.clone().unwrap();
        assert_eq!(pause.reason, PauseReason::Breakpoint);
        assert_eq!(pause.location, Some((main.clone(), 6)));
        assert_eq!(pause.function.as_deref(), Some("tick"));
        assert_eq!(pause.title(), "Stopped at a breakpoint");

        // Removing every breakpoint lets the game run again.
        s.set_breakpoints(&BTreeMap::new());
        assert!(s.installed_breakpoints.is_empty());
        s.resume();
        for _ in 0..5 {
            assert!(s.run_one_frame(Keys::NONE));
        }
    }

    #[test]
    fn stepping_moves_by_line_or_by_frame() {
        let (_dir, main, mut s) = session(COUNTER);
        let bps: BTreeMap<PathBuf, BTreeSet<u32>> = [(main.clone(), [6].into_iter().collect())].into_iter().collect();
        s.set_breakpoints(&bps);
        while s.run_one_frame(Keys::NONE) {}
        assert_eq!(s.pause.as_ref().unwrap().location, Some((main.clone(), 6)));
        s.set_breakpoints(&BTreeMap::new());

        s.step_line();
        let after = s.pause.clone().unwrap();
        assert_eq!(after.reason, PauseReason::Step);
        assert_ne!(after.location, Some((main.clone(), 6)), "a step leaves the line");
        assert!(after.location.is_some());

        let frame = s.emu.frame_counter();
        s.step_frame(Keys::NONE);
        assert_eq!(s.emu.frame_counter(), frame + 1);
        assert!(s.is_paused(), "still paused after one frame");
    }

    #[test]
    fn a_failed_assert_pauses_at_its_line() {
        let (_dir, main, mut s) = session(
            "#include <gba.h>\n#include <assert.h>\nint main(void) {\n    int lives = 0;\n    assert(lives > 0);\n    while (1) vsync();\n}\n",
        );
        for _ in 0..5 {
            if !s.run_one_frame(Keys::NONE) {
                break;
            }
        }
        let pause = s.pause.clone().expect("paused by the assert");
        assert_eq!(pause.reason, PauseReason::Panic("assertion failed: lives > 0".into()));
        assert_eq!(pause.location.as_ref().map(|(_, l)| *l), Some(5));
        assert!(pause.location.as_ref().unwrap().0.ends_with("main.c"), "{main:?}");
        assert!(pause.title().contains("lives > 0"));
        assert!(s.console.iter().any(|l| l.text.starts_with("PANIC:")));
        // Time does not move a stopped game.
        assert_eq!(s.tick(Duration::from_secs(1), Keys::NONE, &mut Vec::new()), 0);
    }

    #[test]
    fn breakpoint_macro_pauses_and_can_continue() {
        let (_dir, _main, mut s) = session(
            "#include <gba.h>\n#include <stdio.h>\nint main(void) {\n    BREAKPOINT();\n    printf(\"after\\n\");\n    while (1) vsync();\n}\n",
        );
        for _ in 0..5 {
            if !s.run_one_frame(Keys::NONE) {
                break;
            }
        }
        let pause = s.pause.clone().expect("paused by BREAKPOINT()");
        assert_eq!(pause.reason, PauseReason::BreakpointMacro);
        assert_eq!(pause.location.as_ref().map(|(_, l)| *l), Some(4));
        s.resume();
        for _ in 0..3 {
            s.run_one_frame(Keys::NONE);
        }
        assert!(s.console.iter().any(|l| l.text == "after"), "the game continues after BREAKPOINT()");
    }

    #[test]
    fn a_crash_is_explained() {
        let (_dir, _main, mut s) = session(
            "#include <gba.h>\ntypedef void (*fn)(void);\nint main(void) {\n\
             u32 *ram = (u32 *)0x02000000;\n\
             for (int i = 0; i < 16; i++) ram[i] = 0xE7F000F0;\n\
             ((fn)0x02000000)();\n\
             while (1) vsync();\n}\n",
        );
        let mut crashed = false;
        for _ in 0..10 {
            if !s.run_one_frame(Keys::NONE) {
                crashed = true;
                break;
            }
        }
        assert!(crashed);
        let pause = s.pause.clone().unwrap();
        assert!(matches!(pause.reason, PauseReason::Crash(_)), "{pause:?}");
        assert!(pause.title().contains("bad function pointer"), "{}", pause.title());
    }

    #[test]
    fn reset_restarts_the_game_and_keeps_its_save() {
        let (_dir, _main, mut s) = session(
            "#include <gba.h>\nint main(void) {\n    u8 v = 42;\n    save_write(0, &v, 1);\n    while (1) vsync();\n}\n",
        );
        for _ in 0..5 {
            s.run_one_frame(Keys::NONE);
        }
        s.pause_user();
        s.reset();
        assert_eq!(s.emu.frame_counter(), 0);
        assert!(!s.is_paused());
        assert!(s.console.iter().any(|l| l.text == "Game reset."));
        let save = std::fs::read(s.save_path.as_ref().unwrap()).expect("save written on reset");
        assert_eq!(save[0], 42);
    }

    #[test]
    fn a_session_starts_with_an_existing_save() {
        let src = "#include <gba.h>\n#include <stdio.h>\nint main(void) {\n    u8 v;\n    save_read(0, &v, 1);\n    printf(\"saved=%d\\n\", v);\n    while (1) vsync();\n}\n";
        let (_dir, _main, s) = session(src);
        let save_path = s.save_path.clone().unwrap();
        let mut data = vec![0xFFu8; 32 * 1024];
        data[0] = 7;
        std::fs::write(&save_path, &data).unwrap();
        // A new session on the same ROM reads it.
        let rom_dir = save_path.parent().unwrap();
        let rom = std::fs::read(rom_dir.join("session.gba")).unwrap();
        let mut s = Session::new(rom, None, Some(save_path), 48_000).unwrap();
        s.run_one_frame(Keys::NONE);
        assert!(s.console.iter().any(|l| l.text == "saved=7"), "{:?}", s.console);
        assert!(s.debug.is_none(), "no ELF: no debug info, but the game runs");
    }

    #[test]
    fn garbage_is_not_a_rom() {
        assert!(Session::new(vec![0; 64], None, None, 48_000).is_err());
    }
}
