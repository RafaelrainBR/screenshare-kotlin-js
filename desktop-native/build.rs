fn main() {
    // desktop-capture's ScreenCaptureKit links the Swift runtime, which macOS ships in /usr/lib/swift;
    // its rpath does not reach dependents, so the app and its tests add it (like desktop-tauri).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
