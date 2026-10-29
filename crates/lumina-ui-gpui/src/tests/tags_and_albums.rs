use crate::tests::ui_automation::{app_with_library, seed_library};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::*;

#[core::prelude::v1::test]
fn test_e2e_tag_workflow() {
    let mut cx = gpui::TestAppContext::single();
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
    std::fs::write(&photo_path, b"test-jpeg-content").unwrap();

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
        app.start_tag_rename("mountains".to_string(), cx);
        assert_eq!(app.editing_tag.as_deref(), Some("mountains"));
        app.editing_tag_buffer = "alps".to_string();
        app.commit_tag_rename(cx);
        assert!(app.editing_tag.is_none());
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
    let mut cx = gpui::TestAppContext::single();
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
        assert!(!app.tag_manager.open);
        app.open_tag_manager(cx);
        assert!(app.tag_manager.open);
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
        app.start_tag_merge("wilderness".to_string(), cx);
        assert_eq!(app.tag_manager.merge_source.as_deref(), Some("wilderness"));
        app.commit_tag_merge("forest".to_string(), cx);
        assert!(app.tag_manager.merge_source.is_none());
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

    // 6. Dismiss modal via Escape key
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            assert!(app.tag_manager.open);
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                },
                window,
                cx,
            );
            assert!(!app.tag_manager.open);
        });
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_smart_album_builder_modal_and_sidebar_mutual_exclusion() {
    let mut cx = gpui::TestAppContext::single();
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

    // 2. Open Smart Album Builder Modal & test Editable Title
    view.update(cx, |app, cx| {
        app.open_album_builder(cx);
        assert!(app.album_draft.modal_open);
        assert_eq!(app.album_draft.album_title, "My Smart Album");

        // Focus title
        app.focus_draft_title(cx);
        assert!(app.album_draft.title_focused);

        // Edit title
        app.album_draft.album_title = "My Smart Vacation".to_string();
        app.unfocus_draft_title(cx);
        assert!(!app.album_draft.title_focused);
        assert_eq!(app.album_draft.album_title, "My Smart Vacation");
    });
    cx.run_until_parked();

    // 3. Test Logic Dropdown selection
    view.update(cx, |app, cx| {
        assert_eq!(app.album_draft.match_mode, MatchMode::All);
        app.toggle_draft_logic_dropdown(cx);
        assert!(app.album_draft.logic_dropdown_open);

        app.select_draft_logic(MatchMode::Any, cx);
        assert_eq!(app.album_draft.match_mode, MatchMode::Any);
        assert!(!app.album_draft.logic_dropdown_open);

        app.toggle_draft_logic_dropdown(cx);
        assert!(app.album_draft.logic_dropdown_open);
        app.select_draft_logic(MatchMode::All, cx);
        assert_eq!(app.album_draft.match_mode, MatchMode::All);
        assert!(!app.album_draft.logic_dropdown_open);
    });
    cx.run_until_parked();

    // 4. Test Condition Editing (Photo choice dropdown, Field dropdown, Operator dropdown, (+) insert and (✕) remove)
    view.update(cx, |app, cx| {
        assert_eq!(app.album_draft.rules.len(), 1);
        assert_eq!(app.album_draft.rules[0].field, RuleField::IsFavorite);
        assert_eq!(app.album_draft.rules[0].value, RuleValue::Bool(true));

        // Change Photo choice to "video"
        app.set_draft_rule_photo_choice(0, "video", cx);
        assert_eq!(app.album_draft.rules[0].field, RuleField::MediaType);
        assert_eq!(
            app.album_draft.rules[0].value,
            RuleValue::Text("video".to_string())
        );

        // Change Operator to "is not"
        app.set_draft_rule_operator(0, RuleOperator::NotEquals, cx);
        assert_eq!(app.album_draft.rules[0].operator, RuleOperator::NotEquals);

        // Insert a second rule row using (+)
        app.insert_draft_rule_after(0, cx);
        assert_eq!(app.album_draft.rules.len(), 2);

        // Change second rule Field to Tag
        app.set_draft_rule_field(1, RuleField::Tag, cx);
        assert_eq!(app.album_draft.rules[1].field, RuleField::Tag);

        // Edit Tag rule value inline
        app.start_editing_rule_value(1, cx);
        assert_eq!(app.album_draft.rule_value_editing, Some(1));
        app.album_draft.rule_value_buffer = "sunset".to_string();
        app.commit_editing_rule_value(cx);
        assert!(app.album_draft.rule_value_editing.is_none());
        assert_eq!(
            app.album_draft.rules[1].value,
            RuleValue::Text("sunset".to_string())
        );

        // Remove first rule using (✕)
        app.remove_draft_rule(0, cx);
        assert_eq!(app.album_draft.rules.len(), 1);
        assert_eq!(app.album_draft.rules[0].field, RuleField::Tag);

        // Save smart album creates album and closes modal
        app.save_draft_smart_album(cx);
        assert!(!app.album_draft.modal_open);
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_tag_manager_modular_flow() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        // Seed library tags
        app.library_tag_counts = vec![
            ("sunset".to_string(), 15),
            ("beach".to_string(), 32),
            ("vacation".to_string(), 8),
        ];

        // Open tag manager modal
        app.open_tag_manager(cx);
        assert!(app.tag_manager.open);

        // Test search filter
        app.tag_manager.search = "sun".to_string();
        cx.notify();

        // Render modular tag manager modal without panic
        let _element = crate::views::tag_manager::TagManagerModal::render(app, cx);

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

        // Close modal
        app.close_tag_manager(cx);
        assert!(!app.tag_manager.open);
    });
}

#[core::prelude::v1::test]
fn untagged_count_refreshes_after_tag_add_remove_rename_merge_and_delete() {
    let (temp_dir, engine) = seed_library("untagged_count", 2);
    let items = engine.get_item_slice(0..2);
    assert_eq!(items.len(), 2, "the fixture must create two media items");
    let first_item = items[0].clone();

    let mut cx = gpui::TestAppContext::single();
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
        app.start_tag_rename("travel".to_string(), cx);
        app.editing_tag_buffer = "trip".to_string();
        app.commit_tag_rename(cx);
        assert_eq!(
            app.untagged_count, 1,
            "renaming a tag must not change untagged membership"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("trip".to_string()),
            "the active tag view must follow the rename"
        );

        app.start_tag_merge("trip".to_string(), cx);
        app.commit_tag_merge("getaway".to_string(), cx);
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

    let mut cx = gpui::TestAppContext::single();
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
