//! The few things that differ between macOS and Linux in the user interface:
//! how shortcuts are written, and how to open files and folders.
//!
//! egui already maps `Modifiers::COMMAND` to ⌘ on macOS and Ctrl elsewhere, so
//! the shortcuts themselves work everywhere; only their labels need adapting.

use std::path::Path;

/// Adapt a label written with macOS conventions to the current platform:
/// on Linux, "⇧⌘N" becomes "Ctrl+Shift+N" and "Finder" becomes "file manager".
pub fn k(label: &str) -> String {
    if cfg!(target_os = "macos") {
        label.to_string()
    } else {
        label
            .replace("⇧⌘", "Ctrl+Shift+")
            .replace('⌘', "Ctrl+")
            .replace("in Finder", "in the file manager")
            .replace("Finder", "Files")
    }
}

/// Open a file or folder with the system's default application.
pub fn open(path: &Path) {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

/// Show a file in the file manager (selected on macOS; its folder elsewhere).
pub fn reveal(path: &Path) {
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    } else {
        open(path.parent().unwrap_or(path));
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
            assert_eq!(k("Show Project in Finder"), "Show Project in the file manager");
            assert_eq!(k("Finder"), "Files");
        }
    }
}
