//! Tests for the parent module, kept beside it.

use super::*;

#[cfg(target_os = "linux")]
#[test]
fn unsupported_linux_photo_formats_return_clear_preview_errors() {
    let heic = Path::new("/unused/test.heic");
    let error = match DecoderPipeline::decode_media(MediaId(1), MediaType::Image, heic, 128) {
        Ok(_) => panic!("Linux HEIC preview should be unsupported"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("HEIC/HEIF preview decoding is not available on Linux"),
        "HEIC failure should explain the unsupported format: {error:#}"
    );

    let raw = Path::new("/unused/test.dng");
    let error = match DecoderPipeline::decode_media(MediaId(2), MediaType::Raw, raw, 128) {
        Ok(_) => panic!("Linux RAW preview should be unsupported"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("RAW preview decoding is not available on Linux"),
        "RAW failure should explain the unsupported format: {error:#}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn ffmpeg_scaled_image_decode_keeps_tiff_previews_small_without_upscaling() {
    let png = temp_path("lumina_test_ffmpeg_image").with_extension("png");
    let tiff = temp_path("lumina_test_ffmpeg_image").with_extension("tiff");
    let _png_guard = TempFile(png.clone());
    let _tiff_guard = TempFile(tiff.clone());
    let source = image::RgbImage::from_fn(128, 96, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 239) as u8, ((x + y) % 233) as u8])
    });
    source.save(&png).expect("fixture PNG should be written");
    let status = run_with_timeout(
        Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-y", "-i"])
            .arg(&png)
            .arg(&tiff),
        NATIVE_TOOL_TIMEOUT,
    )
    .expect("ffmpeg should create the TIFF fixture");
    assert!(status.success(), "ffmpeg should encode the TIFF fixture");

    let small = DecoderPipeline::decode_image(MediaId(1), &tiff, 32)
        .expect("ffmpeg should decode and scale TIFF previews");
    assert_eq!((small.width, small.height), (32, 24));
    let native = DecoderPipeline::decode_image(MediaId(2), &tiff, 256)
        .expect("ffmpeg should preserve the native dimensions");
    assert_eq!((native.width, native.height), (128, 96));
}

/// Regression: the Linux port routed large JPEGs to the native decoder on
/// macOS too; macOS keeps them in process (zune, capped through sips).
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn scaled_native_decode_routing_per_platform() {
    let large = 9 * 1024 * 1024;
    assert!(
        uses_scaled_native_decode("tiff", 10),
        "TIFFs always go native"
    );
    assert!(
        uses_scaled_native_decode("tif", 10),
        "TIFFs always go native"
    );
    assert!(
        uses_scaled_native_decode("png", large),
        "large PNGs go native"
    );
    assert!(
        !uses_scaled_native_decode("png", 10),
        "small PNGs stay in process"
    );
    assert!(
        !uses_scaled_native_decode("jpg", 10),
        "small JPEGs stay in process"
    );
    assert_eq!(
        uses_scaled_native_decode("jpg", large),
        cfg!(target_os = "linux"),
        "large JPEGs go native only on Linux; macOS caps them in decode_jpeg_simd"
    );
}

/// Regression: the orientation read stopped at the first 8 MB, so a
/// large TIFF whose IFD0 sits further in showed unrotated on Linux. It
/// now seeks to IFD0 instead of reading the file up to it.
#[test]
fn file_orientation_follows_the_tiff_ifd0_offset() {
    // A big-endian TIFF whose IFD0 (Orientation = 6) sits past 8 MB.
    let ifd_at = EXIF_READ_LIMIT as usize + 1024;
    let mut tiff = b"MM\0\x2a".to_vec();
    tiff.extend_from_slice(&(ifd_at as u32).to_be_bytes());
    tiff.resize(ifd_at, 0);
    tiff.extend_from_slice(&[0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
    let far = temp_path("lumina_test_far_ifd").with_extension("tif");
    let _far_guard = TempFile(far.clone());
    fs::write(&far, &tiff).expect("fixture writes");
    assert_eq!(
        DecoderPipeline::file_exif_orientation(&far),
        6,
        "an orientation in an IFD0 past 8 MB is read"
    );

    // Little-endian, Orientation = 8 after another entry.
    let mut tiff = b"II\x2a\0".to_vec();
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&[2, 0]);
    tiff.extend_from_slice(&[0x00, 0x01, 3, 0, 1, 0, 0, 0, 64, 0, 0, 0]);
    tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 8, 0, 0, 0]);
    let little = temp_path("lumina_test_le_ifd").with_extension("tif");
    let _little_guard = TempFile(little.clone());
    fs::write(&little, &tiff).expect("fixture writes");
    assert_eq!(
        DecoderPipeline::file_exif_orientation(&little),
        8,
        "a little-endian TIFF's orientation is read"
    );

    // IFD0 offset past the end of the file: unreadable, upright.
    let mut broken = b"MM\0\x2a".to_vec();
    broken.extend_from_slice(&u32::MAX.to_be_bytes());
    let truncated = temp_path("lumina_test_bad_ifd").with_extension("tif");
    let _truncated_guard = TempFile(truncated.clone());
    fs::write(&truncated, &broken).expect("fixture writes");
    assert_eq!(
        DecoderPipeline::file_exif_orientation(&truncated),
        1,
        "an IFD0 offset past the end reads as upright"
    );

    let near = temp_path("lumina_test_near_ifd").with_extension("jpg");
    let _near_guard = TempFile(near.clone());
    orientation_six_jpeg(&near, 40, 20);
    assert_eq!(
        DecoderPipeline::file_exif_orientation(&near),
        6,
        "a JPEG's orientation is read"
    );
}

/// Regression: the Linux ffmpeg image path ignored EXIF orientation, so a
/// large camera JPEG showed sideways.
#[cfg(target_os = "linux")]
#[test]
fn ffmpeg_image_decode_applies_exif_orientation() {
    let path = temp_path("lumina_test_ffmpeg_orientation").with_extension("jpg");
    let _guard = TempFile(path.clone());
    orientation_six_jpeg(&path, 400, 200);
    let frame = DecoderPipeline::decode_via_ffmpeg_image(MediaId(1), &path, 128)
        .expect("ffmpeg decodes the orientation fixture");
    assert!(
        frame.height > frame.width,
        "orientation 6 displays portrait: {}x{}",
        frame.width,
        frame.height
    );
    let top = &frame.data[(frame.width as usize / 2) * 4..][..4];
    assert!(
        top[0] > 150 && top[2] < 100,
        "the left (red) half is on top: RGBA {top:?}"
    );
}

/// Regression: a native tool still running when the process exits must be
/// killed, not left orphaned (thousands of hung qlmanage processes once
/// exhausted the per-user process limit).
#[cfg(unix)]
#[test]
fn native_tool_dies_when_the_process_exits() {
    const CHILD_ENV: &str = "LUMINA_NATIVE_TOOL_ORPHAN_PIDFILE";
    if let Ok(pidfile) = std::env::var(CHILD_ENV) {
        let script =
            format!("echo $$ > '{pidfile}.tmp' && mv '{pidfile}.tmp' '{pidfile}' && exec sleep 30");
        std::thread::spawn(move || {
            let _ = run_with_timeout(
                Command::new("sh").args(["-c", &script]),
                Duration::from_secs(60),
            );
        });
        while !Path::new(&pidfile).exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        // The shell can write its pid before spawn() returns and the pid is
        // tracked; exiting in that gap would test nothing.
        let pid: u32 = fs::read_to_string(&pidfile)
            .expect("tool wrote its pid")
            .trim()
            .parse()
            .expect("pid is a number");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !live_children::is_tracked(pid) {
            if Instant::now() >= deadline {
                std::process::exit(2);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        std::process::exit(0);
    }

    let pidfile = std::env::temp_dir().join(format!(
        "lumina_orphan_{}_{}.pid",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "decoder::tests::native_tool_dies_when_the_process_exits",
        ])
        .env(CHILD_ENV, &pidfile)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("re-run test binary as the exiting parent");
    assert!(status.success(), "helper process must exit cleanly");
    let pid = fs::read_to_string(&pidfile).expect("helper wrote the tool pid");
    let _ = fs::remove_file(&pidfile);
    let alive = || {
        let exists = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        #[cfg(target_os = "linux")]
        if exists {
            // A killed orphan can remain as a zombie until the container's
            // init reaps it; it is no longer running and cannot leak work.
            return fs::read_to_string(format!("/proc/{}/stat", pid.trim()))
                .ok()
                .and_then(|stat| {
                    stat.rsplit_once(')')
                        .map(|(_, rest)| rest.trim().to_owned())
                })
                .and_then(|rest| rest.chars().next())
                .is_some_and(|state| state != 'Z');
        }
        exists
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    while alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if alive() {
        let _ = Command::new("kill").args(["-9", pid.trim()]).status();
        panic!(
            "native tool {} outlived the process that spawned it",
            pid.trim()
        );
    }
}

/// Re-runs the test binary on test `name` with `env` set, as a process
/// that exits (running its exit hook) instead of the shared test one.
#[cfg(unix)]
fn run_as_child(name: &str, env: (&str, &str)) -> ExitStatus {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name])
        .env(env.0, env.1)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("re-run test binary")
}

/// Regression: a worker still decoding while the process exits can start
/// a tool after the exit hook killed the tracked ones; that late tool
/// must die too, not be reparented to launchd.
#[cfg(unix)]
#[test]
fn native_tool_started_after_the_exit_hook_is_killed() {
    const CHILD_ENV: &str = "LUMINA_TOOL_AFTER_EXIT_HOOK";
    if std::env::var_os(CHILD_ENV).is_some() {
        // Separate process: the hook marks the whole process as exiting.
        live_children::kill_tracked();
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep spawns");
        let _tracked = live_children::track(child.id());
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
        std::process::exit(3);
    }
    let status = run_as_child(
        "decoder::tests::native_tool_started_after_the_exit_hook_is_killed",
        (CHILD_ENV, "1"),
    );
    assert!(
        status.success(),
        "a tool tracked after the exit hook ran must be killed at once: {status}"
    );
}

/// Regression: a worker killed by the process exit mid-decode never drops
/// its temp dir guard; the exit hook removes the dir instead (thousands
/// of empty `lumina_thumb_*` dirs piled up per test run).
#[cfg(unix)]
#[test]
fn temp_dir_of_an_unfinished_decode_is_removed_at_exit() {
    const CHILD_ENV: &str = "LUMINA_TEMP_DIR_AT_EXIT";
    if let Some(dir) = std::env::var_os(CHILD_ENV) {
        let dir = std::path::PathBuf::from(dir);
        let guard = TempDir::new(dir.clone());
        fs::create_dir_all(&dir).expect("temp dir");
        // As a worker thread that never returns: the guard never drops.
        std::mem::forget(guard);
        std::process::exit(0);
    }
    let dir = std::env::temp_dir().join(format!(
        "lumina_exit_dir_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let status = run_as_child(
        "decoder::tests::temp_dir_of_an_unfinished_decode_is_removed_at_exit",
        (CHILD_ENV, dir.to_str().expect("utf-8 temp path")),
    );
    assert!(status.success(), "helper process must exit cleanly");
    let left = dir.exists();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !left,
        "an unfinished decode's temp dir must not outlive the process"
    );
}

/// Regression: an unreadable image fails in milliseconds. QuickLook hangs
/// on such files, so its fallback held a decode worker for the whole tool
/// timeout per broken file.
#[cfg(target_os = "macos")]
#[test]
fn unreadable_image_fails_fast() {
    let dir = std::env::temp_dir().join(format!(
        "lumina_unreadable_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _guard = TempDir::new(dir.clone());
    fs::create_dir_all(&dir).expect("temp dir");
    for name in ["broken.jpg", "broken.heic", "broken.tiff"] {
        let path = dir.join(name);
        fs::write(&path, b"test-jpeg-content").expect("fixture writes");
        let started = Instant::now();
        let decoded = DecoderPipeline::decode_media(MediaId(1), MediaType::Image, &path, 64);
        let took = started.elapsed();
        assert!(decoded.is_err(), "{name}: fake bytes are not an image");
        assert!(
            took < Duration::from_secs(3),
            "{name}: an unreadable image must fail fast, not wait out a tool timeout: {took:?}"
        );
    }
}

/// Regression: two sips decodes of one item at once (a grid thumbnail and
/// the viewer frame) must not share a temp file. The name held only the
/// id and the clock, which macOS reads in microseconds, so one decode
/// could delete or overwrite the other's output.
#[test]
fn temp_paths_never_repeat() {
    assert_ne!(
        temp_path("x"),
        temp_path("x"),
        "two decodes of one item must not share a temp file"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn concurrent_native_decodes_of_one_item_all_succeed() {
    let dir = std::env::temp_dir().join(format!(
        "lumina_native_race_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _guard = TempDir::new(dir.clone());
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("a.png");
    image::RgbImage::from_pixel(16, 16, image::Rgb([200, 60, 40]))
        .save(&path)
        .expect("fixture writes");
    let failures: usize = (0..16)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                (0..4)
                    .filter(|_| DecoderPipeline::decode_via_imageio(MediaId(1), &path, 16).is_err())
                    .count()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().expect("decode thread"))
        .sum();
    assert_eq!(failures, 0, "every concurrent decode of one item succeeds");
}

#[cfg(unix)]
#[test]
fn native_tool_timeout_kills_slow_child() {
    let started = Instant::now();
    let err = run_with_timeout(
        Command::new("sh").args(["-c", "sleep 5"]),
        Duration::from_millis(100),
    )
    .expect_err("slow command must time out");
    assert!(err.to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[cfg(target_os = "linux")]
#[test]
fn cancelled_native_tool_is_killed_while_it_is_running() {
    use std::sync::atomic::{AtomicI32, Ordering};

    let cancel = Arc::new(AtomicI32::new(0));
    let signal = Arc::clone(&cancel);
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        signal.store(1, Ordering::Release);
    });
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep starts for the cancellation test");
    let started = Instant::now();
    let error = wait_with_timeout_or_cancel(child, Duration::from_secs(10), &cancel)
        .expect_err("cancelling a native tool should stop its wait");
    canceller.join().expect("canceller thread completes");
    assert!(error.to_string().contains("cancelled"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "cancellation should kill a running child promptly, took {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn native_tool_returns_fast_status() {
    let status = run_with_timeout(
        Command::new("sh").args(["-c", "exit 0"]),
        Duration::from_secs(2),
    )
    .expect("fast command should run");
    assert!(status.success());
}

fn test_frame(width: u32, height: u32, pixels: &[[u8; 4]]) -> DecodedFrame {
    let mut data = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        data.extend_from_slice(pixel);
    }
    DecodedFrame {
        id: MediaId(1),
        width,
        height,
        format: PixelFormat::Bgra8,
        data,
    }
}

fn flatten(pixels: &[[u8; 4]]) -> Vec<u8> {
    pixels.iter().flatten().copied().collect()
}

#[test]
fn quarter_turn_normalization_wraps_signed_values() {
    assert_eq!(crate::normalize_quarter_turns(0), 0);
    assert_eq!(crate::normalize_quarter_turns(1), 1);
    assert_eq!(crate::normalize_quarter_turns(3), 3);
    assert_eq!(crate::normalize_quarter_turns(4), 0, "a full turn is 0");
    assert_eq!(
        crate::normalize_quarter_turns(-1),
        3,
        "Rotate Left from 0 must wrap to 3"
    );
    assert_eq!(crate::normalize_quarter_turns(-4), 0);
    assert_eq!(crate::normalize_quarter_turns(7), 3);
    assert_eq!(crate::normalize_quarter_turns(i32::MIN), 0);
}

#[test]
fn rotate_frame_maps_pixels_for_each_quarter_turn() {
    let a = [10, 11, 12, 13];
    let b = [20, 21, 22, 23];
    let c = [30, 31, 32, 33];
    let d = [40, 41, 42, 43];
    let pixels = [a, b, c, d];

    let source = test_frame(2, 2, &pixels);
    let original_data = source.data.clone();
    let right = DecoderPipeline::rotate_frame(source, 1);
    assert_eq!((right.width, right.height), (2, 2));
    assert_eq!(right.data, flatten(&[c, a, d, b]), "clockwise turn");

    let left = DecoderPipeline::rotate_frame(test_frame(2, 2, &pixels), 3);
    assert_eq!(left.data, flatten(&[b, d, a, c]), "counter-clockwise turn");

    let half = DecoderPipeline::rotate_frame(test_frame(2, 2, &pixels), 2);
    assert_eq!(half.data, flatten(&[d, c, b, a]), "half turn");

    let mut repeated = test_frame(2, 2, &pixels);
    for _ in 0..4 {
        repeated = DecoderPipeline::rotate_frame(repeated, 1);
    }
    assert_eq!(repeated.data, original_data, "four turns are the identity");

    let round_trip = DecoderPipeline::rotate_frame(right, 3);
    assert_eq!(
        round_trip.data, original_data,
        "a left turn must undo a right turn"
    );
}

#[test]
fn rotate_frame_swaps_the_long_edge_for_quarter_turns() {
    let wide = test_frame(2, 1, &[[1, 2, 3, 4], [5, 6, 7, 8]]);
    let quarter = DecoderPipeline::rotate_frame(wide, 1);
    assert_eq!(
        (quarter.width, quarter.height),
        (1, 2),
        "a 90° turn must swap width and height"
    );
    assert_eq!(
        quarter.data,
        flatten(&[[1, 2, 3, 4], [5, 6, 7, 8]]),
        "the left pixel becomes the top pixel"
    );

    let half = DecoderPipeline::rotate_frame(
        test_frame(
            3,
            2,
            &[
                [1, 1, 1, 1],
                [2, 2, 2, 2],
                [3, 3, 3, 3],
                [4, 4, 4, 4],
                [5, 5, 5, 5],
                [6, 6, 6, 6],
            ],
        ),
        2,
    );
    assert_eq!((half.width, half.height), (3, 2), "a half turn keeps size");
}

#[test]
fn rotate_frame_leaves_unsupported_buffers_unchanged() {
    let odd = DecodedFrame {
        id: MediaId(1),
        width: 2,
        height: 1,
        format: PixelFormat::Rgba8,
        data: vec![1, 2, 3],
    };
    let rotated = DecoderPipeline::rotate_frame(odd, 1);
    assert_eq!((rotated.width, rotated.height), (2, 1));
    assert_eq!(rotated.data, vec![1, 2, 3]);
}

/// Regression: videos went to a qlmanage subprocess that hung until its
/// timeout on some files, leaving their cards blank.
#[cfg(target_os = "macos")]
#[test]
fn videos_decode_in_process_through_avfoundation() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../lumina-ui-gpui/tests/fixtures/clip.mp4");
    let frame = DecoderPipeline::decode_media(MediaId(3), MediaType::Video, &fixture, 320)
        .expect("a video thumbnail must decode");
    assert_eq!(
        (frame.width, frame.height, frame.format),
        (64, 48, PixelFormat::Bgra8),
        "video frames come back at their own size in the texture format"
    );
}

/// A JPEG whose left half is red and right half blue, tagged EXIF
/// Orientation 6 (display turned 90° clockwise: the left half on top).
pub(crate) fn orientation_six_jpeg(path: &Path, width: u32, height: u32) {
    let image = image::RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            image::Rgb([230, 20, 20])
        } else {
            image::Rgb([20, 20, 230])
        }
    });
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode_image(&image)
        .expect("fixture encodes");
    // Big-endian TIFF, IFD0 with one entry: Orientation (0x0112) = 6.
    let mut app1 = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
    app1.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    segment.extend_from_slice(&app1);
    // After SOI and the encoder's APP0 (JFIF) segment.
    let app0_end = 4 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
    jpeg.splice(app0_end..app0_end, segment);
    fs::write(path, jpeg).expect("fixture writes");
}

#[test]
fn icc_description_reads_v2_desc_and_v4_mluc() {
    let profiles = Path::new("/System/Library/ColorSync/Profiles");
    if profiles.is_dir() {
        // sRGB Profile.icc is a v2 `desc` tag, Display P3.icc a v4 `mluc`.
        let read = |name: &str| fs::read(profiles.join(name)).expect("system ICC profile");
        let srgb = read("sRGB Profile.icc");
        let p3 = read("Display P3.icc");
        assert_eq!(icc_description(&srgb).as_deref(), Some("sRGB IEC61966-2.1"));
        assert_eq!(icc_description(&p3).as_deref(), Some("Display P3"));
        assert!(is_srgb_profile(&srgb), "sRGB is not converted");
        assert!(!is_srgb_profile(&p3), "Display P3 must be converted");
        let desc = 132
            + 12 * (0..)
                .find(|i| &srgb[132 + 12 * i..136 + 12 * i] == b"desc")
                .expect("desc tag");
        let be = |at: usize| u32::from_be_bytes(srgb[at..at + 4].try_into().unwrap()) as usize;
        let (body, body_len) = (be(desc + 4), be(desc + 8));
        for len in 0..srgb.len() {
            let parsed = icc_description(&srgb[..len]);
            if len < body + body_len {
                assert_eq!(
                    parsed, None,
                    "a profile cut to {len} bytes, inside its desc tag, has no description"
                );
            }
        }
        let mut huge = srgb.clone();
        huge[body + 8..body + 12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(icc_description(&huge), None, "a huge desc length");
    }
    assert_eq!(icc_description(b"not a profile"), None);
    assert_eq!(icc_description(&[0xff; 200]), None, "bad tag count");
    assert!(
        !is_srgb_profile(b"not a profile"),
        "an unparseable profile counts as non-sRGB"
    );
}

/// `jpeg` with `icc` spliced in as a single APP2 ICC_PROFILE segment.
#[cfg(target_os = "macos")]
fn with_icc(jpeg: &[u8], icc: &[u8]) -> Vec<u8> {
    let mut payload = b"ICC_PROFILE\0\x01\x01".to_vec();
    payload.extend_from_slice(icc);
    let mut segment = vec![0xff, 0xe2];
    segment.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    segment.extend_from_slice(&payload);
    let mut out = jpeg.to_vec();
    out.splice(2..2, segment);
    out
}

/// Regression for the sRGB conversion: an untagged or sRGB-tagged JPEG
/// keeps its pixels (no second conversion, no shift), while a Display P3
/// one is converted.
#[cfg(target_os = "macos")]
#[test]
fn srgb_and_untagged_jpegs_are_not_converted() {
    let image = image::RgbImage::from_fn(64, 32, |x, _| {
        if x < 32 {
            image::Rgb([200, 60, 40])
        } else {
            image::Rgb([40, 120, 220])
        }
    });
    let mut untagged = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut untagged, 95)
        .encode_image(&image)
        .expect("fixture encodes");
    let profiles = Path::new("/System/Library/ColorSync/Profiles");
    let srgb = with_icc(
        &untagged,
        &fs::read(profiles.join("sRGB Profile.icc")).expect("sRGB profile"),
    );
    let p3 = with_icc(
        &untagged,
        &fs::read(profiles.join("Display P3.icc")).expect("P3 profile"),
    );

    let dir = std::env::temp_dir().join(format!(
        "lumina_icc_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    let _guard = TempDir::new(dir.clone());
    let decode = |name: &str, bytes: &[u8]| {
        let path = dir.join(name);
        fs::write(&path, bytes).expect("fixture writes");
        DecoderPipeline::decode_media(MediaId(1), MediaType::Image, &path, 64)
            .expect("fixture decodes")
    };
    let plain = decode("untagged.jpg", &untagged);
    let tagged = decode("srgb.jpg", &srgb);
    let wide = decode("p3.jpg", &p3);

    // BGRA at the centre of the left half.
    let left = |f: &DecodedFrame| {
        let i = (16 * f.width as usize + 16) * 4;
        [f.data[i + 2], f.data[i + 1], f.data[i]]
    };
    let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3);
    assert!(
        near(left(&plain), [200, 60, 40]),
        "an untagged JPEG keeps its sRGB values: {:?}",
        left(&plain)
    );
    assert_eq!(
        tagged.data, plain.data,
        "an sRGB-tagged JPEG decodes exactly like the untagged one"
    );
    assert!(
        !near(left(&wide), left(&plain)),
        "the same values tagged Display P3 are converted to sRGB: {:?}",
        left(&wide)
    );
}

/// Regression: the image-crate fallback must keep image's 512 MiB output
/// buffer guard. A tiny WebP whose header declares 16383x16383 (about
/// 768 MiB of RGB) is refused before any buffer is allocated, rather
/// than allocated (an abort when the allocation fails).
#[test]
fn huge_declared_image_is_refused_before_allocating() {
    // VP8L header: signature 0x2f, then 14-bit width-1 and height-1
    // (both 16382, so 16383 px), no alpha, version 0; a few bytes of
    // bitstream.
    let size_bits: u32 = 16382 | (16382 << 14);
    let mut vp8l = vec![0x2f];
    vp8l.extend_from_slice(&size_bits.to_le_bytes());
    vp8l.extend_from_slice(&[0; 11]);
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(&((4 + 8 + vp8l.len()) as u32).to_le_bytes());
    webp.extend_from_slice(b"WEBPVP8L");
    webp.extend_from_slice(&(vp8l.len() as u32).to_le_bytes());
    webp.extend_from_slice(&vp8l);

    let path = std::env::temp_dir().join(format!(
        "lumina_huge_{}_{}.webp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&path, &webp).expect("fixture writes");
    let _guard = TempFile(path.clone());
    let Err(err) = DecoderPipeline::decode_image(MediaId(1), &path, 256) else {
        panic!("a 16383x16383 image must not decode in process");
    };
    assert!(
        matches!(
            err.downcast_ref::<image::ImageError>(),
            Some(image::ImageError::Limits(_))
        ),
        "the output buffer guard refuses it, not a later decode error: {err:#}"
    );
}

/// The sRGB match that ImageIO applies to every decode must leave sRGB and
/// untagged pixels alone (no shift), checked against a lossless source.
#[cfg(target_os = "macos")]
#[test]
fn imageio_decode_does_not_shift_srgb_or_untagged_pixels() {
    use image::ImageEncoder;
    let image = image::RgbImage::from_fn(64, 32, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 8 + x) as u8, (255 - x * 3) as u8])
    });
    let encode = |icc: Option<Vec<u8>>| {
        let mut png = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
        if let Some(icc) = icc {
            encoder
                .set_icc_profile(icc)
                .expect("PNG takes an ICC profile");
        }
        encoder
            .write_image(&image, 64, 32, image::ExtendedColorType::Rgb8)
            .expect("fixture encodes");
        png
    };
    let srgb =
        fs::read("/System/Library/ColorSync/Profiles/sRGB Profile.icc").expect("sRGB profile");
    let dir = std::env::temp_dir().join(format!(
        "lumina_imageio_shift_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    let _guard = TempDir::new(dir.clone());
    for (name, png) in [
        ("untagged.png", encode(None)),
        ("srgb.png", encode(Some(srgb))),
    ] {
        let path = dir.join(name);
        fs::write(&path, png).expect("fixture writes");
        let frame = DecoderPipeline::decode_via_imageio(MediaId(1), &path, 64)
            .expect("ImageIO decodes the fixture");
        assert_eq!(
            (frame.width, frame.height),
            (64, 32),
            "{name}: not rescaled"
        );
        let worst = frame
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(image.pixels())
            .map(|(bgra, rgb)| {
                (0..3)
                    .map(|c| bgra[2 - c].abs_diff(rgb[c]))
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);
        assert!(
            worst <= 2,
            "{name}: the sRGB match must not shift pixels, worst channel off by {worst}"
        );
    }
}

/// Regression: the native path (large JPEGs at small sizes, HEIC, RAW)
/// returned unrotated pixels while the in-process path applied EXIF
/// orientation, so a crop picked a different region per decode size.
#[cfg(target_os = "macos")]
#[test]
fn native_and_in_process_decodes_agree_on_exif_orientation() {
    let dir = std::env::temp_dir().join(format!(
        "lumina_orientation_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("rotated.jpg");
    orientation_six_jpeg(&path, 2800, 1400);

    // 320 goes through ImageIO (over 2560 px), 4096 decodes with zune.
    for size in [320, 4096] {
        let frame = DecoderPipeline::decode_media(MediaId(1), MediaType::Image, &path, size)
            .expect("the fixture decodes");
        assert!(
            frame.height > frame.width,
            "orientation 6 displays portrait at size {size}: {}x{}",
            frame.width,
            frame.height
        );
        let top = &frame.data[(frame.width as usize / 2) * 4..][..4];
        assert!(
            top[2] > 150 && top[0] < 100,
            "the left (red) half is on top at size {size}: BGRA {top:?}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// Sum of horizontal neighbour differences in the middle row: a proxy for
/// retained edge contrast.
fn edge_energy(img: &image::RgbaImage) -> u64 {
    let (w, h) = img.dimensions();
    let row = h / 2;
    (1..w)
        .map(|x| {
            let a = img.get_pixel(x, row)[1] as i32;
            let b = img.get_pixel(x - 1, row)[1] as i32;
            (a - b).unsigned_abs() as u64
        })
        .sum()
}

/// Regression: thumbnails were downscaled with `FilterType::Triangle`
/// (bilinear), which visibly softens a reduced photo. The decode path must
/// stay sharper than a plain bilinear pass over the same source.
#[test]
fn thumbnail_downscale_keeps_edges_sharper_than_bilinear() {
    // 6 px vertical bars in a 600 px source: a 6x reduction to 100 px.
    let source = image::RgbImage::from_fn(600, 600, |x, _| {
        image::Rgb(if (x / 6) % 2 == 0 {
            [0, 0, 0]
        } else {
            [255, 255, 255]
        })
    });
    let frame =
        DecoderPipeline::resize_to_frame(MediaId(1), DynamicImage::ImageRgb8(source.clone()), 100);
    assert_eq!((frame.width, frame.height), (100, 100));
    let decoded = image::RgbaImage::from_raw(frame.width, frame.height, frame.data)
        .expect("the frame keeps its dimensions");
    let bilinear = DynamicImage::ImageRgb8(source)
        .resize(100, 100, FilterType::Triangle)
        .to_rgba8();

    let (sharp, soft) = (edge_energy(&decoded), edge_energy(&bilinear));
    assert!(soft > 0, "the bilinear reference must keep some contrast");
    assert!(
        sharp > soft,
        "a thumbnail ({sharp}) must keep more edge contrast than a bilinear downscale ({soft})"
    );
}

/// A fresh temp dir for one test, removed when the guard drops.
#[cfg(target_os = "macos")]
fn imageio_dir(name: &str) -> (std::path::PathBuf, TempDir) {
    let dir = temp_path(&format!("lumina_imageio_{name}"));
    let guard = TempDir::new(dir.clone());
    fs::create_dir_all(&dir).expect("temp dir");
    (dir, guard)
}

/// ImageIO fits the decode inside the size asked for but, like the
/// in-process decoders, never enlarges it (sips did).
#[cfg(target_os = "macos")]
#[test]
fn imageio_decode_fits_the_size_and_never_enlarges() {
    let (dir, _guard) = imageio_dir("fit");
    let path = dir.join("wide.png");
    image::RgbImage::from_pixel(400, 200, image::Rgb([10, 200, 30]))
        .save(&path)
        .expect("fixture writes");
    let fitted = DecoderPipeline::decode_via_imageio(MediaId(1), &path, 100).expect("decodes");
    assert_eq!(
        (fitted.width, fitted.height),
        (100, 50),
        "fits inside 100 px"
    );
    assert_eq!(fitted.data.len(), 100 * 50 * 4, "a tight BGRA buffer");
    let whole = DecoderPipeline::decode_via_imageio(MediaId(1), &path, 2048).expect("decodes");
    assert_eq!(
        (whole.width, whole.height),
        (400, 200),
        "a decode larger than the image keeps its own size"
    );
}

/// Frames hold straight alpha, as the image crate decodes it: ImageIO's
/// premultiplied pixels are divided back out.
#[cfg(target_os = "macos")]
#[test]
fn imageio_decode_keeps_straight_alpha() {
    let (dir, _guard) = imageio_dir("alpha");
    let path = dir.join("half.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 100, 50, 128]))
        .save(&path)
        .expect("fixture writes");
    let frame = DecoderPipeline::decode_via_imageio(MediaId(1), &path, 8).expect("decodes");
    let bgra = &frame.data[..4];
    let expected = [50u8, 100, 200, 128];
    assert!(
        bgra.iter().zip(expected).all(|(&got, want)| got.abs_diff(want) <= 2),
        "a half-transparent pixel keeps its colour unpremultiplied: got BGRA {bgra:?}, want {expected:?}"
    );
}

/// A file ImageIO cannot read fails fast with an error, and reports no
/// pixel size, rather than a frame.
#[cfg(target_os = "macos")]
#[test]
fn imageio_rejects_a_file_that_is_not_an_image() {
    let (dir, _guard) = imageio_dir("reject");
    let path = dir.join("fake.heic");
    fs::write(&path, b"not an image at all").expect("fixture writes");
    assert!(DecoderPipeline::decode_via_imageio(MediaId(1), &path, 64).is_err());
    assert_eq!(native_pixel_size(&path), None);
    let good = dir.join("good.png");
    image::RgbImage::new(30, 20)
        .save(&good)
        .expect("fixture writes");
    assert_eq!(
        native_pixel_size(&good),
        Some((30, 20)),
        "control: a PNG's size"
    );
}
