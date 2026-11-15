//! Windows only: embed the application icon and version information in
//! `gbade.exe`, so Explorer, the taskbar, the Start menu shortcut and the
//! Task Manager show GBADE's icon and name. Nothing to do elsewhere: macOS
//! uses the app bundle's icon, Linux the desktop entry's.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    windows_resources();
}

#[cfg(windows)]
fn windows_resources() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico")
        .set("ProductName", "GBADE")
        .set("FileDescription", "GBADE - Game Boy Advance development environment")
        .set("LegalCopyright", "GBADE contributors (MIT OR Apache-2.0)");
    // A missing resource compiler (rc.exe from the Windows SDK) only costs
    // the icon, so it is a warning, not a failed build.
    if let Err(e) = res.compile() {
        println!("cargo:warning=gbade.exe will have no icon: {e}");
    }
}
