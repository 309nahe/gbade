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
    let main_c = gbade_toolchain::paths::canonical(&root.join("src/main.c"));
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

#[test]
fn code_assistance_and_git() {
    isolate_cache();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(tmp.path());
    app.settings.git_name = "Tester".into();
    app.settings.git_email = "tester@example.com".into();
    app.settings.track_with_git = true;
    let mut h = harness(app);
    h.run_steps(2);
    h.get_all_by_label("Create from this").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("Create").click();
    h.run_steps(3);

    // A new project starts tracked, with one snapshot.
    assert!(h.state().git.is_some(), "new projects are git repositories");
    assert_eq!(h.state().git_ui.history.len(), 1);
    if h.state().lsp.is_none() {
        eprintln!("clangd missing: run `cargo xtask fetch-llvm`; skipping the code assistance part");
        return;
    }
    let main = h.state().docs[0].path.clone();

    // 1. Live error while typing: no build needed.
    {
        let d = &mut h.state_mut().docs[0];
        d.text = d.text.replacen("int presses = 0;", "int presses = 0", 1);
    }
    wait_until(&mut h, 30, "live error from clangd", |a| {
        a.live_diagnostics.get(&a.docs[0].path).is_some_and(|d| d.iter().any(|d| d.message.contains("expected ';'")))
    });
    assert!(h.state().last_build.is_none(), "found without building");
    assert!(h.state().messages_for(&main).iter().any(|m| m.text.contains("💡")), "with the beginner hint");
    h.run_steps(2);
    save(&mut h, "08-live-error");

    // 2. Fix it: the error disappears.
    {
        let d = &mut h.state_mut().docs[0];
        d.text = d.text.replacen("int presses = 0\n", "int presses = 0;\n", 1);
    }
    wait_until(&mut h, 30, "error cleared", |a| {
        a.live_diagnostics.get(&a.docs[0].path).is_some_and(|d| d.iter().all(|d| d.severity != gbade_toolchain::Severity::Error))
    });

    // 3. Completion: type "key_h" on a new line and ask (as typing does).
    let ci = {
        let d = &mut h.state_mut().docs[0];
        let line_start = d.text.find("        key_poll();").unwrap();
        let at = line_start + d.text[line_start..].find('\n').unwrap() + 1;
        d.text.insert_str(at, "        key_h\n");
        d.text[..at + "        key_h".len()].chars().count()
    };
    h.state_mut().lsp_request(&crate::editor::EditorEvent::Complete(ci));
    wait_until(&mut h, 30, "completion items", |a| a.docs[0].completion.is_some());
    let names: Vec<String> = h.state().docs[0].completion.as_ref().unwrap().items.iter().map(|i| i.insert.clone()).collect();
    assert!(names.iter().any(|n| n == "key_hit") && names.iter().any(|n| n == "key_held"), "{names:?}");
    // Give the editor the cursor at the end of "key_h" so the popup is drawn there.
    let id = h.state().docs[0].id;
    h.run_steps(1);
    {
        let ctx = h.ctx.clone();
        let mut st = eframe::egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
        st.cursor.set_char_range(Some(eframe::egui::text::CCursorRange::one(eframe::egui::text::CCursor::new(ci))));
        st.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }
    h.run_steps(3);
    assert!(h.query_by_label_contains("matches").is_some(), "the completion popup is shown");
    save(&mut h, "09-completion");
    {
        let d = &mut h.state_mut().docs[0];
        d.completion = None;
        d.text = d.text.replacen("        key_h\n", "", 1);
    }

    // 4. Hover documentation and go to definition on `vsync`.
    let vs = {
        let t = &h.state().docs[0].text;
        t[..t.find("vsync();").unwrap()].chars().count() + 1
    };
    let word = crate::assist::word_start(&h.state().docs[0].text.chars().collect::<Vec<_>>(), vs + 2);
    h.state_mut().lsp_request(&crate::editor::EditorEvent::Hover(word));
    wait_until(&mut h, 30, "hover text", |a| a.docs[0].hover.text_for(word).is_some());
    assert!(h.state().docs[0].hover.text_for(word).unwrap().to_lowercase().contains("vblank"));
    h.state_mut().lsp_request(&crate::editor::EditorEvent::GotoDefinition(vs));
    wait_until(&mut h, 30, "definition opened", |a| a.docs.len() == 2);
    let header = &h.state().docs[h.state().active_doc];
    assert!(header.path.ends_with("gba/video.h"), "{:?}", header.path);
    assert!(header.read_only, "SDK headers open read-only");
    h.run_steps(2);
    save(&mut h, "10-go-to-definition");
    h.state_mut().active_doc = 0;

    // 5. Git: the edit shows in the Changes tab; commit it.
    {
        let d = &mut h.state_mut().docs[0];
        d.text = d.text.replacen("Hello, Game Boy Advance!", "Hello from git!", 1);
    }
    assert!(h.state_mut().save_all());
    h.run_steps(2);
    h.get_by_label("Changes (1)").click();
    h.run_steps(3);
    assert_eq!(h.state().git_ui.changes.len(), 1);
    // The margin shows the changed line.
    assert!(!h.state().docs[0].git_lines.is_empty(), "changed lines are marked in the margin");
    save(&mut h, "11-git-changes");
    h.state_mut().git_ui.message = "Change the greeting".into();
    h.run_steps(1);
    h.get_by_label("Commit all (1)").click();
    h.run_steps(3);
    assert!(h.state().git_ui.changes.is_empty());
    assert_eq!(h.state().git_ui.history.len(), 2);
    assert_eq!(h.state().git_ui.history[0].summary, "Change the greeting");

    // 6. The commit's diff.
    h.get_by_label("Change the greeting").click();
    h.run_steps(3);
    assert_eq!(h.state().center, crate::app::CenterView::Diff);
    save(&mut h, "12-commit-diff");
    let _ = (main, Instant::now());
}

#[test]
fn tutorials_tab_runs_an_example_in_one_click() {
    isolate_cache();
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness(make_app(tmp.path()));
    h.run_steps(2);

    // The welcome screen and the toolbar both lead to the tutorials.
    h.get_all_by_label("🎓 Tutorials").next().expect("a Tutorials button").click();
    h.run_steps(3);
    assert_eq!(h.state().center, crate::app::CenterView::Tutorials);
    // Listed on the left, and shown first (its title in the header).
    assert_eq!(h.query_all_by_label("1. First Pixels").count(), 2);
    save(&mut h, "08-tutorials");

    // Pick a tutorial from the list, then try its example.
    h.get_by_label("3. Buttons").click();
    h.run_steps(3);
    assert!(h.state().tutorials_viewer.current.as_ref().unwrap().ends_with("03-buttons"));
    save(&mut h, "09-tutorial-buttons");
    h.get_by_role_and_label(eframe::egui::accesskit::Role::Button, "▶ Try this example").click();
    h.run_steps(2);

    let project = h.state().project.as_ref().expect("a project was created").root.clone();
    assert!(project.starts_with(gbade_toolchain::paths::canonical(tmp.path())), "{}", project.display());
    assert!(project.join("README.md").exists(), "the lesson comes with the project");
    assert_eq!(h.state().center, crate::app::CenterView::Editor);
    wait_until(&mut h, 120, "the example runs", |a| a.session.is_some());

    // Trying again makes a second copy instead of overwriting the first.
    h.state_mut().center = crate::app::CenterView::Tutorials;
    h.run_steps(2);
    h.get_by_role_and_label(eframe::egui::accesskit::Role::Button, "▶ Try this example").click();
    h.run_steps(2);
    let second = h.state().project.as_ref().unwrap().root.clone();
    assert_ne!(second, project);
    assert!(second.file_name().unwrap().to_string_lossy().ends_with("-2"), "{}", second.display());
    wait_until(&mut h, 120, "the second copy runs", |a| a.session.is_some() && a.build_job.is_none());
    h.run_steps(5);
    save(&mut h, "10-tutorial-example-running");
}

/// The same running game in every theme: `theme-<slug>.png`, used by the
/// README's theme gallery.
#[test]
fn every_theme_draws_the_ide() {
    use crate::themes::ThemeId;
    isolate_cache();
    let tmp = tempfile::tempdir().unwrap();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let dest = tmp.path().join("pong");
    gbade_toolchain::project::Project::create_from_template(&examples.join("pong"), &dest, "Pong").unwrap();
    let mut h = harness(make_app(tmp.path()));
    h.state_mut().open_project(&dest);
    h.run_steps(2);
    h.get_by_label("▶  Run").click();
    wait_until(&mut h, 120, "game running", |a| a.session.is_some() && a.build_job.is_none());
    wait_until(&mut h, 20, "a few frames", |a| a.session.as_ref().is_some_and(|s| s.emu.frame_counter() >= 60));
    for id in ThemeId::ALL {
        h.state_mut().settings.set_theme(id);
        h.run_steps(3);
        assert_eq!(h.ctx.style().visuals, crate::themes::visuals(id), "{}", id.name());
        assert_eq!(crate::themes::current(&h.ctx), id);
        save(&mut h, &format!("theme-{}", id.slug()));
    }
    // Themes picked before this version are remembered as light or dark.
    let old = Settings { dark_mode: false, theme: None, ..Settings::default() };
    assert_eq!(old.theme(), ThemeId::Light);
    assert_eq!(Settings::default().theme(), ThemeId::Dark);
}

#[test]
fn line_numbers_stay_visible_when_scrolling_sideways() {
    use eframe::egui::{self, Event, Modifiers, MouseWheelUnit, Pos2, Vec2};
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(tmp.path());
    // A project with one very long line, so there is something to scroll to.
    let dir = tmp.path().join("wide");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("gbade.toml"), "[game]\nname = \"Wide\"\n").unwrap();
    let long = format!("// {}\n", "a very long comment, to scroll sideways through. ".repeat(60));
    let mut text = String::from("#include <gba.h>\n");
    for i in 0..40 {
        text.push_str(&format!("int variable_number_{i} = {i};\n"));
    }
    text.push_str(&long);
    text.push_str("int main(void) { while (1) { vsync(); } }\n");
    std::fs::write(dir.join("src/main.c"), text).unwrap();
    app.open_project(&dir);
    let mut h = harness(app);
    h.run_steps(4);

    let (g0, left0, moved0) = h.state().docs[0].gutter_drawn.expect("the editor was drawn");
    assert_eq!(moved0, 0.0, "nothing is scrolled yet");
    assert_eq!(g0.left(), left0, "at the start the gutter is at the left edge");
    save(&mut h, "13-gutter-not-scrolled");

    // Scroll far to the right, with the pointer over the text.
    h.event(Event::PointerMoved(Pos2::new(600.0, 300.0)));
    for _ in 0..12 {
        h.event_modifiers(
            Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(-400.0, 0.0), modifiers: Modifiers::NONE },
            Modifiers::NONE,
        );
        h.run_steps(2);
    }
    h.run_steps(3);
    let (g1, left1, moved1) = h.state().docs[0].gutter_drawn.unwrap();
    eprintln!("scrolled sideways by {moved1} px");
    assert!(moved1 > 300.0, "the text must really have scrolled sideways (only {moved1} px)");
    assert_eq!(left1, left0, "the visible area itself did not move");
    assert_eq!(g1.left(), left1, "scrolled right, the gutter must still be at the left edge of the visible area");
    assert!(g1.width() > 20.0, "the gutter keeps its width");
    save(&mut h, "14-gutter-scrolled-right");

    // And clicking it still toggles a breakpoint on that line.
    let line1_y = {
        let y = g1.top() + 8.0;
        Pos2::new(g1.left() + 9.0, y)
    };
    let before = h.state().breakpoints.values().map(|s| s.len()).sum::<usize>();
    h.event(Event::PointerMoved(line1_y));
    h.event(Event::PointerButton { pos: line1_y, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(Event::PointerButton { pos: line1_y, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    let after = h.state().breakpoints.values().map(|s| s.len()).sum::<usize>();
    assert_eq!(after, before + 1, "a click on the sticky gutter sets a breakpoint");
}
