//! The few things that differ between macOS, Linux and Windows in the user
//! interface: how shortcuts are written, how to open files and folders, and
//! (Windows only) how the command line reaches its terminal.
//!
//! egui already maps `Modifiers::COMMAND` to ⌘ on macOS and Ctrl elsewhere, so
//! the shortcuts themselves work everywhere; only their labels need adapting.

use std::path::Path;

/// Adapt a label written with macOS conventions to the current platform:
/// "⇧⌘N" becomes "Ctrl+Shift+N", and "Finder" becomes "Files" on Linux and
/// "Explorer" on Windows.
pub fn k(label: &str) -> String {
    if cfg!(target_os = "macos") {
        return label.to_string();
    }
    let keys = label.replace("⇧⌘", "Ctrl+Shift+").replace('⌘', "Ctrl+");
    if cfg!(windows) {
        keys.replace("Finder", "Explorer")
    } else {
        keys.replace("in Finder", "in the file manager").replace("Finder", "Files")
    }
}

/// Open a file or folder with the system's default application.
pub fn open(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

/// Show a file in the file manager: selected on macOS and Windows, its
/// folder opened on Linux.
pub fn reveal(path: &Path) {
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
        return;
    }
    #[cfg(windows)]
    {
        // Explorer parses its own command line: the path goes right after
        // the comma, in quotes, which normal argument quoting cannot express.
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
    }
    #[cfg(not(windows))]
    open(path.parent().unwrap_or(path));
}

/// Windows: `gbade.exe` is a windowed program (no console window appears
/// when the IDE starts). When it is used as a command-line tool instead,
/// attach to the console of the terminal that started it, so that `println!`
/// output, `Ctrl+C` and the exit code reach that terminal. Does nothing if
/// there is no such console (output redirected to a file or a pipe keeps
/// working either way) and on other platforms.
pub fn attach_parent_console() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        // SAFETY: plain Win32 call without pointers; failure is harmless.
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_platform() {
        if cfg!(target_os = "macos") {
            assert_eq!(k("Reset   ⇧⌘R"), "Reset   ⇧⌘R");
        } else {
            assert_eq!(k("Reset   ⇧⌘R"), "Reset   Ctrl+Shift+R");
            assert_eq!(k("Press ⌘R to run"), "Press Ctrl+R to run");
            if cfg!(windows) {
                assert_eq!(k("Show Project in Finder"), "Show Project in Explorer");
                assert_eq!(k("Finder"), "Explorer");
            } else {
                assert_eq!(k("Show Project in Finder"), "Show Project in the file manager");
                assert_eq!(k("Finder"), "Files");
            }
        }
    }
}
