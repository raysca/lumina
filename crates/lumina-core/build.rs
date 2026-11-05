fn main() {
    println!("cargo:rerun-if-changed=src/video_frame_macos.m");
    // The target, not the host, decides: build scripts run on the host.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    cc::Build::new()
        .file("src/video_frame_macos.m")
        .flag("-fobjc-arc")
        .compile("lumina_video_frame");
    for framework in ["AVFoundation", "CoreMedia", "CoreGraphics", "Foundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
