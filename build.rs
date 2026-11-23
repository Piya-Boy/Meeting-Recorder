//! Embeds the app icon into the Windows executable (Explorer, taskbar pin, Alt-Tab).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/app.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/app.ico");
        res.set("ProductName", "Meeting Recorder");
        res.set("FileDescription", "Meeting Recorder");
        if let Err(e) = res.compile() {
            // A missing resource compiler must not break the build: the exe just has no icon.
            println!("cargo:warning=could not embed the exe icon: {e}");
        }
    }
}
