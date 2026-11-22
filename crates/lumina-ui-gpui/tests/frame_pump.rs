//! The macOS frame pump (src/player/macos_frames.m) on a real clip.
//!
//! AVPlayer only delivers frames while the main run loop runs, and libtest
//! runs tests on worker threads, so this file has its own `main` that pumps
//! the main run loop between polls. The bridge is linked in by build.rs.

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_core_video::{
        CVPixelBuffer, CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType,
        CVPixelBufferGetWidth,
    };
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::time::{Duration, Instant};

    extern "C" {
        fn lumina_player_create() -> *mut c_void;
        fn lumina_player_destroy(player: *mut c_void);
        fn lumina_player_load(
            player: *mut c_void,
            path: *const c_char,
            wait_secs: f64,
            out_duration: *mut f64,
            out_width: *mut c_int,
            out_height: *mut c_int,
            out_error: *mut *mut c_char,
        ) -> c_int;
        fn lumina_player_play(player: *mut c_void);
        fn lumina_player_pause(player: *mut c_void);
        fn lumina_player_seek(player: *mut c_void, seconds: f64);
        fn lumina_player_set_looping(player: *mut c_void, looping: c_int);
        fn lumina_player_copy_frame(
            player: *mut c_void,
            dst: *mut u8,
            capacity: usize,
            out_width: *mut c_int,
            out_height: *mut c_int,
        ) -> c_int;
        fn lumina_player_take_surface(player: *mut c_void) -> *mut c_void;
        fn CVPixelBufferGetIOSurface(buffer: *const c_void) -> *const c_void;
        fn CVPixelBufferRelease(buffer: *const c_void);
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after: u8) -> i32;
    }

    /// The fixture is 64x48, 10 fps, 2 s.
    const W: usize = 64;
    const H: usize = 48;

    struct Player(*mut c_void);

    impl Player {
        fn load() -> Self {
            let path = CString::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/clip.mp4"
            ))
            .expect("fixture path");
            let player = Player(unsafe { lumina_player_create() });
            let (mut duration, mut w, mut h) = (0.0, 0, 0);
            let mut error = std::ptr::null_mut();
            let ok = unsafe {
                lumina_player_load(
                    player.0,
                    path.as_ptr(),
                    0.0,
                    &mut duration,
                    &mut w,
                    &mut h,
                    &mut error,
                )
            };
            assert_eq!(ok, 1, "the fixture must load");
            player
        }

        fn snapshot(&self) -> Option<Vec<u8>> {
            let mut data = vec![0u8; W * H * 4];
            let (mut w, mut h) = (0, 0);
            let status = unsafe {
                lumina_player_copy_frame(self.0, data.as_mut_ptr(), data.len(), &mut w, &mut h)
            };
            (status == 1).then_some(data)
        }

        /// Width, height, pixel format and whether it is IOSurface-backed.
        fn surface(&self) -> Option<(usize, usize, u32, bool)> {
            let raw = unsafe { lumina_player_take_surface(self.0) };
            if raw.is_null() {
                return None;
            }
            // SAFETY: a +1 retained CVPixelBuffer, released below.
            let buffer = unsafe { &*(raw as *const CVPixelBuffer) };
            let info = (
                CVPixelBufferGetWidth(buffer),
                CVPixelBufferGetHeight(buffer),
                CVPixelBufferGetPixelFormatType(buffer),
                !unsafe { CVPixelBufferGetIOSurface(raw) }.is_null(),
            );
            unsafe { CVPixelBufferRelease(raw) };
            Some(info)
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            unsafe { lumina_player_destroy(self.0) };
        }
    }

    type Surface = (usize, usize, u32, bool);

    /// Polls like the UI tick (snapshot, then surface), running the main run
    /// loop between polls, until `done` or `secs` pass.
    fn poll(
        player: &Player,
        secs: f64,
        done: impl Fn(&[Vec<u8>], &[Surface]) -> bool,
    ) -> (Vec<Vec<u8>>, Vec<Surface>) {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        let (mut frames, mut surfaces) = (Vec::new(), Vec::new());
        while Instant::now() < deadline && !done(&frames, &surfaces) {
            frames.extend(player.snapshot());
            surfaces.extend(player.surface());
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.004, 0) };
        }
        (frames, surfaces)
    }

    fn surfaces_are_native_size_iosurface_420f_and_paused_frames_snapshot_too() {
        let player = Player::load();
        unsafe { lumina_player_seek(player.0, 0.5) };
        let (frames, surfaces) = poll(&player, 5.0, |f, s| !f.is_empty() && !s.is_empty());
        let (w, h, format, iosurface) = *surfaces
            .last()
            .expect("a paused seek delivers a GPU surface");
        assert_eq!((w, h), (W, H), "the surface is the video's native size");
        assert_eq!(
            format,
            u32::from_be_bytes(*b"420f"),
            "GPUI's surface renderer only paints full-range 4:2:0"
        );
        assert!(
            iosurface,
            "the surface must be IOSurface-backed for the zero-copy path"
        );
        assert!(
            !frames.is_empty(),
            "a paused frame also comes as a snapshot"
        );
    }

    fn playing_pulls_every_surface_and_snapshots_about_twice_a_second() {
        let player = Player::load();
        unsafe {
            lumina_player_set_looping(player.0, 1);
            lumina_player_play(player.0);
        }
        // 10 fps for 2 s: 20 frames, and 4 or 5 snapshots at 2 per second.
        let (frames, surfaces) = poll(&player, 2.0, |_, _| false);
        unsafe { lumina_player_pause(player.0) };
        assert!(
            surfaces.len() >= 14,
            "every frame reaches the viewer as a surface, got {}",
            surfaces.len()
        );
        assert!(
            (2..=5).contains(&frames.len()),
            "playing snapshots are throttled to about 2 per second, got {}",
            frames.len()
        );
    }

    fn a_seek_drops_a_frame_pulled_before_it() {
        let first = |player: &Player| {
            let (frames, _) = poll(player, 5.0, |f, _| !f.is_empty());
            frames.into_iter().next().expect("a paused frame arrives")
        };
        let at_zero = first(&Player::load());

        let player = Player::load();
        // Let the time-zero frame be pulled and left waiting, untaken.
        let settle = Instant::now() + Duration::from_millis(500);
        while Instant::now() < settle {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, 0) };
        }
        unsafe { lumina_player_seek(player.0, 1.5) };
        assert_ne!(
            first(&player),
            at_zero,
            "the first frame after a seek must be the destination, not the one waiting before it"
        );
    }

    fn a_seek_wakes_a_pump_that_went_idle() {
        let player = Player::load();
        poll(&player, 5.0, |f, _| !f.is_empty());
        // Paused with nothing new: the pump sleeps after about half a second.
        let idle = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < idle {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, 0) };
        }
        unsafe { lumina_player_seek(player.0, 1.0) };
        let (frames, surfaces) = poll(&player, 3.0, |f, s| !f.is_empty() && !s.is_empty());
        assert!(
            !frames.is_empty() && !surfaces.is_empty(),
            "a seek must wake an idle pump and deliver the destination"
        );
    }

    pub fn run() {
        let tests: [(&str, fn()); 4] = [
            (
                "a_seek_wakes_a_pump_that_went_idle",
                a_seek_wakes_a_pump_that_went_idle,
            ),
            (
                "surfaces_are_native_size_iosurface_420f_and_paused_frames_snapshot_too",
                surfaces_are_native_size_iosurface_420f_and_paused_frames_snapshot_too,
            ),
            (
                "playing_pulls_every_surface_and_snapshots_about_twice_a_second",
                playing_pulls_every_surface_and_snapshots_about_twice_a_second,
            ),
            (
                "a_seek_drops_a_frame_pulled_before_it",
                a_seek_drops_a_frame_pulled_before_it,
            ),
        ];
        for (name, test) in tests {
            test();
            println!("test {name} ... ok");
        }
        println!("test result: ok. {} passed; 0 failed", tests.len());
    }
}
