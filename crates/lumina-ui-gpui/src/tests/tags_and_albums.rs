use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::*;

#[core::prelude::v1::test]
fn test_e2e_tag_workflow() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    // Create test photo on disk
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_e2e_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let photo_path = temp_dir.join("photo1.jpg");
    std::fs::write(&photo_path, crate::tests::ui_automation::tiny_jpeg()).unwrap();

    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();

    // Drain until initial scan is complete
    while let Ok(event) = rx.recv() {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }

    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 1);
    let first_item = items[0].clone();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.refresh_album_counts();
        // Select the item
        app.selected_item = Some(first_item);
        app.selected_index = Some(0);
        app.selected_indices.insert(0);
        app.refresh_selected_tags();
        cx.notify();
    });
    cx.run_until_parked();

    // 1. Tag item: open tag input -> type "mountains" -> commit
    view.update(cx, |app, cx| {
        app.start_tag_input(cx);
        assert!(app.tag_input_active);
        app.tag_input_buffer = "mountains".to_string();
        app.commit_tag_input(cx);
        assert!(!app.tag_input_active);
        assert_eq!(app.selected_item_tags, vec!["mountains".to_string()]);
        assert_eq!(app.library_tag_counts, vec![("mountains".to_string(), 1)]);
    });
    cx.run_until_parked();

    // 2. Filter by tag via search
    view.update(cx, |app, cx| {
        app.set_search_query("tag:mountains", cx);
        assert!(
            app.selected_item.is_none(),
            "changing filters must clear the previous selection"
        );
        assert_eq!(app.get_active_count(), 1);
        let slice = app.get_active_slice(0..1);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].file_name, "photo1.jpg");
        // Re-select the filtered item so the rename step has a selection to update.
        app.selected_item = Some(slice[0].clone());
        app.selected_index = Some(0);
        app.selected_indices.insert(0);
        app.refresh_selected_tags();
    });
    cx.run_until_parked();

    // 3. Rename tag: "mountains" -> "alps"
    view.update(cx, |app, cx| {
        app.start_modal_tag_rename("mountains".to_string(), cx);
        assert_eq!(app.tag_manager.editing.as_deref(), Some("mountains"));
        app.tag_manager.editing_buffer = "alps".to_string();
        app.commit_modal_tag_rename(cx);
        assert!(app.tag_manager.editing.is_none());
        assert_eq!(app.library_tag_counts, vec![("alps".to_string(), 1)]);
        assert_eq!(app.selected_item_tags, vec!["alps".to_string()]);
        // Search query should automatically adjust to new tag name
        assert_eq!(app.search_query, "tag:alps");
    });
    cx.run_until_parked();

    // 4. Delete tag globally
    view.update(cx, |app, cx| {
        app.delete_tag_globally("alps".to_string(), cx);
        assert!(app.library_tag_counts.is_empty());
        assert!(app.selected_item_tags.is_empty());
        assert!(app.search_query.is_empty());
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_tag_manager_modal_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_tm_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let photo_path = temp_dir.join("tm_photo.jpg");
    std::fs::write(&photo_path, b"dummy-content").unwrap();

    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();

    while let Ok(event) = rx.recv() {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }

    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 1);
    let test_item = items[0].clone();

    // Add 3 tags to the item
    engine
        .add_tags_batch(
            &[test_item.id],
            &[
                "nature".to_string(),
                "travel".to_string(),
                "forest".to_string(),
            ],
        )
        .unwrap();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.refresh_album_counts();
        app.refresh_tag_counts();
        cx.notify();
    });
    cx.run_until_parked();

    // 1. Open Tag Manager
    view.update(cx, |app, cx| {
        assert!(!app.tags_page_shown());
        app.open_tag_manager(cx);
        assert!(app.tags_page_shown());
        assert_eq!(app.library_tag_counts.len(), 3);
    });
    cx.run_until_parked();

    // 2. Sort and Search in modal
    view.update(cx, |app, cx| {
        app.set_tag_sort_order(TagSortOrder::CountDesc, cx);
        assert_eq!(app.tag_manager.sort, TagSortOrder::CountDesc);
        app.set_tag_sort_order(TagSortOrder::Alphabetical, cx);
        assert_eq!(app.tag_manager.sort, TagSortOrder::Alphabetical);

        // Filter search
        app.tag_manager.search = "nat".to_string();
        cx.notify();
    });
    cx.run_until_parked();

    // 3. Rename in modal: nature -> wilderness
    view.update(cx, |app, cx| {
        app.start_modal_tag_rename("nature".to_string(), cx);
        assert_eq!(app.tag_manager.editing.as_deref(), Some("nature"));
        app.tag_manager.editing_buffer = "wilderness".to_string();
        app.commit_modal_tag_rename(cx);
        assert!(app.tag_manager.editing.is_none());
        let tags = app
            .library_tag_counts
            .iter()
            .map(|(t, _)| t.clone())
            .collect::<Vec<_>>();
        assert!(tags.contains(&"wilderness".to_string()));
        assert!(!tags.contains(&"nature".to_string()));
    });
    cx.run_until_parked();

    // 4. Merge tags in modal: wilderness into forest
    view.update(cx, |app, cx| {
        app.toggle_tag_manager_selection("wilderness".to_string(), cx);
        app.toggle_tag_manager_selection("forest".to_string(), cx);
        assert_eq!(
            app.tag_merge_target().as_deref(),
            Some("forest"),
            "ties merge A–Z"
        );
        app.merge_checked_tags(cx);
        assert!(app.tag_manager.selected.is_empty());
        let tags = app
            .library_tag_counts
            .iter()
            .map(|(t, _)| t.clone())
            .collect::<Vec<_>>();
        assert!(!tags.contains(&"wilderness".to_string()));
        assert!(tags.contains(&"forest".to_string()));
    });
    cx.run_until_parked();

    // 5. Multi-selection & Bulk delete
    view.update(cx, |app, cx| {
        app.toggle_tag_manager_selection("forest".to_string(), cx);
        app.toggle_tag_manager_selection("travel".to_string(), cx);
        assert_eq!(app.tag_manager.selected.len(), 2);
        app.bulk_delete_tags(vec!["forest".to_string(), "travel".to_string()], cx);
        assert!(app.library_tag_counts.is_empty());
    });
    cx.run_until_parked();

    // 6. Escape with nothing checked stays on the page (no sheet to close)
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            assert!(app.tags_page_shown());
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            assert!(app.tags_page_shown(), "Esc does not leave the Tags page");
        });
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_smart_album_and_tag_sidebar_selection_are_exclusive() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_album_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let (engine, _) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.refresh_album_counts();
        cx.notify();
    });
    cx.run_until_parked();

    // 1. Verify Mutual Exclusivity between Smart Album and Tag in Sidebar
    view.update(cx, |app, cx| {
        // Select user smart album (e.g. id = 42)
        app.select_album(AlbumSelection::User(42), cx);
        assert_eq!(app.selection, AlbumSelection::User(42));

        // Now select tag "travel"
        app.select_album(AlbumSelection::Tag("travel".to_string()), cx);
        // Must be Tag("travel"), NOT AlbumSelection::User(42) - mutually exclusive!
        assert_eq!(app.selection, AlbumSelection::Tag("travel".to_string()));
        assert_ne!(app.selection, AlbumSelection::User(42));

        // Toggling tag returns to AllLibrary
        app.select_album(AlbumSelection::AllLibrary, cx);
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_tag_manager_modular_flow() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update_in(cx, |app, window, cx| {
        // Seed library tags
        app.library_tag_counts = vec![
            ("sunset".to_string(), 15),
            ("beach".to_string(), 32),
            ("vacation".to_string(), 8),
        ];

        // No library is open, so `open_tag_manager` would refuse: select
        // the page directly.
        app.selection = AlbumSelection::Tags;
        assert!(app.tags_page_shown());

        // Test search filter
        app.tag_manager.search = "sun".to_string();
        cx.notify();

        // Render the Tags page body without panic
        let _element = crate::views::tag_manager::render(app, window, cx);

        // Test sorting by Alphabetical vs CountDesc
        app.set_tag_sort_order(TagSortOrder::Alphabetical, cx);
        assert_eq!(app.tag_manager.sort, TagSortOrder::Alphabetical);

        app.set_tag_sort_order(TagSortOrder::CountDesc, cx);
        assert_eq!(app.tag_manager.sort, TagSortOrder::CountDesc);

        // Test multi-selection
        app.toggle_tag_manager_selection("sunset".to_string(), cx);
        assert!(app.tag_manager.selected.contains("sunset"));

        app.clear_tag_manager_selection(cx);
        assert!(app.tag_manager.selected.is_empty());

        // Leave the page
        app.select_album(AlbumSelection::AllLibrary, cx);
        assert!(!app.tags_page_shown());
    });
}

#[core::prelude::v1::test]
fn adding_a_keyword_path_stores_it_normalized_and_refuses_over_long_ones() {
    let (temp_dir, engine) = seed_library("keyword_path_add", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.selected_indices = [0].into_iter().collect();
        app.selection_anchor = Some(0);
        app.selected_index = Some(0);
        app.selected_item = Some(item.clone());
        app.update_batch_summary();

        app.batch_add_tag(" Places / Europe ".to_string(), cx);
        assert!(app.error_message.is_none(), "a valid path is written");
        assert_eq!(
            engine.get_tags_for_item(item.id),
            vec!["places/europe".to_string()],
            "the catalog stores the normalized path"
        );
        app.undo_last(cx);
        assert!(
            engine.get_tags_for_item(item.id).is_empty(),
            "undo removes the row the add wrote"
        );

        // Typing is capped, but a parent plus child can still go over.
        let long = format!("{}/{}", "a".repeat(lumina_core::text::MAX_NAME_CHARS), "b");
        app.batch_add_tag(long, cx);
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not add tag")),
            "an over-long path surfaces an error instead of being cut: {:?}",
            app.error_message
        );
        assert!(
            engine.get_tags_for_item(item.id).is_empty(),
            "a refused path writes nothing"
        );
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn renaming_a_tag_stores_the_normalized_path_and_refuses_over_long_ones() {
    let (temp_dir, engine) = seed_library("keyword_path_rename", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(&[item.id], &["trip".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.start_modal_tag_rename("trip".to_string(), cx);
        app.tag_manager.editing_buffer = " Places / Europe ".to_string();
        app.commit_modal_tag_rename(cx);
        assert!(app.error_message.is_none(), "a valid path renames");
        assert_eq!(
            engine.get_tags_for_item(item.id),
            vec!["places/europe".to_string()],
            "a rename with spaces round the slash stores the normalized path"
        );

        app.start_modal_tag_rename("places/europe".to_string(), cx);
        app.tag_manager.editing_buffer = "Places / Asia".to_string();
        app.commit_modal_tag_rename(cx);
        assert_eq!(
            engine.get_tags_for_item(item.id),
            vec!["places/asia".to_string()],
            "the tag manager rename stores the normalized path"
        );

        let long = format!("{}/b", "a".repeat(lumina_core::text::MAX_NAME_CHARS));
        app.start_modal_tag_rename("places/asia".to_string(), cx);
        app.tag_manager.editing_buffer = long;
        app.commit_modal_tag_rename(cx);
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not rename tag")),
            "an over-long rename surfaces an error instead of being cut: {:?}",
            app.error_message
        );
        assert_eq!(
            engine.get_tags_for_item(item.id),
            vec!["places/asia".to_string()],
            "a refused rename changes nothing"
        );
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn untagged_count_refreshes_after_tag_add_remove_rename_merge_and_delete() {
    let (temp_dir, engine) = seed_library("untagged_count", 2);
    let items = engine.get_item_slice(0..2);
    assert_eq!(items.len(), 2, "the fixture must create two media items");
    let first_item = items[0].clone();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        assert_eq!(
            app.untagged_count, 2,
            "a freshly scanned library must be entirely untagged"
        );

        app.selected_indices = [0].into_iter().collect();
        app.selection_anchor = Some(0);
        app.selected_index = Some(0);
        app.selected_item = Some(first_item.clone());
        app.update_batch_summary();

        app.batch_add_tag("travel".to_string(), cx);
        assert_eq!(
            app.untagged_count, 1,
            "adding a tag must refresh the cached untagged count"
        );

        app.batch_remove_tag("travel".to_string(), cx);
        assert_eq!(
            app.untagged_count, 2,
            "removing the last tag must return the item to Untagged"
        );

        app.batch_add_tag("travel".to_string(), cx);
        assert_eq!(app.untagged_count, 1);
        assert!(app.error_message.is_none(), "the tag writes must succeed");

        app.select_album(AlbumSelection::Tag("travel".to_string()), cx);
        app.start_modal_tag_rename("travel".to_string(), cx);
        app.tag_manager.editing_buffer = "trip".to_string();
        app.commit_modal_tag_rename(cx);
        assert_eq!(
            app.untagged_count, 1,
            "renaming a tag must not change untagged membership"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("trip".to_string()),
            "the active tag view must follow the rename"
        );

        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        engine
            .add_tags_batch(&[first_item.id], &["getaway".to_string()])
            .unwrap();
        app.refresh_album_counts();
        app.toggle_tag_manager_selection("trip".to_string(), cx);
        app.toggle_tag_manager_selection("getaway".to_string(), cx);
        app.merge_checked_tags(cx);
        assert_eq!(
            app.untagged_count, 1,
            "merging tags must leave the item tagged"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("getaway".to_string()),
            "the active tag view must follow the merge target"
        );

        app.delete_tag_globally("getaway".to_string(), cx);
        assert_eq!(
            app.untagged_count, 2,
            "deleting the last tag must refresh the untagged count"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "deleting the active tag must fall back to All"
        );
        assert!(
            app.library_tag_counts.is_empty(),
            "the sidebar tag list must be refreshed after the delete"
        );
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn failed_tag_mutation_surfaces_in_the_error_banner_and_keeps_the_tag() {
    let (temp_dir, engine) = seed_library("tag_error", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(&[item.id], &["travel".to_string()])
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        assert!(app
            .library_tag_counts
            .iter()
            .any(|(tag, _)| tag == "travel"));
        app.select_album(AlbumSelection::Tag("travel".to_string()), cx);

        // The UI crate has no raw SQL handle, so the catalog failure is injected
        // at the shared mutation funnel every tag handler reports through.
        let failure: anyhow::Result<usize> = Err(anyhow::anyhow!("database is locked"));
        let applied = app.apply_tag_mutation("Could not delete tag", failure, cx);

        assert!(
            applied.is_none(),
            "a failed tag mutation must not report success to the caller"
        );
        assert_eq!(
            app.error_message.as_deref(),
            Some("Could not delete tag: database is locked"),
            "the catalog error must be visible in the banner"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("travel".to_string()),
            "a failed delete must keep the active tag view"
        );
        assert!(
            app.library_tag_counts
                .iter()
                .any(|(tag, _)| tag == "travel"),
            "a failed delete must not remove the tag from the sidebar"
        );

        app.dismiss_error(cx);
        assert!(app.error_message.is_none());
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

// ---------------------------------------------------------------------------
// Phase 6 (BUGS.md item 17): rendered tag-manager interaction tests.
//
// These drive synthetic clicks at the rendered bounds of the controls the
// modal paints. Before Phase 6 the rows carried no debug selectors and the
// row surface had no click handler, so nothing inside the card could be
// exercised by automation.
// ---------------------------------------------------------------------------

/// Seeds a one-item library with the given tags applied to that item.
fn tagged_library(
    prefix: &str,
    tags: &[&str],
) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let (temp_dir, engine) = seed_library(prefix, 1);
    let item = engine
        .get_item_slice(0..1)
        .into_iter()
        .next()
        .expect("the fixture must create one media item");
    let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
    engine
        .add_tags_batch(&[item.id], &tags)
        .expect("seeding tags must succeed");
    (temp_dir, engine)
}

/// Refreshes the tag list and opens the modal, then paints the real view tree.
fn open_tag_manager(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.refresh_tag_counts();
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
}

/// Builds a per-tag selector such as `tag-manager-row-alpha`.
fn tag_selector(control: &str, tag: &str) -> &'static str {
    leak(format!("tag-manager-{control}-{tag}"))
}

#[core::prelude::v1::test]
fn rendered_row_click_opens_the_tag_in_the_detail_panel() {
    let (temp_dir, engine) = tagged_library("tag_row_click", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_tag_manager(&view, vcx);

    click(vcx, tag_selector("row", "alpha"));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.focused.as_deref(),
            Some("alpha"),
            "clicking a row must show that tag in the detail panel"
        );
        assert!(
            app.tag_manager.selected.is_empty(),
            "a row click must not check the row; the checkbox does that"
        );
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-detail-edit").is_some(),
        "the detail panel must offer Rename for the focused tag"
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn rendered_checkbox_click_toggles_selection_exactly_once() {
    let (temp_dir, engine) = tagged_library("tag_checkbox_click", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_tag_manager(&view, vcx);

    click(vcx, tag_selector("checkbox", "alpha"));
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.selected.contains("alpha"),
            "the checkbox must select the tag"
        );
        assert_eq!(
            app.tag_manager.selected.len(),
            1,
            "the checkbox must not also bubble into the row and toggle twice"
        );
    });

    click(vcx, tag_selector("checkbox", "alpha"));
    view.update(vcx, |app, _| {
        assert!(
            !app.tag_manager.selected.contains("alpha"),
            "a second checkbox click must deselect the tag"
        );
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn rendered_rename_click_starts_editing_without_toggling_the_row() {
    let (temp_dir, engine) = tagged_library("tag_rename_click", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_tag_manager(&view, vcx);

    click(vcx, tag_selector("row", "alpha"));
    click(vcx, "tag-detail-edit");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.editing.as_deref(),
            Some("alpha"),
            "clicking Rename must start editing that tag"
        );
        assert!(
            app.tag_manager.selected.is_empty(),
            "the Rename click must not also toggle the row selection"
        );
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn rendered_show_in_library_filters_and_closes_the_manager() {
    let (temp_dir, engine) = tagged_library("tag_name_click", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_tag_manager(&view, vcx);

    click(vcx, tag_selector("row", "alpha"));
    click(vcx, "tag-detail-view-all");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "tag:alpha",
            "Show in Library must filter the grid by that tag"
        );
        assert!(
            !app.tags_page_shown(),
            "Show in Library must close the tag manager"
        );
        assert!(
            app.tag_manager.selected.is_empty(),
            "Show in Library must not check the row"
        );
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn rendered_search_box_click_focuses_and_typing_filters() {
    let (temp_dir, engine) = tagged_library("tag_search_click", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_tag_manager(&view, vcx);

    // Start from the unfocused state so the click has to create focus.
    view.update(vcx, |app, cx| {
        app.tag_manager.search_focused = false;
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "tag-manager-search");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.search_focused,
            "clicking the search box must focus the filter field"
        );
    });

    vcx.simulate_input("alp");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.search, "alp",
            "typing while the manager is open must filter the tag list"
        );
        assert!(
            app.tag_manager.search_focused,
            "typing must not drop the filter field focus state"
        );
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn keyboard_browses_tags_and_renames_the_focused_one() {
    let (temp_dir, engine) = tagged_library("tag_keyboard", &["alpha", "beta"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_tag_manager(&view, vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "down down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.focused.as_deref(),
            Some("beta"),
            "↓ twice from no focus must land on the second tag"
        );
    });
    keystrokes(vcx, "up");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.editing.as_deref(),
            Some("alpha"),
            "↵ must rename the focused tag"
        );
    });
    keystrokes(vcx, "backspace backspace backspace backspace backspace");
    vcx.simulate_input("gamma");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.focused.as_deref(),
            Some("gamma"),
            "the detail panel must follow a rename"
        );
    });
    assert!(
        engine
            .catalog()
            .get_all_tags()
            .unwrap()
            .contains(&"gamma".to_string()),
        "the rename must reach the catalog"
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn tag_manager_keyboard_focus_scrolls_into_view_and_select_all_checks_rows() {
    let names: Vec<String> = (0..40).map(|i| format!("tag{i:02}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (temp_dir, engine) = tagged_library("tag_scroll", &refs);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(640.0)));
    open_tag_manager(&view, vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "up");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("tag39"));
        assert!(
            app.tag_manager.list_offset_y() < gpui::px(0.0),
            "focusing the last tag by keyboard must scroll the list to it"
        );
    });

    click(vcx, "tag-manager-select-all");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.selected.len(),
            40,
            "select-all must check every shown tag"
        );
    });
    click(vcx, "tag-manager-select-all");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.selected.is_empty(),
            "a second select-all click must clear the checks"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn a_long_name_keeps_the_rename_field_in_the_detail_panel() {
    let (dir, engine) = seed_library("rename_overflow", 2);
    let id = engine.get_item_slice(0..1)[0].id;
    engine.add_tags_batch(&[id], &["trip".to_string()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.open_tag_manager(cx);
        app.focus_tag_manager_tag("trip".to_string(), cx);
        app.start_modal_tag_rename("trip".to_string(), cx);
        app.tag_manager.editing_buffer = "w".repeat(lumina_core::text::MAX_NAME_CHARS);
        cx.notify();
    });
    render_ui(vcx);
    let panel = vcx
        .debug_bounds("tag-manager-detail")
        .expect("the detail panel renders");
    let field = vcx
        .debug_bounds("tag-manager-rename-field")
        .expect("the rename field renders");
    assert!(
        field.right() <= panel.right() && field.left() >= panel.left(),
        "a long name scrolls inside the field instead of widening it: panel {panel:?}, field {field:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn long_tag_and_smart_album_names_stay_inside_their_columns() {
    let (dir, engine) = seed_library("sidebar_long_names", 2);
    let long = "w".repeat(lumina_core::text::MAX_NAME_CHARS);
    let id = engine.get_item_slice(0..1)[0].id;
    engine
        .add_tags_batch(&[id], std::slice::from_ref(&long))
        .unwrap();
    let album = engine
        .create_smart_album(&long, "folder", MatchMode::All, &[])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    let sidebar = vcx.debug_bounds("sidebar").expect("the sidebar renders");
    let delete = vcx
        .debug_bounds(leak(format!("smart-album-delete-{}", album.id)))
        .expect("the album's delete control renders");
    assert!(
        delete.right() <= sidebar.right(),
        "a long album name truncates instead of pushing its controls out: sidebar {sidebar:?}, delete {delete:?}"
    );

    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    render_ui(vcx);
    let detail = vcx
        .debug_bounds("tag-manager-detail")
        .expect("the Tags page renders");
    let tag_row = vcx
        .debug_bounds(leak(format!("tag-manager-row-{long}")))
        .expect("the long tag renders");
    assert!(
        tag_row.right() <= detail.left(),
        "a long tag name truncates in the list instead of pushing into the detail: detail {detail:?}, tag {tag_row:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn typing_an_alias_tags_and_undoes_its_keyword() {
    let (temp_dir, engine) = seed_library("alias_add", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine.set_alias("nyc", "places/usa/new york").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.selected_indices = [0].into_iter().collect();
        app.selection_anchor = Some(0);
        app.selected_index = Some(0);
        app.selected_item = Some(item.clone());
        app.update_batch_summary();

        app.start_tag_input(cx);
        app.tag_input_buffer = "NYC".to_string();
        app.commit_tag_input(cx);
        assert!(app.error_message.is_none());
        assert_eq!(
            engine.get_tags_for_item(item.id),
            vec!["places/usa/new york".to_string()],
            "typing an alias stores its keyword"
        );
        app.undo_last(cx);
        assert!(
            engine.get_tags_for_item(item.id).is_empty(),
            "undo records and removes the keyword row, not the alias"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
