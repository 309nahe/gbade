//! The forwarder passes arguments, output and the exit code through. On
//! Windows this is checked against the real gbade.exe by `cargo xtask smoke`;
//! here a shell script stands in for it.

#[cfg(unix)]
#[test]
fn arguments_output_and_exit_code_are_forwarded() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let console = dir.path().join("gbade.com");
    std::fs::copy(env!("CARGO_BIN_EXE_gbade-console"), &console).unwrap();
    let gui = dir.path().join("gbade");
    std::fs::write(&gui, "#!/bin/sh\necho \"args:$*\"\necho oops >&2\nexit 7\n").unwrap();
    std::fs::set_permissions(&gui, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = std::process::Command::new(&console).args(["build", "my game"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "args:build my game\n");
    assert_eq!(String::from_utf8_lossy(&out.stderr), "oops\n");
    assert_eq!(out.status.code(), Some(7));
}

#[test]
fn a_missing_gui_program_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let console = dir.path().join(format!("gbade-console{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_gbade-console"), &console).unwrap();
    let out = std::process::Command::new(&console).arg("build").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot find"), "{}", String::from_utf8_lossy(&out.stderr));
}
