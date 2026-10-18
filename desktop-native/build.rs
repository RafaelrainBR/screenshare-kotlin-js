fn main() {
    // Link flags that do not reach dependents, so the app and its tests add them themselves:
    // desktop-capture's ScreenCaptureKit links the Swift runtime, which macOS ships in /usr/lib/swift
    // (like desktop-tauri), and libwebrtc's Objective-C categories need -ObjC or the linker drops them.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        println!("cargo:rustc-link-arg=-ObjC");
    }
}
