//! v3 docked inspector: one-column summary for a single item.

use super::ui_automation::{app_with_library, click, leak, render_ui, seed_library};

#[core::prelude::v1::test]
fn docked_inspector_summary_rates_labels_and_opens_get_info() {
    let (temp_dir, engine) = seed_library("inspector_summary", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);

    click(vcx, card);
    click(vcx, "inspector-toggle");
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("inspector-summary").is_some(),
        "a single selection must dock the one-column summary"
    );
    assert!(
        vcx.debug_bounds("inspector-filename").is_some(),
        "the summary must name the file"
    );

    click(vcx, "inspector-star-4");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        4,
        "clicking the fourth star must persist a 4-star rating"
    );

    click(vcx, "inspector-label-2");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(
        labels.get(&item.id).copied(),
        Some(2),
        "clicking a label dot must persist that color label"
    );
    assert_eq!(
        lumina_core::read_sidecar(&item.path)
            .unwrap()
            .map(|xmp| xmp.color_label),
        Some(2),
        "a label change must reach the XMP sidecar like a rating does"
    );

    click(vcx, "inspector-get-info");
    view.update(vcx, |app, _| {
        assert!(
            !app.inspector_open,
            "Get Info must hand off to the floating inspector and close the dock"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Seeded library, window 1440×900, first card selected and Info on.
fn docked<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    lumina_core::MediaItem,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let (view, vcx) = app_with_library(cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", item.id.0)));
    click(vcx, "inspector-toggle");
    render_ui(vcx);
    (dir, engine, item, view, vcx)
}

#[core::prelude::v1::test]
fn tags_field_adds_and_removes_plain_chips() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, item, view, vcx) = docked("inspector_tags", &mut cx);

    click(vcx, "tag-add");
    super::ui_automation::keystrokes(vcx, "b e a c h enter");
    assert_eq!(engine.get_tags_for_item(item.id), vec!["beach".to_string()]);
    render_ui(vcx);
    let chip = vcx
        .debug_bounds("tag-chip-beach")
        .expect("the chip renders");
    let field = vcx.debug_bounds("inspector-tags").unwrap();
    assert!(
        field.contains(&chip.origin),
        "chips sit inside the one field"
    );
    assert_eq!(
        f32::from(chip.size.height),
        28.0,
        "chips are the 28px design Chip"
    );

    click(vcx, "tag-remove-beach");
    assert!(
        engine.get_tags_for_item(item.id).is_empty(),
        "× removes the tag"
    );
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    // Keyboard: ⌫ in the empty field removes the last tag.
    for word in ["s u n enter", "s e a enter", "backspace"] {
        click(vcx, "tag-add");
        super::ui_automation::keystrokes(vcx, word);
    }
    let left = engine.get_tags_for_item(item.id);
    assert_eq!(left.len(), 1, "⌫ removes exactly one tag: {left:?}");
    view.update(vcx, |app, _| {
        assert!(app.tag_input_active, "the field stays open after ⌫");
        assert!(app.delete_confirm_state.is_none(), "⌫ never reaches delete");
    });

    render_ui(vcx);
    click(vcx, leak(format!("tag-filter-{}", left[0])));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query,
            format!("tag:{}", left[0]),
            "a chip's text filters by its tag"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn footer_is_pinned_and_rating_targets_are_22px() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, item, view, vcx) = docked("inspector_footer", &mut cx);

    let summary = vcx.debug_bounds("inspector-summary").unwrap();
    let footer = vcx.debug_bounds("inspector-footer").unwrap();
    assert!(
        (f32::from(summary.bottom()) - f32::from(footer.bottom())).abs() < 1.0,
        "the footer sits at the bottom: {footer:?} in {summary:?}"
    );
    assert_eq!(
        f32::from(summary.size.width) + 1.0,
        300.0,
        "the inspector is 300px with its 1px left border"
    );
    for selector in ["inspector-star-1", "inspector-label-0", "inspector-label-6"] {
        let b = vcx.debug_bounds(selector).unwrap();
        assert_eq!(f32::from(b.size.width), 22.0, "{selector} is a 22px target");
    }

    click(vcx, "inspector-label-3");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(labels.get(&item.id).copied(), Some(3));
    click(vcx, "inspector-label-0");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(
        labels.get(&item.id).copied().unwrap_or(0),
        0,
        "No label clears the color"
    );
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn info_with_nothing_selected_keeps_the_panel() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, _engine, _item, view, vcx) = docked("inspector_empty", &mut cx);
    view.update(vcx, |app, cx| app.clear_selection(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(app.inspector_open, "Info stays on"));
    assert!(
        vcx.debug_bounds("inspector-empty").is_some(),
        "an empty state shows"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn summary_formats_capture_time_place_and_video_strip() {
    let mut item = crate::app::types::get_sample_items()[0].clone();
    item.city = Some("Lisbon".into());
    item.admin1_region = Some("Lisbon".into());
    item.country_code = Some("PT".into());
    assert_eq!(
        crate::views::inspector::summary::place_line(&item).as_deref(),
        Some("Lisbon, PT"),
        "a region equal to the city is not repeated"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(3840, 2160),
        "4K"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(1920, 1080),
        "1080p"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(1000, 700),
        "1000 × 700"
    );
}

#[core::prelude::v1::test]
fn rating_beside_another_apps_sidecar_leaves_it_and_says_so() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, item, view, vcx) = docked("inspector_foreign_sidecar", &mut cx);
    let sidecar = lumina_core::sidecar_path(&item.path);
    let foreign = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:darktable=\"http://darktable.sf.net/\" darktable:history_end=\"3\"/></rdf:RDF></x:xmpmeta>\n";
    std::fs::write(&sidecar, foreign).unwrap();

    click(vcx, "inspector-star-4");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        4,
        "the catalog keeps the rating"
    );
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        foreign,
        "another app's sidecar must not be rewritten"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.last_action.as_deref(),
            Some(
                "1 XMP sidecar was left unchanged: another app wrote it, or Lumina cannot read it"
            ),
            "the skipped sidecar must be reported in plain words"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn info_panel_sections_follow_detail_design() {
    use crate::views::inspector::capture::capture_rows;
    use crate::views::inspector::format_bytes;
    use crate::views::inspector::summary::{format_badge, when_line};

    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, item, view, vcx) = docked("inspector_design", &mut cx);

    for selector in [
        "inspector-filename",
        "inspector-format-badge",
        "inspector-when",
        "tag-add",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the panel must show {selector}"
        );
    }
    assert!(
        vcx.debug_bounds("inspector-capture").is_none(),
        "a photo with no EXIF has no Capture section (no \"—\" rows)"
    );

    click(vcx, "tag-add");
    super::ui_automation::keystrokes(vcx, "b e a c h enter");
    assert_eq!(engine.get_tags_for_item(item.id), vec!["beach".to_string()]);
    render_ui(vcx);
    let chip = vcx
        .debug_bounds("tag-chip-beach")
        .expect("a tag renders as a chip");
    assert_eq!(
        f32::from(chip.size.height),
        28.0,
        "tags use the 28px design Chip"
    );

    // Give the item camera EXIF and a long name, as a real photo would have.
    view.update(vcx, |app, cx| {
        let it = app.selected_item.as_mut().expect("an item is selected");
        it.make = Some("FUJIFILM".into());
        it.model = Some("X100V".into());
        it.lens = Some("23mm F2".into());
        it.f_number = Some(2.0);
        it.exposure_time = Some("1/640".into());
        it.iso = Some(400);
        it.focal_length = Some(23.0);
        it.file_name = format!("{}.jpg", "very_long_file_name_".repeat(8));
        cx.notify();
    });
    render_ui(vcx);
    let capture = vcx
        .debug_bounds("inspector-capture")
        .expect("a photo with EXIF shows the Capture section");
    for selector in [
        "inspector-capture-camera",
        "inspector-capture-lens",
        "inspector-capture-exposure",
        "inspector-capture-focal",
    ] {
        let row = vcx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("Capture must show {selector}"));
        assert_eq!(f32::from(row.size.height), 19.0, "{selector} is a 19px row");
    }
    let star = vcx.debug_bounds("inspector-star-1").unwrap();
    assert!(
        star.top() >= capture.bottom(),
        "Rating & Label sits below Capture: star {star:?} capture {capture:?}"
    );
    let summary = vcx.debug_bounds("inspector-summary").unwrap();
    let badge = vcx.debug_bounds("inspector-format-badge").unwrap();
    let name = vcx.debug_bounds("inspector-filename").unwrap();
    assert!(
        badge.right() <= summary.right() && name.right() <= badge.left(),
        "a long filename keeps the badge visible: name {name:?} badge {badge:?}"
    );

    // The strings behind the rows.
    let mut photo = item.clone();
    photo.file_name = "DSCF4821.RAF".into();
    photo.media_type = lumina_core::MediaType::Raw;
    assert_eq!(format_badge(&photo), "RAW", "RAW files badge as RAW");
    photo.media_type = lumina_core::MediaType::Image;
    photo.file_name = "IMG_1.heic".into();
    assert_eq!(
        format_badge(&photo),
        "HEIC",
        "else the uppercased extension"
    );

    photo.timestamp_epoch_ms = chrono::NaiveDate::from_ymd_opt(2025, 6, 14)
        .unwrap()
        .and_hms_opt(18, 42, 9)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    photo.file_size = 48_200_000;
    assert_eq!(
        when_line(&photo),
        format!("Sat 14 Jun 2025 · 18:42:09 · {}", format_bytes(48_200_000)),
        "the When line has seconds and the size"
    );

    assert!(capture_rows(&photo, None).is_empty(), "no EXIF, no rows");
    photo.make = Some("Canon".into());
    photo.model = Some("Canon EOS R5".into());
    photo.f_number = Some(2.0);
    photo.exposure_time = Some("1/640".into());
    photo.iso = Some(400);
    photo.focal_length = Some(23.0);
    let rows: Vec<(&str, String)> = capture_rows(&photo, Some(35))
        .into_iter()
        .map(|row| (row.selector, row.value))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("inspector-capture-camera", "Canon EOS R5".to_string()),
            (
                "inspector-capture-exposure",
                "ƒ/2.0 · 1/640 · ISO 400".to_string()
            ),
            ("inspector-capture-focal", "23 mm · 35 eq.".to_string()),
        ],
        "rows are omitted one by one; the make is not repeated in the model"
    );

    view.update(vcx, |app, _| assert!(app.error_message.is_none()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The selectors of `capture_rows` with their values.
fn rows_of(item: &lumina_core::MediaItem, focal_35mm: Option<u32>) -> Vec<(&'static str, String)> {
    crate::views::inspector::capture::capture_rows(item, focal_35mm)
        .into_iter()
        .map(|row| (row.selector, row.value))
        .collect()
}

#[core::prelude::v1::test]
fn capture_rows_skip_blank_exif_and_dedupe_the_brand() {
    use crate::views::inspector::summary::format_badge;
    let mut photo = crate::app::types::get_sample_items()[0].clone();
    photo.media_type = lumina_core::MediaType::Image;
    photo.make = Some("Canon".into());
    // The scanner stores an empty tag as Some("").
    photo.model = Some("".into());
    photo.lens = Some("  ".into());
    photo.exposure_time = Some("".into());
    photo.f_number = None;
    photo.iso = None;
    photo.focal_length = None;
    assert_eq!(
        rows_of(&photo, None),
        vec![("inspector-capture-camera", "Canon".to_string())],
        "blank model, lens and shutter make no rows and no trailing space"
    );

    photo.make = Some("NIKON CORPORATION".into());
    photo.model = Some("NIKON D850".into());
    assert_eq!(
        rows_of(&photo, None)[0].1,
        "NIKON D850",
        "a make's first word already in the model is not repeated"
    );
    photo.make = Some("OLYMPUS IMAGING CORP.".into());
    photo.model = Some("E-M1".into());
    assert_eq!(
        rows_of(&photo, None)[0].1,
        "OLYMPUS IMAGING CORP. E-M1",
        "a model without the brand keeps the make"
    );

    photo.file_name = "README".into();
    assert_eq!(format_badge(&photo), "", "no extension, no badge text");
}

#[core::prelude::v1::test]
fn capture_rows_show_duration_and_resolution_for_videos() {
    let mut video = crate::app::types::get_sample_items()[0].clone();
    video.media_type = lumina_core::MediaType::Video;
    video.duration_ms = Some(75_000);
    video.width = 3840;
    video.height = 2160;
    video.make = None;
    video.model = None;
    video.lens = None;
    video.f_number = None;
    video.exposure_time = None;
    video.iso = None;
    video.focal_length = None;
    assert_eq!(
        rows_of(&video, None),
        vec![
            ("inspector-capture-duration", "1:15".to_string()),
            ("inspector-capture-resolution", "4K".to_string()),
        ],
        "a video's Capture rows are its duration and resolution"
    );
}

/// A JPEG container whose EXIF says FocalLengthIn35mmFilm = 35 (in the Exif
/// sub-IFD, where cameras write it). No image data: only the read matters.
fn jpeg_with_35mm_equivalent() -> Vec<u8> {
    #[rustfmt::skip]
    let tiff: [u8; 44] = [
        b'I', b'I', 42, 0, 8, 0, 0, 0,
        // IFD0: one entry, the Exif IFD pointer (LONG) to offset 26.
        1, 0, 0x69, 0x87, 4, 0, 1, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0,
        // Exif IFD: FocalLengthIn35mmFilm (SHORT) = 35.
        1, 0, 0x05, 0xA4, 3, 0, 1, 0, 0, 0, 35, 0, 0, 0, 0, 0, 0, 0,
    ];
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);
    jpeg
}

#[core::prelude::v1::test]
fn docked_capture_reads_the_35mm_equivalent_once_the_selection_rests() {
    use crate::app::info_details::INFO_DETAILS_SETTLE;
    use std::time::Duration;

    let (dir, engine) = seed_library("inspector_settle", 2);
    let items = engine.get_item_slice(0..2);
    std::fs::write(&items[0].path, jpeg_with_35mm_equivalent()).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);

    // Scrub: dock on item 1, then move straight to item 0.
    click(vcx, leak(format!("media-card-{}", items[1].id.0)));
    click(vcx, "inspector-toggle");
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", items[0].id.0)));
    view.update(vcx, |app, cx| {
        app.selected_item.as_mut().unwrap().focal_length = Some(23.0);
        cx.notify();
    });
    render_ui(vcx);
    let focal_35mm = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            let item = app.selected_item.clone().unwrap();
            assert_eq!(item.id, items[0].id);
            app.info_details
                .as_ref()
                .filter(|d| d.is_for(&item))
                .map(|d| d.file.clone())
        })
    };
    assert_eq!(
        focal_35mm(&view, vcx).map(|f| f.focal_length_35mm),
        Some(None),
        "the request is pending, not read, right after the selection moves"
    );

    vcx.executor()
        .advance_clock(INFO_DETAILS_SETTLE - Duration::from_millis(50));
    vcx.run_until_parked();
    assert_eq!(
        focal_35mm(&view, vcx).map(|f| f.focal_length_35mm),
        Some(None),
        "no read before the selection has rested"
    );

    vcx.executor().advance_clock(Duration::from_millis(60));
    vcx.run_until_parked();
    render_ui(vcx);
    assert_eq!(
        focal_35mm(&view, vcx).and_then(|f| f.focal_length_35mm),
        Some(35),
        "the background read lands once the selection rests"
    );
    // What the focal row now shows, from the loaded details.
    let shown = view.update(vcx, |app, _| {
        let item = app.selected_item.clone().unwrap();
        let eq = app
            .info_details
            .as_ref()
            .and_then(|d| d.file.focal_length_35mm);
        rows_of(&item, eq)
    });
    assert!(
        shown.contains(&("inspector-capture-focal", "23 mm · 35 eq.".to_string())),
        "the panel's focal row shows the 35 mm equivalent: {shown:?}"
    );
    assert!(vcx.debug_bounds("inspector-capture-focal").is_some());

    let _ = std::fs::remove_dir_all(&dir);
}
