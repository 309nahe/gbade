//! Helpers shared by the end-to-end tests.
#![allow(dead_code)]

use gbade_emu::{Emulator, Keys, LogEntry};
use gbade_toolchain::{BuildOptions, BuildOutput, Project, Toolchain, build};
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// A cache dir shared by all tests so the runtime is compiled once.
pub fn test_options() -> BuildOptions {
    BuildOptions { cache_dir: Some(repo_root().join("target").join("gbade-test-cache")), jobs: 0, force: false }
}

/// Copy an example (or any project dir) into a fresh temp dir.
pub fn copy_project(src: &Path) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().unwrap();
    gbade_toolchain::project::copy_dir(src, dir.path()).unwrap();
    let p = Project::open(dir.path()).unwrap();
    (dir, p)
}

/// Create a project from a main.c string.
pub fn project_with_main(main_c: &str) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("gbade.toml"), "[game]\nname = \"Test\"\n").unwrap();
    std::fs::write(dir.path().join("src/main.c"), main_c).unwrap();
    let p = Project::open(dir.path()).unwrap();
    (dir, p)
}

pub fn build_ok(p: &Project) -> BuildOutput {
    let tc = Toolchain::locate().expect("toolchain");
    let out = build(p, &tc, &test_options());
    if !out.success {
        panic!("build failed:\n{}\n{:#?}", out.log, out.diagnostics);
    }
    out
}

pub struct Run {
    pub emu: Emulator,
    pub logs: Vec<LogEntry>,
}

impl Run {
    pub fn new(out: &BuildOutput) -> Run {
        let rom = std::fs::read(out.rom_path.as_ref().unwrap()).unwrap();
        let mut emu = Emulator::new();
        emu.load_rom(&rom, None).unwrap();
        Run { emu, logs: Vec::new() }
    }
    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let r = self.emu.run_frame();
            assert_eq!(r, gbade_emu::StopReason::FrameDone);
            let mut a = Vec::new();
            self.emu.drain_audio(&mut a);
        }
        self.logs.extend(self.emu.take_logs());
    }
    pub fn press(&mut self, keys: Keys, frames: usize) {
        self.emu.set_keys(keys);
        self.frames(frames);
        self.emu.set_keys(Keys::NONE);
        self.frames(2);
    }
    pub fn game_output(&self) -> Vec<String> {
        self.logs.iter().filter(|l| l.is_game_output()).map(|l| l.message.clone()).collect()
    }
    pub fn printed(&self, text: &str) -> bool {
        self.game_output().iter().any(|m| m.contains(text))
    }
}
