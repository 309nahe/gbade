//! `gbade.com`: GBADE's command line on Windows.
//!
//! Windows programs are either "console" or "windows" (GUI) programs, fixed
//! at build time. `gbade.exe` is a GUI program, so that opening the IDE from
//! Explorer or the Start menu shows no console window. But a terminal does
//! not wait for a GUI program: `gbade.exe build` would return to the prompt
//! at once and lose the exit code.
//!
//! This tiny console program fixes that, the way Visual Studio's
//! `devenv.com` does. It sits next to `gbade.exe` in the ZIP as `gbade.com`;
//! since Windows tries `.com` before `.exe` (see `PATHEXT`), typing `gbade
//! build` in cmd or PowerShell runs it. It starts `gbade.exe` with the same
//! arguments and the same standard input/output, waits, and exits with its
//! exit code. `gbade.exe` attaches to this console (see `platform.rs` in the
//! `gbade` crate), so Ctrl+C reaches it directly; this program ignores
//! Ctrl+C and just reports how the command ended.
//!
//! Without arguments (`gbade` alone) it starts the IDE and returns
//! immediately, like double-clicking `gbade.exe`.
//!
//! The program builds on every platform (it is a plain forwarder there too)
//! but is only shipped on Windows.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// The GUI program this console program stands for: `gbade.exe` next to it
/// (`gbade` on other platforms).
fn gui_program(this: &Path) -> PathBuf {
    this.with_file_name(format!("gbade{}", std::env::consts::EXE_SUFFIX))
}

/// Let Ctrl+C go to `gbade.exe` (attached to the same console) instead of
/// killing this process first, which would return to the prompt while the
/// command is still printing.
fn ignore_ctrl_c() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
        // SAFETY: no handler routine, just "ignore Ctrl+C in this process".
        unsafe {
            SetConsoleCtrlHandler(None, 1);
        }
    }
}

fn main() -> ExitCode {
    let this = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gbade.com"));
    let gui = gui_program(&this);
    if !gui.exists() {
        eprintln!("gbade: cannot find {} (it must be next to {})", gui.display(), this.display());
        return ExitCode::from(1);
    }
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    if args.is_empty() {
        // Just open the IDE; the terminal stays free.
        return match Command::new(&gui).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("gbade: cannot start {}: {e}", gui.display());
                ExitCode::from(1)
            }
        };
    }

    ignore_ctrl_c();
    match Command::new(&gui).args(&args).status() {
        // Exit codes are 0-255 in practice (gbade uses 0-3); anything
        // unusual (a crash, Ctrl+C) still reports a failure.
        Ok(status) => match status.code() {
            Some(c) => ExitCode::from(u8::try_from(c).unwrap_or(1)),
            None => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!("gbade: cannot start {}: {e}", gui.display());
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_to_the_program_next_to_it() {
        let gui = gui_program(Path::new("/opt/GBADE/gbade.com"));
        assert_eq!(gui.parent(), Some(Path::new("/opt/GBADE")));
        let name = gui.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, if cfg!(windows) { "gbade.exe" } else { "gbade" });
    }
}
