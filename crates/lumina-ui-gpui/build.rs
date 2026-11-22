fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/player/macos_bridge.m")
            .file("src/player/macos_frames.m")
            .file("src/share/macos_share.m")
            .file("src/storage/macos_volume.m")
            .flag("-fobjc-arc")
            .flag("-Wno-deprecated-declarations")
            .flag("-Werror=unguarded-availability-new")
            .compile("lumina_macos_bridge");

        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rerun-if-changed=src/player/macos_bridge.m");
        println!("cargo:rerun-if-changed=src/player/macos_frames.m");
        println!("cargo:rerun-if-changed=src/player/macos_frames.h");
        println!("cargo:rerun-if-changed=src/share/macos_share.m");
        println!("cargo:rerun-if-changed=src/storage/macos_volume.m");
    }
}
