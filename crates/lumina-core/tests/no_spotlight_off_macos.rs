//! Off macOS a video's duration never comes from a program called `mdls`
//! on the PATH: Spotlight is macOS only, and a scan ran it for every video
//! the header parser could not read. Its own test binary, because it
//! changes PATH for the process.

#![cfg(all(unix, not(target_os = "macos")))]

use std::os::unix::fs::PermissionsExt;

#[test]
fn a_video_duration_is_not_read_from_mdls_off_macos() {
    let dir = std::env::temp_dir().join(format!("lumina_no_mdls_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let fake = dir.join("mdls");
    std::fs::write(&fake, "#!/bin/sh\necho 'kMDItemDurationSeconds = 42'\n").expect("fake mdls");
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("executable");
    let clip = dir.join("clip.mkv");
    std::fs::write(&clip, b"not an mp4 header").expect("clip");

    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.clone()];
    paths.extend(std::env::split_paths(&path));
    // This test binary runs this one test, so no other thread reads the
    // environment while it changes.
    std::env::set_var("PATH", std::env::join_paths(paths).expect("PATH"));

    let duration = lumina_core::extract_video_duration(&clip);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(duration, None, "the fake mdls must not be run");
}
