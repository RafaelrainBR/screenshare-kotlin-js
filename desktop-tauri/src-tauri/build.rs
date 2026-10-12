fn main() {
    // screencapturekit links the Swift runtime, which macOS ships in /usr/lib/swift.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let architecture = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let target = format!(
            "{}-apple-macos14.2",
            if architecture == "aarch64" {
                "arm64"
            } else {
                &architecture
            }
        );
        let status = std::process::Command::new("swiftc")
            .args([
                "-parse-as-library",
                "-emit-library",
                "-static",
                "-O",
                "-module-name",
                "ScreenshareAppAudio",
                "-target",
                &target,
            ])
            .arg("src/capture/mac_audio.swift")
            .arg("-o")
            .arg(output.join("libScreenshareAppAudio.a"))
            .status()
            .expect("swiftc is required for macOS app audio");
        assert!(status.success(), "macOS app audio compilation failed");
        println!("cargo:rerun-if-changed=src/capture/mac_audio.swift");
        println!("cargo:rustc-link-search=native={}", output.display());
        println!("cargo:rustc-link-lib=static=ScreenshareAppAudio");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AVFAudio");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
