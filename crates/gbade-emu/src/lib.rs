//! # gbade-emu
//!
//! Safe Rust wrapper around the [mGBA](https://mgba.io) Game Boy Advance
//! emulator core (MPL-2.0, by Vicki Pfau and contributors).
//!
//! The C side lives in `shim/gbade_shim.c`; this module only converts between
//! Rust and C types and owns the emulator handle.
//!
//! ```no_run
//! use gbade_emu::{Emulator, Keys};
//! let rom = std::fs::read("game.gba").unwrap();
//! let mut emu = Emulator::new();
//! emu.load_rom(&rom, None).unwrap();
//! emu.set_keys(Keys::A);
//! emu.run_frame();
//! let pixels = emu.framebuffer_rgba(); // 240 * 160 * 4 bytes
//! ```

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::{Arc, Mutex};

/// Width of the GBA screen in pixels.
pub const SCREEN_WIDTH: usize = 240;
/// Height of the GBA screen in pixels.
pub const SCREEN_HEIGHT: usize = 160;

mod ffi {
    use super::*;

    #[repr(C)]
    pub struct GbadeEmu {
        _private: [u8; 0],
    }

    pub type LogFn = extern "C" fn(*mut c_void, c_int, *const c_char, *const c_char);

    unsafe extern "C" {
        pub fn gbade_emu_create() -> *mut GbadeEmu;
        pub fn gbade_emu_destroy(emu: *mut GbadeEmu);
        pub fn gbade_emu_set_log_callback(emu: *mut GbadeEmu, f: LogFn, ctx: *mut c_void);
        pub fn gbade_emu_load_rom(
            emu: *mut GbadeEmu,
            rom: *const u8,
            rom_len: usize,
            save: *const u8,
            save_len: usize,
        ) -> c_int;
        pub fn gbade_emu_reset(emu: *mut GbadeEmu);
        pub fn gbade_emu_run_frame(emu: *mut GbadeEmu) -> c_int;
        pub fn gbade_emu_step(emu: *mut GbadeEmu) -> c_int;
        pub fn gbade_emu_stop_address(emu: *mut GbadeEmu) -> u32;
        pub fn gbade_emu_frame_counter(emu: *mut GbadeEmu) -> u32;
        pub fn gbade_emu_framebuffer(emu: *mut GbadeEmu) -> *const u32;
        pub fn gbade_emu_set_keys(emu: *mut GbadeEmu, keys: u32);
        pub fn gbade_emu_set_audio_rate(emu: *mut GbadeEmu, rate: u32);
        pub fn gbade_emu_read_audio(emu: *mut GbadeEmu, out: *mut i16, max_frames: usize) -> usize;
        pub fn gbade_emu_read(emu: *mut GbadeEmu, address: u32, width: c_int) -> u32;
        pub fn gbade_emu_read_block(emu: *mut GbadeEmu, address: u32, out: *mut u8, len: usize);
        pub fn gbade_emu_write(emu: *mut GbadeEmu, address: u32, value: u32, width: c_int);
        pub fn gbade_emu_cpu_state(emu: *mut GbadeEmu, regs: *mut u32, cpsr: *mut u32);
        pub fn gbade_emu_disassemble(
            emu: *mut GbadeEmu,
            address: u32,
            thumb: c_int,
            out: *mut c_char,
            out_len: usize,
        ) -> c_int;
        pub fn gbade_emu_add_breakpoint(emu: *mut GbadeEmu, address: u32) -> c_int;
        pub fn gbade_emu_clear_breakpoints(emu: *mut GbadeEmu);
        pub fn gbade_emu_state_size(emu: *mut GbadeEmu) -> usize;
        pub fn gbade_emu_save_state(emu: *mut GbadeEmu, out: *mut u8, len: usize) -> c_int;
        pub fn gbade_emu_load_state(emu: *mut GbadeEmu, input: *const u8, len: usize) -> c_int;
        pub fn gbade_emu_savedata(emu: *mut GbadeEmu, out: *mut u8, max_len: usize) -> usize;
    }
}

/// GBA buttons, as a bit mask (same bit layout as the GBA's `REG_KEYINPUT`,
/// but 1 = pressed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Keys(pub u32);

impl Keys {
    pub const NONE: Keys = Keys(0);
    pub const A: Keys = Keys(1 << 0);
    pub const B: Keys = Keys(1 << 1);
    pub const SELECT: Keys = Keys(1 << 2);
    pub const START: Keys = Keys(1 << 3);
    pub const RIGHT: Keys = Keys(1 << 4);
    pub const LEFT: Keys = Keys(1 << 5);
    pub const UP: Keys = Keys(1 << 6);
    pub const DOWN: Keys = Keys(1 << 7);
    pub const R: Keys = Keys(1 << 8);
    pub const L: Keys = Keys(1 << 9);

    /// All buttons with their display names, in GBA bit order.
    pub const ALL: [(Keys, &'static str); 10] = [
        (Keys::A, "A"),
        (Keys::B, "B"),
        (Keys::SELECT, "Select"),
        (Keys::START, "Start"),
        (Keys::RIGHT, "Right"),
        (Keys::LEFT, "Left"),
        (Keys::UP, "Up"),
        (Keys::DOWN, "Down"),
        (Keys::R, "R"),
        (Keys::L, "L"),
    ];

    pub fn contains(self, other: Keys) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Keys {
    type Output = Keys;
    fn bitor(self, rhs: Keys) -> Keys {
        Keys(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Keys {
    fn bitor_assign(&mut self, rhs: Keys) {
        self.0 |= rhs.0;
    }
}

/// Why `run_frame` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The frame completed normally.
    FrameDone,
    /// Execution reached a breakpoint at this address.
    Breakpoint(u32),
    /// The CPU hit an instruction it cannot execute (usually a crash: jumping
    /// to garbage, or a corrupted function pointer).
    IllegalInstruction(u32),
    /// A watchpoint triggered.
    Watchpoint(u32),
    /// The debugger stopped for another reason.
    Other(u32),
}

/// Severity of a log message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    /// The game used a feature mGBA does not emulate.
    Stub,
    /// The game did something invalid (e.g. read unmapped memory).
    GameError,
}

impl LogLevel {
    fn from_mgba(level: c_int) -> LogLevel {
        match level {
            0x01 => LogLevel::Fatal,
            0x02 => LogLevel::Error,
            0x04 => LogLevel::Warn,
            0x08 => LogLevel::Info,
            0x10 => LogLevel::Debug,
            0x20 => LogLevel::Stub,
            _ => LogLevel::GameError,
        }
    }
}

/// A message from the emulator, or printed by the game (`printf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub level: LogLevel,
    /// mGBA log category id, e.g. `"gba.debug"` for game prints,
    /// `"gba.memory"` for bad memory accesses.
    pub category: String,
    pub message: String,
    /// Frame number at which the message was emitted.
    pub frame: u32,
}

impl LogEntry {
    /// True if this was printed by the game itself (printf / debug_log).
    pub fn is_game_output(&self) -> bool {
        self.category == "gba.debug"
    }
}

/// CPU registers snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuState {
    /// r0..r15. `regs[15]` is the address of the instruction being executed.
    pub regs: [u32; 16],
    pub cpsr: u32,
}

impl CpuState {
    pub fn pc(&self) -> u32 {
        self.regs[15]
    }
    pub fn sp(&self) -> u32 {
        self.regs[13]
    }
    pub fn lr(&self) -> u32 {
        self.regs[14]
    }
    /// True when the CPU executes 16-bit Thumb instructions.
    pub fn thumb(&self) -> bool {
        self.cpsr & (1 << 5) != 0
    }
    /// Name of the CPU mode (System, IRQ...).
    pub fn mode_name(&self) -> &'static str {
        match self.cpsr & 0x1F {
            0x10 => "User",
            0x11 => "FIQ",
            0x12 => "IRQ",
            0x13 => "Supervisor",
            0x17 => "Abort",
            0x1B => "Undefined",
            0x1F => "System",
            _ => "?",
        }
    }
}

/// Error returned when a ROM cannot be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LoadError {}

struct LogSink {
    entries: Mutex<Vec<LogEntry>>,
    frame: std::sync::atomic::AtomicU32,
}

extern "C" fn log_trampoline(ctx: *mut c_void, level: c_int, cat: *const c_char, msg: *const c_char) {
    // SAFETY: ctx is the Arc<LogSink> pointer registered in Emulator::new and
    // kept alive for the emulator's whole lifetime.
    let sink = unsafe { &*(ctx as *const LogSink) };
    let category = if cat.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(cat) }.to_string_lossy().into_owned()
    };
    let message = if msg.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(msg) }.to_string_lossy().into_owned()
    };
    let entry = LogEntry {
        level: LogLevel::from_mgba(level),
        category,
        message,
        frame: sink.frame.load(std::sync::atomic::Ordering::Relaxed),
    };
    if let Ok(mut e) = sink.entries.lock() {
        // Cap memory if the game prints in a tight loop and nobody drains.
        if e.len() < 100_000 {
            e.push(entry);
        }
    }
}

/// A GBA emulator instance.
pub struct Emulator {
    raw: *mut ffi::GbadeEmu,
    logs: Arc<LogSink>,
    loaded: bool,
    audio_tmp: Vec<i16>,
}

// The mGBA core is not thread-safe, but it can be moved between threads as
// long as only one thread uses it at a time, which `&mut self` guarantees.
unsafe impl Send for Emulator {}

impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Emulator {
    /// Create an emulator with no game loaded.
    pub fn new() -> Self {
        let raw = unsafe { ffi::gbade_emu_create() };
        assert!(!raw.is_null(), "failed to allocate emulator");
        let logs = Arc::new(LogSink {
            entries: Mutex::new(Vec::new()),
            frame: std::sync::atomic::AtomicU32::new(0),
        });
        unsafe {
            ffi::gbade_emu_set_log_callback(raw, log_trampoline, Arc::as_ptr(&logs) as *mut c_void);
        }
        Emulator { raw, logs, loaded: false, audio_tmp: vec![0; 8192] }
    }

    /// Load a ROM (and optionally existing save data) and reset the console.
    pub fn load_rom(&mut self, rom: &[u8], save: Option<&[u8]>) -> Result<(), LoadError> {
        if rom.len() < 0xC0 {
            return Err(LoadError(format!(
                "ROM is too small ({} bytes): a GBA ROM has at least a 192-byte header",
                rom.len()
            )));
        }
        let (sp, sl) = match save {
            Some(s) if !s.is_empty() => (s.as_ptr(), s.len()),
            _ => (std::ptr::null(), 0),
        };
        let ok = unsafe { ffi::gbade_emu_load_rom(self.raw, rom.as_ptr(), rom.len(), sp, sl) };
        self.loaded = ok != 0;
        if self.loaded {
            Ok(())
        } else {
            Err(LoadError("mGBA could not load this file as a GBA ROM".into()))
        }
    }

    /// True once a ROM was successfully loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Restart the game (like pressing the power button off and on).
    pub fn reset(&mut self) {
        unsafe { ffi::gbade_emu_reset(self.raw) }
    }

    /// Run until the next frame is complete, or until a breakpoint/crash.
    pub fn run_frame(&mut self) -> StopReason {
        if !self.loaded {
            return StopReason::FrameDone;
        }
        self.logs.frame.store(self.frame_counter(), std::sync::atomic::Ordering::Relaxed);
        let r = unsafe { ffi::gbade_emu_run_frame(self.raw) };
        let addr = unsafe { ffi::gbade_emu_stop_address(self.raw) };
        match r {
            0 => StopReason::FrameDone,
            1 => StopReason::Breakpoint(addr),
            2 => StopReason::IllegalInstruction(addr),
            3 => StopReason::Watchpoint(addr),
            _ => StopReason::Other(addr),
        }
    }

    /// Execute exactly one CPU instruction.
    pub fn step_instruction(&mut self) {
        if self.loaded {
            unsafe {
                ffi::gbade_emu_step(self.raw);
            }
        }
    }

    /// Number of frames emulated since the last reset.
    pub fn frame_counter(&self) -> u32 {
        unsafe { ffi::gbade_emu_frame_counter(self.raw) }
    }

    /// The last rendered frame, 240x160 pixels, as mGBA's native 32-bit
    /// colour (little-endian bytes: R, G, B, X).
    pub fn framebuffer(&self) -> &[u32] {
        unsafe {
            std::slice::from_raw_parts(ffi::gbade_emu_framebuffer(self.raw), SCREEN_WIDTH * SCREEN_HEIGHT)
        }
    }

    /// The last rendered frame as RGBA bytes (alpha = 255).
    pub fn framebuffer_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 4);
        for &px in self.framebuffer() {
            let [r, g, b, _] = px.to_le_bytes();
            out.extend_from_slice(&[r, g, b, 255]);
        }
        out
    }

    /// Colour of one pixel as (r, g, b).
    pub fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let [r, g, b, _] = self.framebuffer()[y * SCREEN_WIDTH + x].to_le_bytes();
        (r, g, b)
    }

    /// Set which buttons are held down.
    pub fn set_keys(&mut self, keys: Keys) {
        unsafe { ffi::gbade_emu_set_keys(self.raw, keys.0) }
    }

    /// Change the output sample rate of the audio (default 48000 Hz).
    pub fn set_audio_rate(&mut self, rate: u32) {
        unsafe { ffi::gbade_emu_set_audio_rate(self.raw, rate) }
    }

    /// Append available audio (interleaved stereo i16) to `out`. Call after
    /// every frame, otherwise samples are dropped.
    pub fn drain_audio(&mut self, out: &mut Vec<i16>) {
        loop {
            let frames = self.audio_tmp.len() / 2;
            let got = unsafe { ffi::gbade_emu_read_audio(self.raw, self.audio_tmp.as_mut_ptr(), frames) };
            out.extend_from_slice(&self.audio_tmp[..got * 2]);
            if got < frames {
                break;
            }
        }
    }

    /// Read memory without side effects (safe on I/O registers).
    pub fn read8(&self, addr: u32) -> u8 {
        unsafe { ffi::gbade_emu_read(self.raw, addr, 1) as u8 }
    }
    pub fn read16(&self, addr: u32) -> u16 {
        unsafe { ffi::gbade_emu_read(self.raw, addr, 2) as u16 }
    }
    pub fn read32(&self, addr: u32) -> u32 {
        unsafe { ffi::gbade_emu_read(self.raw, addr, 4) }
    }
    /// Read a block of memory into a new Vec.
    pub fn read_block(&self, addr: u32, len: usize) -> Vec<u8> {
        let mut v = vec![0u8; len];
        unsafe { ffi::gbade_emu_read_block(self.raw, addr, v.as_mut_ptr(), len) };
        v
    }
    pub fn write8(&mut self, addr: u32, v: u8) {
        unsafe { ffi::gbade_emu_write(self.raw, addr, v as u32, 1) }
    }
    pub fn write16(&mut self, addr: u32, v: u16) {
        unsafe { ffi::gbade_emu_write(self.raw, addr, v as u32, 2) }
    }
    pub fn write32(&mut self, addr: u32, v: u32) {
        unsafe { ffi::gbade_emu_write(self.raw, addr, v, 4) }
    }

    /// Snapshot of the CPU registers.
    pub fn cpu_state(&self) -> CpuState {
        let mut s = CpuState::default();
        if self.loaded {
            unsafe { ffi::gbade_emu_cpu_state(self.raw, s.regs.as_mut_ptr(), &mut s.cpsr) };
        }
        s
    }

    /// Disassemble one instruction. Returns the text and the instruction size.
    pub fn disassemble(&self, addr: u32, thumb: bool) -> (String, u32) {
        let mut buf = [0 as c_char; 128];
        let size =
            unsafe { ffi::gbade_emu_disassemble(self.raw, addr, thumb as c_int, buf.as_mut_ptr(), buf.len()) };
        let text = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned();
        (text, size.max(2) as u32)
    }

    /// Stop execution when the CPU is about to execute `addr`.
    pub fn add_breakpoint(&mut self, addr: u32) -> bool {
        self.loaded && unsafe { ffi::gbade_emu_add_breakpoint(self.raw, addr) } >= 0
    }

    pub fn clear_breakpoints(&mut self) {
        if self.loaded {
            unsafe { ffi::gbade_emu_clear_breakpoints(self.raw) }
        }
    }

    /// Serialize the whole console state (a "save state").
    pub fn save_state(&mut self) -> Option<Vec<u8>> {
        if !self.loaded {
            return None;
        }
        let size = unsafe { ffi::gbade_emu_state_size(self.raw) };
        let mut buf = vec![0u8; size];
        let ok = unsafe { ffi::gbade_emu_save_state(self.raw, buf.as_mut_ptr(), size) };
        (ok != 0).then_some(buf)
    }

    /// Restore a state produced by [`Emulator::save_state`].
    pub fn load_state(&mut self, state: &[u8]) -> bool {
        self.loaded && unsafe { ffi::gbade_emu_load_state(self.raw, state.as_ptr(), state.len()) } != 0
    }

    /// The cartridge save memory (SRAM/Flash/EEPROM), if the game uses any.
    pub fn save_data(&self) -> Vec<u8> {
        if !self.loaded {
            return Vec::new();
        }
        let size = unsafe { ffi::gbade_emu_savedata(self.raw, std::ptr::null_mut(), 0) };
        let mut v = vec![0u8; size];
        if size > 0 {
            unsafe { ffi::gbade_emu_savedata(self.raw, v.as_mut_ptr(), size) };
        }
        v
    }

    /// Take all log messages emitted since the last call.
    pub fn take_logs(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut *self.logs.entries.lock().unwrap())
    }
}

impl Drop for Emulator {
    fn drop(&mut self) {
        unsafe { ffi::gbade_emu_destroy(self.raw) }
    }
}
