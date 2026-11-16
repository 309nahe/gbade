//! Every tutorial's example builds without a warning and plays: the lessons
//! promise "click Try this example", so the examples must always work.

mod common;
use common::*;
use gbade_emu::{Keys, StopReason};

/// (tutorial folder, buttons to press, text the example must print).
const EXPECTED: &[(&str, &[Keys], &str)] = &[
    ("01-first-pixels", &[], "Drew the first pixels."),
    ("02-the-game-loop", &[], "one more second: 60 frames"),
    ("03-buttons", &[Keys::A, Keys::A], "A pressed: colour 2"),
    ("04-smooth-movement", &[Keys::A], "Throw from y ="),
    ("05-collisions", &[], "New coin at ("),
    ("06-tiles-and-backgrounds", &[], "Level built from 30 x 20 tiles."),
    ("07-sprites-and-animation", &[Keys::A], "Hero at (112, 72)"),
    ("08-scrolling-and-camera", &[Keys::A], "player (40, 160), camera (0, 88)"),
    ("09-game-states-and-text", &[Keys::START], "State: PLAY"),
    ("10-sound-effects", &[Keys::A, Keys::R], "melody"),
];

#[test]
fn every_tutorial_is_listed_here() {
    let mut on_disk: Vec<String> = std::fs::read_dir(repo_root().join("tutorials"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().join("gbade.toml").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let listed: Vec<&str> = EXPECTED.iter().map(|(d, _, _)| *d).collect();
    assert_eq!(on_disk, listed, "add new tutorials to EXPECTED");
}

#[test]
fn every_tutorial_example_builds_cleanly_and_plays() {
    for (folder, presses, expected) in EXPECTED {
        let (_dir, p) = copy_project(&repo_root().join("tutorials").join(folder));
        let out = build_ok(&p);
        assert!(out.warnings().next().is_none(), "{folder}: warnings {:#?}", out.diagnostics);
        let mut run = Run::new(&out);
        run.frames(70);
        for k in presses.iter() {
            run.press(*k, 3);
        }
        // A little longer with the D-pad held: movement code runs too.
        run.emu.set_keys(Keys::RIGHT | Keys::DOWN);
        for _ in 0..60 {
            assert_eq!(run.emu.run_frame(), StopReason::FrameDone, "{folder} stopped");
        }
        run.logs.extend(run.emu.take_logs());
        let out_text = run.game_output();
        assert!(!out_text.iter().any(|l| l.contains("GBADE:PANIC")), "{folder} panicked: {out_text:?}");
        assert!(run.printed(expected), "{folder} did not print {expected:?}: {out_text:?}");
    }
}

#[test]
fn the_collisions_example_collects_coins() {
    // The coins come from a seeded random generator, so the first one is
    // always in the same place. Walk to it: up from (20, 70) along the left
    // of the first wall, then right. The player moves 2 pixels per frame.
    let (_dir, p) = copy_project(&repo_root().join("tutorials/05-collisions"));
    let mut run = Run::new(&build_ok(&p));
    run.frames(2);
    assert!(run.printed("New coin at (54, 20)"), "the first coin moved: {:?}", run.game_output());
    for (keys, frames) in [(Keys::UP, 25), (Keys::RIGHT, 15)] {
        run.emu.set_keys(keys);
        run.frames(frames);
    }
    assert!(run.printed("Coin! Score: 1"), "{:?}", run.game_output());
    // A new coin appears somewhere else.
    assert_eq!(run.game_output().iter().filter(|l| l.starts_with("New coin")).count(), 2);
}

#[test]
fn the_game_states_example_goes_round_the_loop() {
    let (_dir, p) = copy_project(&repo_root().join("tutorials/09-game-states-and-text"));
    let mut run = Run::new(&build_ok(&p));
    run.frames(5);
    run.press(Keys::START, 2);
    for _ in 0..5 {
        run.press(Keys::A, 2);
    }
    run.frames(10 * 60); // the 10-second round ends
    run.press(Keys::START, 2);
    let out = run.game_output();
    let states: Vec<&String> = out.iter().filter(|l| l.starts_with("State:")).collect();
    assert_eq!(states.len(), 4, "{out:?}");
    assert!(states[2].starts_with("State: GAME_OVER, score 5"), "{out:?}");
    assert!(states[3].starts_with("State: TITLE"), "{out:?}");
}
