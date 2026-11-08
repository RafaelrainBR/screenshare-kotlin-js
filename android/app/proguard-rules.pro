# libwebrtc's Java side (webrtc and jni_zero) is found by name from C++ (ADM, MediaCodec, JNI_OnLoad registration).
-keep class livekit.org.** { *; }
# The Rust engine's exports bind to these native methods by name.
-keep class com.rafaelrain.screenshare.engine.Engine { native <methods>; *; }
# Referenced by jni_zero but generated only in Chromium builds; never loaded here.
-dontwarn livekit.org.jni_zero.JniZeroJni
