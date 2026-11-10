//! Export presets (plan 5 D3): File > Export as JPEG… asks for a folder and
//! writes clean JPEGs there.

use lumina_core::ExportPreset;

use crate::app::library::EXPORT_PICKS;
use crate::app::types::UiMessage;
use crate::tests::ui_automation::{app_with_library, render_ui, seed_library_undecodable};

#[core::prelude::v1::test]
fn export_as_jpeg_uses_picked_folder() {
    let (dir, engine) = seed_library_undecodable("export_jpeg", 1);
    // The seeded file is a stand-in; the JPEG preset decodes, so make it real.
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut bytes)
        .encode_image(&image::RgbImage::from_pixel(
            24,
            12,
            image::Rgb([200, 0, 0]),
        ))
        .unwrap();
    std::fs::write(dir.join("photo_00.jpg"), bytes).unwrap();
    // A quarter turn tells a converted JPEG (12x24) from a copied original (24x12).
    let id = engine.get_item_slice(0..1)[0].id;
    engine.set_rotations(&[(id, 1)]).unwrap();
    let destination = dir.join("picked");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));

    view.update(vcx, |app, _| {
        app.command_query = "jpeg".to_string();
        assert!(
            app.palette_rows()
                .iter()
                .any(|row| row.selector == "command-item-command.export_jpeg"),
            "the palette lists Export as JPEG…"
        );
        app.command_query.clear();
    });
    EXPORT_PICKS.with(|picks| picks.borrow_mut().clear());
    vcx.dispatch_action(crate::actions::ExportAsJpeg);
    vcx.dispatch_action(crate::actions::ExportOriginals);
    assert_eq!(
        EXPORT_PICKS.with(|picks| picks.borrow().clone()),
        vec![ExportPreset::Jpeg2048, ExportPreset::Originals],
        "each File menu item asks for a folder with its own preset"
    );

    // The folder dialog answers with the folder and the preset it was opened for.
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::ExportFolderPicked(destination.clone(), ExportPreset::Jpeg2048),
            cx,
        )
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.jobs.export, None, "the export finished");
        assert!(
            app.toast
                .current()
                .is_some_and(|t| t.message.starts_with("Exported 1")),
            "the existing export toast reports the result, got {:?}",
            app.toast.current().map(|t| &t.message)
        );
    });
    let written: Vec<String> = std::fs::read_dir(&destination)
        .expect("the export writes into the picked folder")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        written,
        vec!["photo_00.jpg".to_string()],
        "one JPEG, no sidecar"
    );
    let exported = image::open(destination.join("photo_00.jpg")).expect("a real JPEG");
    assert_eq!(
        (exported.width(), exported.height()),
        (12, 24),
        "the picked preset is the one that runs: a rotated JPEG, not the original"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn jpeg_export_says_when_a_video_loses_its_rotation() {
    let dir = std::env::temp_dir().join(format!(
        "lumina_uitest_export_video_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("clip.mp4"), b"not really a movie").unwrap();
    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        dir.clone(),
        dir.join("catalog.db"),
        Some(dir.join("cache")),
    )
    .unwrap();
    let scanned = std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(10)).ok())
        .any(|event| matches!(event, lumina_core::EngineEvent::ScanComplete { .. }));
    assert!(scanned, "the initial scan must finish");
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.select_all(cx));

    let toast = |vcx: &mut gpui::VisualTestContext, folder: &str| {
        let destination = dir.join(folder);
        view.update(vcx, |app, cx| {
            app.toast.dismiss();
            app.export_to_folder(destination, ExportPreset::Jpeg2048, cx)
        });
        render_ui(vcx);
        view.update(vcx, |app, _| {
            app.toast
                .current()
                .map(|t| t.message.clone())
                .unwrap_or_default()
        })
    };
    let plain = toast(vcx, "plain");
    assert!(
        plain.contains("1 video exported as the original") && !plain.contains("rotation"),
        "an unrotated video is just counted: {plain:?}"
    );
    engine.set_rotations(&[(id, 1)]).unwrap();
    let turned = toast(vcx, "turned");
    assert!(
        turned.contains("1 video exported as the original (1 without Lumina's rotation)"),
        "a rotated video copied as the original says its rotation stays behind: {turned:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
