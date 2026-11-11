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
}
