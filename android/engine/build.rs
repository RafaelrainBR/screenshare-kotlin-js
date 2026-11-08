fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        webrtc_sys_build::configure_jni_symbols().expect("libwebrtc JNI symbols");
    }
}
