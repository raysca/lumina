//! Save Tag as Smart Album with tag names that hold spaces and quotes: the
//! saved rule must match exactly the items under that tag.

use super::downloads::TempLibrary;
use super::tag_manager::open;
use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, seed_library,
};
use lumina_core::{MediaId, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

#[core::prelude::v1::test]
fn save_tag_as_smart_album_with_spaces_and_quotes_matches_exactly() {
    let (dir, engine) = seed_library("tm_save_smart_names", 6);
    let _dir = TempLibrary(dir);
    let mut ids: Vec<u64> = engine.get_item_slice(0..6).iter().map(|i| i.id.0).collect();
    ids.sort();
    let ids: Vec<MediaId> = ids.into_iter().map(MediaId).collect();
    let quoted = r#"o'brien's "best""#;
    // Each name's decoys share a prefix or a word, so a rule that splits on
    // spaces or drops the quotes would match them too.
    let tagged: [(usize, &str); 6] = [
        (0, "new york"),
        (1, "new york/harlem"),
        (2, "new york city"),
        (3, "new"),
        (4, quoted),
        (5, "o'brien's"),
    ];
    for (i, tag) in tagged {
        engine.add_tags_batch(&ids[i..=i], &[tag.into()]).unwrap();
    }
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));

    for (tag, expected) in [("new york", vec![ids[0], ids[1]]), (quoted, vec![ids[4]])] {
        open(&view, vcx);
        click(vcx, leak(format!("tag-manager-row-{tag}")));
        render_ui(vcx);
        click(vcx, "tag-detail-save-smart");
        render_ui(vcx);
        keystrokes(vcx, "enter");
        let album_id = view.update(vcx, |app, _| {
            assert!(!app.album_draft.modal_open, "↵ saves the album for {tag:?}");
            app.user_smart_albums
                .iter()
                .find(|a| {
                    a.rules
                        .iter()
                        .any(|r| r.value == RuleValue::Text(tag.into()))
                })
                .map(|a| a.id)
                .unwrap_or_else(|| panic!("an album with the rule Tag is {tag:?} is saved"))
        });
        let saved = engine
            .list_smart_albums()
            .into_iter()
            .find(|a| a.id == album_id)
            .expect("the album is persisted");
        assert_eq!(
            saved.rules,
            [SmartAlbumRule {
                field: RuleField::Tag,
                operator: RuleOperator::Equals,
                value: RuleValue::Text(tag.into()),
            }],
            "the persisted rule keeps the tag name {tag:?} verbatim"
        );
        let mut matched: Vec<u64> = engine
            .query_smart_album(&saved, 10, 0)
            .iter()
            .map(|item| item.id.0)
            .collect();
        matched.sort();
        let expected: Vec<u64> = expected.iter().map(|id| id.0).collect();
        assert_eq!(
            matched, expected,
            "Tag is {tag:?} must match exactly its items and its children, never a decoy"
        );
        view.update(vcx, |app, _| {
            assert_eq!(
                app.get_active_count(),
                expected.len(),
                "the opened album shows exactly the {tag:?} items"
            )
        });
    }
}
