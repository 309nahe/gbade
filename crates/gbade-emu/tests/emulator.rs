//! The emulator API, tested on a tiny hand-assembled ROM so that these tests
//! need no compiler and know exactly what the CPU executes. The toolchain's
//! end-to-end tests run real compiled games on top of this.
//!
//! The ROM, in ARM code: it counts loop iterations in r0 and reads the
//! buttons into r1, as a game would.
//!
//! ```text
//! 0x08000000  b    0x080000C0          entry point, jumps over the header
//! 0x080000C0  mov  r0, #0
//! 0x080000C4  mov  r2, #0x04000000
//! 0x080000C8  add  r2, r2, #0x130      r2 = KEYINPUT
//! 0x080000CC  add  r0, r0, #1          <- LOOP
//! 0x080000D0  ldrh r1, [r2]
//! 0x080000D4  b    0x080000CC
//! ```
//!
//! mGBA starts at address 0 in its built-in replacement BIOS, which then
//! jumps to the cartridge entry point.

use gbade_emu::{Emulator, Keys, SCREEN_HEIGHT, SCREEN_WIDTH, StopReason};

const ENTRY: u32 = 0x0800_0000;
const START: u32 = 0x0800_00C0;
const LOOP: u32 = 0x0800_00CC;
const EWRAM: u32 = 0x0200_0000;
const IWRAM: u32 = 0x0300_0000;

fn tiny_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x400];
    let mut put = |at: usize, insn: u32| rom[at..at + 4].copy_from_slice(&insn.to_le_bytes());
    put(0x00, 0xEA00_002E); // b 0x080000C0  ((0xC0 - 8) / 4 = 0x2E)
    put(0xC0, 0xE3A0_0000); // mov r0, #0
    put(0xC4, 0xE3A0_2301); // mov r2, #0x04000000  (1 ror 6)
    put(0xC8, 0xE282_2F4C); // add r2, r2, #0x130   (0x4C ror 30)
    put(0xCC, 0xE280_0001); // add r0, r0, #1
    put(0xD0, 0xE1D2_10B0); // ldrh r1, [r2]
    put(0xD4, 0xEAFF_FFFC); // b 0x080000CC  ((0xCC - 0xDC) / 4 = -4)
    rom[0xB2] = 0x96; // the fixed header byte mGBA uses to recognise a ROM
    rom
}

fn loaded() -> Emulator {
    let mut emu = Emulator::new();
    emu.load_rom(&tiny_rom(), None).expect("the tiny ROM loads");
    emu
}

/// Step until the CPU is about to execute `addr` (at most `max` instructions).
fn step_to(emu: &mut Emulator, addr: u32, max: usize) {
    for _ in 0..max {
        if emu.cpu_state().pc() == addr {
            return;
        }
        emu.step_instruction();
    }
    panic!("never reached {addr:#010x}, PC is {:#010x}", emu.cpu_state().pc());
}

#[test]
fn an_empty_emulator_does_nothing_and_does_not_crash() {
    let mut emu = Emulator::new();
    assert!(!emu.is_loaded());
    assert_eq!(emu.run_frame(), StopReason::FrameDone);
    emu.step_instruction();
    assert_eq!(emu.cpu_state(), Default::default());
    assert_eq!(emu.save_state(), None);
    assert!(!emu.load_state(&[1, 2, 3]));
    assert!(!emu.add_breakpoint(LOOP));
    emu.clear_breakpoints();
    assert!(emu.save_data().is_empty());
    assert!(emu.take_logs().is_empty());
}

#[test]
fn files_that_are_not_roms_are_refused() {
    let mut emu = Emulator::new();
    let err = emu.load_rom(&[0u8; 100], None).unwrap_err();
    assert!(err.to_string().contains("too small"), "{err}");
    // Big enough, but without the ROM signature.
    assert!(emu.load_rom(&vec![0u8; 0x400], None).is_err());
    assert!(!emu.is_loaded());
    // A good ROM afterwards still loads.
    emu.load_rom(&tiny_rom(), None).unwrap();
    assert!(emu.is_loaded());
}

#[test]
fn the_cpu_runs_the_rom_from_its_entry_point() {
    let mut emu = loaded();
    // The replacement BIOS runs first, from address 0.
    assert_eq!(emu.cpu_state().pc(), 0);
    step_to(&mut emu, ENTRY, 10_000);
    let s = emu.cpu_state();
    assert!(!s.thumb(), "the cartridge entry point is ARM code");
    assert_eq!(s.mode_name(), "System");
    step_to(&mut emu, START, 2);
    step_to(&mut emu, LOOP, 4);
    // r0 is 0 entering the loop, and each add makes it one more.
    assert_eq!(emu.cpu_state().regs[0], 0);
    assert_eq!(emu.cpu_state().regs[2], 0x0400_0130);
    emu.step_instruction(); // add
    assert_eq!(emu.cpu_state().regs[0], 1);
    emu.step_instruction(); // ldrh
    emu.step_instruction(); // b
    assert_eq!(emu.cpu_state().pc(), LOOP);
    emu.step_instruction();
    assert_eq!(emu.cpu_state().regs[0], 2);
}

#[test]
fn frames_advance_the_frame_counter() {
    let mut emu = loaded();
    assert_eq!(emu.frame_counter(), 0);
    for _ in 0..10 {
        assert_eq!(emu.run_frame(), StopReason::FrameDone);
    }
    assert_eq!(emu.frame_counter(), 10);
    // The loop ran a lot: r0 counts its iterations.
    assert!(emu.cpu_state().regs[0] > 10_000, "r0 = {}", emu.cpu_state().regs[0]);
    emu.reset();
    assert_eq!(emu.frame_counter(), 0);
    assert_eq!(emu.cpu_state().pc(), 0, "back to the start of the BIOS");
}

#[test]
fn breakpoints_stop_the_frame_and_can_be_cleared() {
    let mut emu = loaded();
    assert!(emu.add_breakpoint(LOOP));
    assert_eq!(emu.run_frame(), StopReason::Breakpoint(LOOP));
    assert_eq!(emu.cpu_state().pc(), LOOP);
    let r0 = emu.cpu_state().regs[0];
    // Running again stops at the next iteration.
    emu.step_instruction();
    assert_eq!(emu.run_frame(), StopReason::Breakpoint(LOOP));
    assert_eq!(emu.cpu_state().regs[0], r0 + 1);
    emu.clear_breakpoints();
    assert_eq!(emu.run_frame(), StopReason::FrameDone);
}

#[test]
fn memory_reads_and_writes_in_every_width() {
    let mut emu = loaded();
    emu.write32(EWRAM, 0x1122_3344);
    assert_eq!(emu.read32(EWRAM), 0x1122_3344);
    assert_eq!(emu.read16(EWRAM), 0x3344, "little-endian");
    assert_eq!(emu.read8(EWRAM + 3), 0x11);
    emu.write16(IWRAM + 2, 0xBEEF);
    emu.write8(IWRAM, 0x7F);
    assert_eq!(emu.read_block(IWRAM, 4), vec![0x7F, 0x00, 0xEF, 0xBE]);
    // The ROM is readable and holds what was loaded.
    assert_eq!(emu.read32(LOOP), 0xE280_0001);
    assert_eq!(emu.read_block(ENTRY + 0xB2, 1), vec![0x96]);
}

#[test]
fn the_game_sees_the_held_buttons() {
    // r1 holds what the game last read from KEYINPUT, which is active-low:
    // a held button reads as 0. (mGBA updates the register when the game
    // reads it, so this checks what a game really sees.)
    let mut emu = loaded();
    let keys_seen = |emu: &mut Emulator| {
        emu.run_frame();
        emu.cpu_state().regs[1] & 0x3FF
    };
    assert_eq!(keys_seen(&mut emu), 0x3FF);
    emu.set_keys(Keys::A | Keys::START);
    assert_eq!(keys_seen(&mut emu), 0x3FF & !(Keys::A.0 | Keys::START.0));
    for (key, name) in Keys::ALL {
        emu.set_keys(key);
        assert_eq!(keys_seen(&mut emu), 0x3FF & !key.0, "{name}");
    }
    emu.set_keys(Keys::NONE);
    assert_eq!(keys_seen(&mut emu), 0x3FF);
}

#[test]
fn save_states_restore_registers_and_memory() {
    let mut emu = loaded();
    for _ in 0..3 {
        emu.run_frame();
    }
    emu.write32(EWRAM, 0xCAFE_F00D);
    let before = emu.cpu_state();
    let frame = emu.frame_counter();
    let state = emu.save_state().expect("a loaded game can be saved");
    assert!(!state.is_empty());

    for _ in 0..5 {
        emu.run_frame();
    }
    emu.write32(EWRAM, 0);
    assert_ne!(emu.cpu_state(), before);

    assert!(emu.load_state(&state));
    assert_eq!(emu.cpu_state(), before);
    assert_eq!(emu.read32(EWRAM), 0xCAFE_F00D);
    assert_eq!(emu.frame_counter(), frame);
    // Garbage is refused and changes nothing.
    assert!(!emu.load_state(&[0u8; 16]));
    assert_eq!(emu.cpu_state(), before);
}

#[test]
fn disassembly_reads_the_rom() {
    let emu = loaded();
    let (text, size) = emu.disassemble(LOOP, false);
    assert_eq!(size, 4, "ARM instructions are 4 bytes");
    assert!(text.to_lowercase().contains("add"), "{text}");
    let (text, _) = emu.disassemble(START, false);
    assert!(text.to_lowercase().contains("mov"), "{text}");
}

#[test]
fn the_screen_and_the_sound_produce_data_every_frame() {
    let mut emu = loaded();
    emu.run_frame();
    assert_eq!(emu.framebuffer().len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    let rgba = emu.framebuffer_rgba();
    assert_eq!(rgba.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    assert!(rgba.chunks(4).all(|p| p[3] == 255), "opaque");
    let (r, g, b) = emu.pixel(SCREEN_WIDTH - 1, SCREEN_HEIGHT - 1);
    assert_eq!(&rgba[rgba.len() - 4..rgba.len() - 1], &[r, g, b]);

    // About one frame of stereo audio at 48 kHz: 48000 / 59.73 = 804 samples
    // per channel. The exact count depends on buffering; check the order of
    // magnitude, over several frames.
    let mut audio = Vec::new();
    for _ in 0..30 {
        emu.run_frame();
        emu.drain_audio(&mut audio);
    }
    let per_frame = audio.len() as f64 / 2.0 / 30.0;
    assert!((700.0..900.0).contains(&per_frame), "{per_frame} samples per frame at 48 kHz");

    emu.set_audio_rate(24_000);
    let mut half = Vec::new();
    for _ in 0..30 {
        emu.run_frame();
        emu.drain_audio(&mut half);
    }
    let per_frame = half.len() as f64 / 2.0 / 30.0;
    assert!((350.0..450.0).contains(&per_frame), "{per_frame} samples per frame at 24 kHz");
}

#[test]
fn a_game_without_save_memory_has_no_save_data() {
    let mut emu = loaded();
    emu.run_frame();
    assert!(emu.save_data().is_empty());
}

#[test]
fn several_emulators_are_independent() {
    let mut a = loaded();
    let mut b = loaded();
    a.write32(EWRAM, 1);
    b.write32(EWRAM, 2);
    for _ in 0..3 {
        a.run_frame();
    }
    assert_eq!((a.read32(EWRAM), b.read32(EWRAM)), (1, 2));
    assert_eq!((a.frame_counter(), b.frame_counter()), (3, 0));
}
