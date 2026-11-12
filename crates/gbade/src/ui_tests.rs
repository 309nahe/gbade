//! UI tests: drive the real IDE headlessly with egui_kittest, click buttons
//! like a user would, and save screenshots to `target/ui-snapshots/` so the
//! UI can be reviewed without opening the app.

use crate::app::{BottomTab, GbadeApp};
use crate::settings::Settings;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn snapshot_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn make_app(projects: &Path) -> GbadeApp {
    let ctx = eframe::egui::Context::default();
    let settings = Settings { projects_dir: Some(projects.to_path_buf()), ..Settings::default() };
    GbadeApp::with_settings(&ctx, settings)
}

fn harness(app: GbadeApp) -> Harness<'static, GbadeApp> {
    Harness::builder()
        .with_size([1400.0, 880.0])
        .build_state(
            |ctx, app: &mut GbadeApp| {
                app.apply_theme(ctx);
                app.frame(ctx)
            },
            app,
        )
}

fn save(h: &mut Harness<'_, GbadeApp>, name: &str) {
    match h.render() {
        Ok(img) => {
            img.save(snapshot_dir().join(format!("{name}.png"))).unwrap();
        }
        Err(e) => eprintln!("could not render {name}: {e}"),
    }
}

/// Step the UI until `cond` is true (or panic after `secs`).
fn wait_until(h: &mut Harness<'_, GbadeApp>, secs: u64, what: &str, cond: impl Fn(&GbadeApp) -> bool) {
    let start = Instant::now();
    while !cond(h.state()) {
        if start.elapsed() > Duration::from_secs(secs) {
            panic!("timed out waiting for: {what}\nbuild log:\n{}", h.state().build_log);
        }
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Use a separate runtime cache so tests never touch the user's cache.
fn isolate_cache() {
    // SAFETY: tests set the same value; nothing reads it concurrently in a
    // way that would observe a torn write.
    unsafe {
        std::env::set_var(
            "GBADE_CACHE_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gbade-test-cache"),
        );
    }
}

#[test]
fn welcome_screen_lists_examples() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness(make_app(tmp.path()));
    h.run_steps(3);
    h.get_by_label("✨ New Project");
    h.get_by_label("📂 Open Project");
    assert!(h.query_by_label("Hello World").is_some(), "example list should show Hello World");
    save(&mut h, "01-welcome");
}

#[test]
fn create_build_run_and_debug_a_project() {
    isolate_cache();
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness(make_app(tmp.path()));
    h.run_steps(2);

    // 1. Create a project from the Hello World example through the dialog.
    h.get_all_by_label("Create from this").next().unwrap().click();
    h.run_steps(2);
    save(&mut h, "02-new-project-dialog");
    h.get_by_label("Create").click();
    h.run_steps(3);
    let root = h.state().project.as_ref().expect("project created").root.clone();
    assert!(root.join("src/main.c").exists());
    assert_eq!(h.state().docs.len(), 1, "main.c should be open");

    // 2. Run it.
    h.get_by_label("▶  Run").click();
    wait_until(&mut h, 120, "game running", |a| a.session.is_some());
    wait_until(&mut h, 20, "hello printed", |a| {
        a.session.as_ref().is_some_and(|s| s.console.iter().any(|l| l.text.contains("Hello from the GBA!")))
    });
    // Let the game draw a few complete frames before taking the picture.
    wait_until(&mut h, 20, "a few frames", |a| a.session.as_ref().is_some_and(|s| s.emu.frame_counter() >= 20));
    h.run_steps(2);
    assert_eq!(h.state().bottom_tab, BottomTab::Console);
    save(&mut h, "03-running");

    // 3. Introduce a compile error: the Problems panel must show it with a hint.
    {
        let app = h.state_mut();
        let doc = &mut app.docs[0];
        doc.text = doc.text.replacen("int presses = 0;", "int presses = 0", 1);
    }
    h.get_by_label("▶  Run").click();
    h.step();
    assert!(h.state().build_job.is_some(), "clicking Run should start a build");
    wait_until(&mut h, 60, "build finished", |a| a.build_job.is_none());
    h.run_steps(3);
    let app = h.state();
    assert!(!app.last_build.as_ref().unwrap().success);
    assert_eq!(app.bottom_tab, BottomTab::Problems);
    assert!(app.diagnostics.iter().any(|d| d.message.contains("expected ';'")), "{:#?}", app.diagnostics);
    // The file was saved before building (that's how the error was found).
    assert!(std::fs::read_to_string(root.join("src/main.c")).unwrap().contains("int presses = 0\n"));
    save(&mut h, "04-compile-error");

    // 4. Fix it, set a breakpoint inside the A-button branch, press A.
    {
        let app = h.state_mut();
        let doc = &mut app.docs[0];
        doc.text = doc.text.replacen("int presses = 0\n", "int presses = 0;\n", 1);
    }
    let main_c = root.join("src/main.c").canonicalize().unwrap();
    let text = h.state().docs[0].text.clone();
    let line = text.lines().position(|l| l.contains("presses++;")).unwrap() as u32 + 1;
    h.state_mut().toggle_breakpoint(&main_c, line);
    h.get_by_label("▶  Run").click();
    h.step();
    assert!(h.state().build_job.is_some());
    wait_until(&mut h, 60, "game restarted", |a| a.build_job.is_none() && a.session.is_some());
    h.run_steps(10);
    // Hold "X" (= GBA A button) until the breakpoint stops the game.
    let start = Instant::now();
    while !h.state().session.as_ref().is_some_and(|s| s.is_paused()) {
        assert!(start.elapsed() < Duration::from_secs(20), "breakpoint never hit");
        h.key_down(eframe::egui::Key::X);
        h.step();
    }
    h.key_up(eframe::egui::Key::X);
    h.run_steps(3);
    let pause = h.state().session.as_ref().unwrap().pause.clone().unwrap();
    assert_eq!(pause.reason, crate::session::PauseReason::Breakpoint);
    assert_eq!(pause.location.as_ref().map(|(_, l)| *l), Some(line));
    assert!(h.query_by_label("Stopped at a breakpoint").is_some());
    save(&mut h, "05-breakpoint");

    // 5. Step one line, then continue.
    h.get_by_label("⤵ Step line").click();
    h.run_steps(2);
    let after = h.state().session.as_ref().unwrap().pause.clone().unwrap();
    assert_eq!(after.reason, crate::session::PauseReason::Step);
    assert_ne!(after.location.as_ref().map(|(_, l)| *l), Some(line));
    h.state_mut().breakpoints.clear();
    h.state_mut().toggle_pause();
    h.run_steps(3);
    assert!(!h.state().session.as_ref().unwrap().is_paused());

    // 6. The inspector tabs render without problems.
    for tab in ["I/O", "Palettes", "Tiles", "Sprites", "Memory", "CPU"] {
        h.get_by_label(tab).click();
        h.run_steps(2);
        save(&mut h, &format!("06-inspector-{}", tab.replace('/', "")));
    }

    // 7. Learn panel.
    h.get_by_label("📖 Learn").click();
    h.run_steps(3);
    save(&mut h, "07-learn");
}
