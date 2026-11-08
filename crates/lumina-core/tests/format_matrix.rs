//! Format matrix (plan 5, Task E1): for every image path the decoder takes,
//! does a photo come out upright, in sRGB, and with the right catalog size?
//!
//! Fixtures are built in a temp dir at test time from one 64x32 PNG with a
//! red quadrant on blue, converted with `sips`. Nothing is committed or
//! downloaded. Orientation-6 fixtures store the red quadrant bottom-left, so
//! a correct decode (90 degrees clockwise) shows a 32x64 portrait with the
//! red quadrant top-left; every upright cell expects red top-left.
//!
//! Cells that fail on today's code are `#[ignore]`d with the reason, so the
//! suite stays green and the failures stay visible:
//! `cargo test -p lumina-core --test format_matrix -- --ignored`.
//! Video is covered by the `video_frame` tests and is not repeated here.
#![cfg(target_os = "macos")]

use lumina_core::{
    DecodedFrame, DecoderPipeline, EngineEvent, ExportPreset, LuminaEngine, MediaId, MediaItem,
    MediaType, PixelFormat,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DISPLAY_P3: &str = "/System/Library/ColorSync/Profiles/Display P3.icc";
/// Decode size for the 64x32 fixtures. The sips path scales up to it
/// (`-Z`), so decode cells check the aspect ratio, not an exact size.
const SMALL_DECODE: u32 = 256;
/// The large fixture is over 2560 px and larger than this target, so its
/// decode takes the sips path.
const LARGE_DECODE: u32 = 320;
/// Per-channel distance from sRGB red (255, 0, 0) that a color cell allows.
/// JPEG/HEIC quantization on a flat 16x16 block stays well inside it; the
/// Display P3 encoding of sRGB red read as if it were sRGB is about
/// (234, 51, 35), which the green channel puts outside it.
const COLOR_TOLERANCE: i32 = 20;

/// A temp dir removed on drop, so a failing cell still cleans up.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "lumina_format_matrix_{tag}_{}_{nanos}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// How a fixture is made.
#[derive(Clone, Copy)]
struct Spec {
    /// Tagged EXIF orientation 6 (stored landscape, displayed portrait).
    rotated: bool,
    /// 3000x1500 instead of 64x32.
    large: bool,
    /// Pixels converted to and tagged with Display P3.
    p3: bool,
}

const PLAIN: Spec = Spec {
    rotated: false,
    large: false,
    p3: false,
};
const ROTATED: Spec = Spec {
    rotated: true,
    ..PLAIN
};

/// A 64x32 image (PNG or WebP by extension), blue with a red quadrant:
/// top-left, or bottom-left for a fixture that will be tagged orientation 6.
fn write_png(path: &Path, red_bottom_left: bool) {
    let image = image::RgbImage::from_fn(64, 32, |x, y| {
        let red_row = if red_bottom_left { y >= 16 } else { y < 16 };
        if x < 32 && red_row {
            image::Rgb([255, 0, 0])
        } else {
            image::Rgb([0, 0, 255])
        }
    });
    image.save(path).expect("write fixture image");
}

fn sips(args: &[&str], src: &Path, dst: &Path) {
    let output = Command::new("sips")
        .args(args)
        .arg(src)
        .arg("--out")
        .arg(dst)
        .output()
        .expect("run sips");
    assert!(
        output.status.success() && dst.exists(),
        "sips {args:?} {} failed: {}",
        src.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The sips format name for a fixture extension.
fn sips_format(ext: &str) -> &str {
    match ext {
        "jpg" => "jpeg",
        other => other,
    }
}

/// Builds `dir/name` (the extension picks the format) as `spec` says.
/// A rotated non-JPEG is converted from a tagged JPEG, so it carries
/// whatever orientation sips writes for that format.
fn fixture(dir: &Path, name: &str, spec: Spec) -> PathBuf {
    let out = dir.join(name);
    let ext = name.rsplit('.').next().expect("fixture extension");
    // sips cannot write WebP; the image crate writes both (lossless).
    if ext == "png" || ext == "webp" {
        assert!(!(spec.rotated || spec.large || spec.p3), "plain only");
        write_png(&out, false);
        return out;
    }
    let png = dir.join(format!("{name}.base.png"));
    write_png(&png, spec.rotated);
    let mut args: Vec<&str> = Vec::new();
    if spec.p3 {
        assert!(Path::new(DISPLAY_P3).exists(), "missing {DISPLAY_P3}");
        args.extend(["-m", DISPLAY_P3]);
    }
    if spec.large {
        args.extend(["-z", "1500", "3000"]);
    }
    if spec.rotated && ext != "jpg" {
        let jpeg = dir.join(format!("{name}.rotated.jpg"));
        args.extend(["-s", "format", "jpeg"]);
        sips(&args, &png, &jpeg);
        tag_orientation_6(&jpeg);
        sips(&["-s", "format", sips_format(ext)], &jpeg, &out);
        let _ = fs::remove_file(jpeg);
    } else {
        args.extend(["-s", "format", sips_format(ext)]);
        sips(&args, &png, &out);
        if spec.rotated {
            tag_orientation_6(&out);
        }
    }
    let _ = fs::remove_file(png);
    out
}

/// Replaces the JPEG's EXIF APP1 segment (sips writes one) with a minimal
/// one holding Orientation = 6, after the JFIF APP0 segment.
fn tag_orientation_6(path: &Path) {
    let mut jpeg = fs::read(path).expect("read fixture JPEG");
    assert_eq!(&jpeg[..2], &[0xff, 0xd8], "a JPEG starts with SOI");
    let mut at = 2;
    let mut insert_at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xff && jpeg[at + 1] != 0xda {
        let len = u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]) as usize;
        let marker = jpeg[at + 1];
        if marker == 0xe1 && jpeg[at + 4..].starts_with(b"Exif\0\0") {
            jpeg.drain(at..at + 2 + len);
            continue;
        }
        if marker == 0xe0 {
            insert_at = at + 2 + len;
        }
        at += 2 + len;
    }
    // Big-endian TIFF, IFD0 with one entry: Orientation (0x0112), SHORT, 6.
    let mut app1 = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
    app1.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    segment.extend_from_slice(&app1);
    jpeg.splice(insert_at..insert_at, segment);
    fs::write(path, jpeg).expect("write tagged JPEG");
}

fn decode(path: &Path, size: u32) -> DecodedFrame {
    DecoderPipeline::decode_media(MediaId(1), MediaType::Image, path, size)
        .unwrap_or_else(|err| panic!("{} must decode: {err:#}", path.display()))
}

fn rgb_at(frame: &DecodedFrame, x: u32, y: u32) -> [i32; 3] {
    let i = ((y * frame.width + x) * 4) as usize;
    let p = &frame.data[i..i + 4];
    let (r, b) = match frame.format {
        PixelFormat::Bgra8 => (p[2], p[0]),
        PixelFormat::Rgba8 => (p[0], p[2]),
    };
    [r as i32, p[1] as i32, b as i32]
}

/// Centers of the displayed top-left, top-right, bottom-left and
/// bottom-right quadrants.
fn quadrants(frame: &DecodedFrame) -> [[i32; 3]; 4] {
    let (w, h) = (frame.width, frame.height);
    [
        rgb_at(frame, w / 4, h / 4),
        rgb_at(frame, w * 3 / 4, h / 4),
        rgb_at(frame, w / 4, h * 3 / 4),
        rgb_at(frame, w * 3 / 4, h * 3 / 4),
    ]
}

fn is_red(p: [i32; 3]) -> bool {
    p[0] >= 180 && p[1] <= 90 && p[2] <= 90
}

fn is_blue(p: [i32; 3]) -> bool {
    p[2] >= 180 && p[0] <= 90 && p[1] <= 90
}

/// A 2:1 frame (portrait or landscape) with only the top-left quadrant red.
fn assert_upright(frame: &DecodedFrame, portrait: bool, cell: &str) {
    let (w, h) = (frame.width as i64, frame.height as i64);
    let (long, short) = if portrait { (h, w) } else { (w, h) };
    assert!(
        (long - 2 * short).abs() <= 2,
        "{cell}: an upright decode is {} 2:1, got {w}x{h}",
        if portrait { "portrait" } else { "landscape" }
    );
    let [tl, tr, bl, br] = quadrants(frame);
    assert!(
        is_red(tl) && is_blue(tr) && is_blue(bl) && is_blue(br),
        "{cell}: only the displayed top-left quadrant is red when upright; \
         TL {tl:?} TR {tr:?} BL {bl:?} BR {br:?} (RGB) in {w}x{h}"
    );
}

/// The red quadrant reads as sRGB red within [`COLOR_TOLERANCE`].
fn assert_srgb_red(frame: &DecodedFrame, cell: &str) {
    let red = quadrants(frame)[0];
    let off = [255 - red[0], red[1], red[2]];
    assert!(
        off.iter().all(|d| d.abs() <= COLOR_TOLERANCE),
        "{cell}: red must decode as sRGB (255, 0, 0) within {COLOR_TOLERANCE} \
         per channel, got {red:?}"
    );
}

/// Opens an engine on a fresh library holding only `file`, waits for the
/// initial scan, and returns the catalog row.
fn scan(file: &Path, then: impl FnOnce(&LuminaEngine, &MediaItem)) {
    let root = TempDir::new("scan");
    let media = root.0.join("media");
    fs::create_dir_all(&media).expect("create media dir");
    let name = file.file_name().expect("fixture name");
    fs::copy(file, media.join(name)).expect("copy fixture into library");
    let (engine, rx) =
        LuminaEngine::open_with_paths(media, root.0.join("catalog.db"), Some(root.0.join("cache")))
            .expect("open engine on a temp library");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(EngineEvent::ScanComplete { .. }) => break,
            Ok(_) => {}
            Err(err) => panic!("no ScanComplete from the initial scan: {err}"),
        }
    }
    let items = engine.get_item_slice(0..10);
    let item = items
        .iter()
        .find(|item| item.file_name == name.to_string_lossy())
        .unwrap_or_else(|| panic!("the scan catalogues {name:?}: {items:?}"));
    then(&engine, item);
}

fn assert_scanned_dims(name: &str, spec: Spec, expected: (u32, u32)) {
    let dir = TempDir::new("dims");
    let file = fixture(&dir.0, name, spec);
    scan(&file, |_, item| {
        assert_eq!(
            (item.width, item.height),
            expected,
            "{name}: the catalog stores the displayed (EXIF-oriented) size"
        );
    });
}

fn decode_cell(name: &str, spec: Spec, size: u32) -> DecodedFrame {
    let dir = TempDir::new("decode");
    decode(&fixture(&dir.0, name, spec), size)
}

// JPEG <= 2560 px: zune-jpeg in process.

#[test]
fn jpeg_small_upright() {
    let frame = decode_cell("a.jpg", PLAIN, SMALL_DECODE);
    assert_upright(&frame, false, "JPEG small, orientation 1");
}

#[test]
fn jpeg_small_orientation_6_upright() {
    let frame = decode_cell("a.jpg", ROTATED, SMALL_DECODE);
    assert_upright(&frame, true, "JPEG small, orientation 6");
}

#[test]
fn jpeg_small_srgb_color() {
    let frame = decode_cell("a.jpg", PLAIN, SMALL_DECODE);
    assert_srgb_red(&frame, "JPEG small, sRGB");
}

#[test]
#[ignore = "fails today: zune decodes P3 pixels without converting to sRGB; fixed by E3"]
fn jpeg_small_p3_color() {
    let spec = Spec { p3: true, ..PLAIN };
    let frame = decode_cell("a.jpg", spec, SMALL_DECODE);
    assert_srgb_red(&frame, "JPEG small, Display P3");
}

#[test]
fn jpeg_small_scanned_dims() {
    assert_scanned_dims("a.jpg", PLAIN, (64, 32));
}

#[test]
fn jpeg_small_orientation_6_scanned_dims() {
    assert_scanned_dims("a.jpg", ROTATED, (32, 64));
}

// JPEG > 2560 px: sips.

const LARGE: Spec = Spec {
    large: true,
    ..PLAIN
};

#[test]
fn jpeg_large_upright() {
    let frame = decode_cell("big.jpg", LARGE, LARGE_DECODE);
    assert_upright(&frame, false, "JPEG large, orientation 1");
}

#[test]
fn jpeg_large_orientation_6_upright() {
    let spec = Spec {
        rotated: true,
        ..LARGE
    };
    let frame = decode_cell("big.jpg", spec, LARGE_DECODE);
    assert_upright(&frame, true, "JPEG large, orientation 6");
}

#[test]
fn jpeg_large_srgb_color() {
    let frame = decode_cell("big.jpg", LARGE, LARGE_DECODE);
    assert_srgb_red(&frame, "JPEG large, sRGB");
}

#[test]
#[ignore = "fails today: the sips decode keeps P3 pixels and the PNG reader ignores the profile; fixed by E3"]
fn jpeg_large_p3_color() {
    let spec = Spec { p3: true, ..LARGE };
    let frame = decode_cell("big.jpg", spec, LARGE_DECODE);
    assert_srgb_red(&frame, "JPEG large, Display P3");
}

#[test]
fn jpeg_large_orientation_1_scanned_dims() {
    assert_scanned_dims("big.jpg", LARGE, (3000, 1500));
}

#[test]
fn jpeg_large_orientation_6_scanned_dims() {
    let spec = Spec {
        rotated: true,
        ..LARGE
    };
    assert_scanned_dims("big.jpg", spec, (1500, 3000));
}

// PNG / WebP: the image crate (sRGB only).

#[test]
fn png_upright_and_color() {
    let frame = decode_cell("a.png", PLAIN, SMALL_DECODE);
    assert_upright(&frame, false, "PNG");
    assert_srgb_red(&frame, "PNG");
}

#[test]
fn webp_upright_and_color() {
    let frame = decode_cell("a.webp", PLAIN, SMALL_DECODE);
    assert_upright(&frame, false, "WebP");
    assert_srgb_red(&frame, "WebP");
}

#[test]
fn png_scanned_dims() {
    assert_scanned_dims("a.png", PLAIN, (64, 32));
}

#[test]
fn webp_scanned_dims() {
    assert_scanned_dims("a.webp", PLAIN, (64, 32));
}

// HEIC: sips, then QuickLook.

#[test]
fn heic_upright() {
    let frame = decode_cell("a.heic", PLAIN, SMALL_DECODE);
    assert_upright(&frame, false, "HEIC, orientation 1");
}

#[test]
fn heic_orientation_6_upright() {
    let frame = decode_cell("a.heic", ROTATED, SMALL_DECODE);
    assert_upright(&frame, true, "HEIC from an orientation-6 JPEG");
}

#[test]
fn heic_srgb_color() {
    let frame = decode_cell("a.heic", PLAIN, SMALL_DECODE);
    assert_srgb_red(&frame, "HEIC, sRGB");
}

#[test]
#[ignore = "fails today: the sips decode keeps P3 pixels and the PNG reader ignores the profile; fixed by E3"]
fn heic_p3_color() {
    let spec = Spec { p3: true, ..PLAIN };
    let frame = decode_cell("a.heic", spec, SMALL_DECODE);
    assert_srgb_red(&frame, "HEIC, Display P3");
}

#[test]
fn heic_scanned_dims() {
    assert_scanned_dims("a.heic", PLAIN, (64, 32));
}

#[test]
fn heic_orientation_6_scanned_dims() {
    assert_scanned_dims("a.heic", ROTATED, (32, 64));
}

// TIFF: no image-crate codec, so QuickLook.

#[test]
fn tiff_upright() {
    let frame = decode_cell("a.tiff", PLAIN, SMALL_DECODE);
    assert_upright(&frame, false, "TIFF, orientation 1");
}

#[test]
fn tiff_orientation_6_upright() {
    let frame = decode_cell("a.tiff", ROTATED, SMALL_DECODE);
    assert_upright(&frame, true, "TIFF from an orientation-6 JPEG");
}

#[test]
fn tiff_scanned_dims() {
    assert_scanned_dims("a.tiff", PLAIN, (64, 32));
}

#[test]
fn tiff_orientation_6_scanned_dims() {
    assert_scanned_dims("a.tiff", ROTATED, (32, 64));
}

// Export: JPEG 2048 from a large orientation-6 JPEG (sips decode).

#[test]
fn export_jpeg2048_large_orientation_6_is_upright_portrait() {
    let dir = TempDir::new("export");
    let spec = Spec {
        rotated: true,
        ..LARGE
    };
    let source = fixture(&dir.0, "big.jpg", spec);
    let out = dir.0.join("out");
    scan(&source, |engine, item| {
        let report = engine
            .export_items_with(
                &[item.id],
                &out,
                ExportPreset::Jpeg2048,
                &AtomicBool::new(false),
            )
            .expect("export runs");
        assert_eq!(report.exported, 1, "one JPEG written: {report:?}");
    });
    let image = image::open(out.join("big.jpg"))
        .expect("the export is a readable JPEG")
        .to_rgba8();
    let (width, height) = image.dimensions();
    let frame = DecodedFrame {
        id: MediaId(1),
        width,
        height,
        format: PixelFormat::Rgba8,
        data: image.into_raw(),
    };
    assert!(
        width.max(height) == 2048,
        "a 3000 px source exports with a long side of exactly 2048: {width}x{height}"
    );
    assert_upright(
        &frame,
        true,
        "export JPEG 2048 of a large orientation-6 JPEG",
    );
}

// RAW: owner-supplied files only.

/// Every RAW file in `$LUMINA_RAW_FIXTURES` decodes to a non-blank frame
/// whose shape matches its displayed (EXIF-oriented) size, and the scanner
/// stores that size. Owner-supplied files are never committed.
#[test]
#[ignore = "needs owner-supplied RAW files in $LUMINA_RAW_FIXTURES"]
fn raw_fixture_matrix() {
    let Some(dir) = std::env::var_os("LUMINA_RAW_FIXTURES") else {
        panic!(
            "SKIPPED, not passed: LUMINA_RAW_FIXTURES is not set. Point it at a \
             folder of CR3/NEF/ARW/DNG files to run the RAW cells."
        );
    };
    let files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read LUMINA_RAW_FIXTURES {dir:?}: {err}"))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            matches!(
                ext.as_str(),
                "arw" | "cr2" | "cr3" | "nef" | "dng" | "raf" | "rw2" | "orf"
            )
        })
        .collect();
    assert!(
        !files.is_empty(),
        "SKIPPED, not passed: no RAW files in LUMINA_RAW_FIXTURES {dir:?}"
    );
    let mut failures = Vec::new();
    for path in &files {
        if let Err(err) = check_raw(path) {
            failures.push(format!("{}: {err}", path.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} RAW files failed:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

/// One RAW file: displayed size from sips and EXIF, then decode and scan.
fn check_raw(path: &Path) -> Result<(), String> {
    // Unverified: no real RAW file has been run yet. Whether sips reports
    // pixelWidth/Height before or after orientation, and for which formats,
    // may need adjusting per format once owner files are tried.
    let output = Command::new("sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight"])
        .arg(path)
        .output()
        .map_err(|err| format!("sips -g: {err}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key)?.trim().parse::<u32>().ok())
    };
    let (w, h) = value("pixelWidth:")
        .zip(value("pixelHeight:"))
        .ok_or("sips reports no pixel size")?;
    let orientation = fs::File::open(path)
        .ok()
        .and_then(|file| {
            exif::Reader::new()
                .read_from_container(&mut std::io::BufReader::new(file))
                .ok()
        })
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
                .value
                .get_uint(0)
        })
        .unwrap_or(1);
    let displayed = if (5..=8).contains(&orientation) {
        (h, w)
    } else {
        (w, h)
    };

    let frame = DecoderPipeline::decode_media(MediaId(1), MediaType::Raw, path, 512)
        .map_err(|err| format!("decode: {err:#}"))?;
    let decoded_portrait = frame.height > frame.width;
    if decoded_portrait != (displayed.1 > displayed.0) {
        return Err(format!(
            "decode is {}x{}, displayed size is {}x{} (orientation {orientation})",
            frame.width, frame.height, displayed.0, displayed.1
        ));
    }
    if frame.data.chunks(4).all(|p| p == &frame.data[..4]) {
        return Err("decode is a single flat color".into());
    }
    let mut scanned = (0, 0);
    scan(path, |_, item| scanned = (item.width, item.height));
    if scanned != displayed {
        return Err(format!(
            "catalog size {}x{}, displayed size {}x{}",
            scanned.0, scanned.1, displayed.0, displayed.1
        ));
    }
    Ok(())
}
