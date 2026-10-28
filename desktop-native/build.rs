fn main() {
    // Link flags that do not reach dependents, so the app and its tests add them themselves:
    // desktop-capture's ScreenCaptureKit links the Swift runtime, which macOS ships in /usr/lib/swift
    // (like desktop-tauri), and libwebrtc's Objective-C categories need -ObjC or the linker drops them.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        println!("cargo:rustc-link-arg=-ObjC");
    }
    // webrtc-sys links NVENC's CUDA against nvcuda.dll, which only exists with an NVIDIA driver:
    // load it on first use (after webrtc-sys checks it is there), or the exe would not start without one.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:nvcuda.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
}
