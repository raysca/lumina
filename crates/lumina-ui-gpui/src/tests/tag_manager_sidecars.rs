//! Tags page edits rewrite the XMP sidecars of the items they change.

use super::tag_manager::{checkbox, count, open};
use crate::tests::ui_automation::{app_with_library, click, keystrokes, render_ui, seed_library};

#[core::prelude::v1::test]
fn rename_keyword_rewrites_sidecars() {
    let (dir, engine) = seed_library("tm_rename_sidecar", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(
            &[item.id],
            &["beach".to_string(), "beach/dunes".to_string()],
        )
        .unwrap();
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    let sidecar_tags = || {
        let mut tags = lumina_core::read_sidecar(&item.path)
            .unwrap()
            .map(|xmp| xmp.tags)
            .unwrap_or_default();
        tags.sort();
        tags
    };
    assert_eq!(sidecar_tags(), ["beach", "beach/dunes"], "Lumina sidecar");

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    click(vcx, "tag-manager-row-beach");
    render_ui(vcx);
    click(vcx, "tag-detail-edit");
    keystrokes(
        vcx,
        "backspace backspace backspace backspace backspace s h o r e enter",
    );
    // The sidecar pass runs on the background executor.
    vcx.run_until_parked();
    assert_eq!(count(&view, vcx, "shore/dunes"), Some(1), "the child moved");
    assert_eq!(
        sidecar_tags(),
        ["shore", "shore/dunes"],
        "a rename rewrites the sidecar of every item it changed"
    );

    view.update(vcx, |app, cx| app.delete_tag_globally("shore".into(), cx));
    vcx.run_until_parked();
    assert!(
        sidecar_tags().is_empty(),
        "a delete rewrites the sidecar too, so Import XMP cannot bring the keyword back"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// One item tagged `tags`, with a Lumina sidecar listing them.
pub(super) fn sidecar_library(
    prefix: &str,
    tags: &[&str],
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    lumina_core::MediaItem,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let items = engine.get_item_slice(0..2);
    let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
    engine.add_tags_batch(&[items[0].id], &tags).unwrap();
    // A second item makes `beach` the most used, so merges fold into it.
    engine
        .add_tags_batch(&[items[1].id], &["beach".to_string()])
        .unwrap();
    engine.write_metadata_sidecars(&[items[0].id]).unwrap();
    (dir, engine, items[0].clone())
}

pub(super) fn sidecar_tags(item: &lumina_core::MediaItem) -> Vec<String> {
    let mut tags = lumina_core::read_sidecar(&item.path)
        .unwrap()
        .map(|xmp| xmp.tags)
        .unwrap_or_default();
    tags.sort();
    tags
}

#[core::prelude::v1::test]
fn merge_and_bulk_delete_rewrite_sidecars() {
    let (dir, engine, item) =
        sidecar_library("tm_merge_sidecar", &["beach", "coast", "sea", "unique"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, checkbox("beach"));
    click(vcx, checkbox("coast"));
    render_ui(vcx);
    click(vcx, "merge-tags-btn");
    vcx.run_until_parked();
    assert_eq!(
        sidecar_tags(&item),
        ["beach", "sea", "unique"],
        "a merge rewrites the sidecars of the items it changed"
    );

    click(vcx, checkbox("sea"));
    click(vcx, checkbox("unique"));
    render_ui(vcx);
    click(vcx, "bulk-delete-tags-btn");
    vcx.run_until_parked();
    assert_eq!(count(&view, vcx, "sea"), None);
    assert_eq!(
        sidecar_tags(&item),
        ["beach"],
        "a bulk delete rewrites the sidecars of the items it changed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
