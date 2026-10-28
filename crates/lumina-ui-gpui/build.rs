fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/player/macos_bridge.m")
            .flag("-fobjc-arc")
            .flag("-Wno-deprecated-declarations")
            .compile("lumina_player_macos");

        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rerun-if-changed=src/player/macos_bridge.m");
    }
}
