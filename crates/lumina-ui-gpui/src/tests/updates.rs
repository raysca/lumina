//! Self-update: Check for Updates downloads a newer release, Restart to
//! Update installs it at quit and relaunches, Later installs at the next
//! quit, and Check Daily is opt-in. Fake backends: no test goes online or
//! replaces an app.

use std::cell::Cell;
use std::sync::atomic::AtomicBool;

use anyhow::{anyhow, Result};
use gpui::{Entity, VisualTestContext};
use lumina_core::update::install::StagedUpdate;
use lumina_core::update::Release;

use super::downloads_screen::marker;
use super::helpers::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library, type_text, TempLibrary,
};
use crate::app::updates::{UpdateBackend, UpdatePhase};
use crate::app::LuminaApp;

thread_local! {
    // Install and relaunch run on the UI thread, at quit.
    static INSTALLS: Cell<usize> = const { Cell::new(0) };
    static RELAUNCHES: Cell<usize> = const { Cell::new(0) };
}

fn release(version: &str) -> Release {
    Release {
        version: version.into(),
        page_url: "https://example.test/release".into(),
        zip: None,
        sums_url: None,
    }
}

fn newer(_: &AtomicBool) -> Result<Release> {
    Ok(release("99.0.0"))
}

fn staged(release: &Release, _: &AtomicBool) -> Result<StagedUpdate> {
    Ok(StagedUpdate::for_tests(&release.version))
}

fn installed(_: &StagedUpdate) -> Result<()> {
    INSTALLS.with(|n| n.set(n.get() + 1));
    Ok(())
}

fn relaunched() -> Result<()> {
    RELAUNCHES.with(|n| n.set(n.get() + 1));
    Ok(())
}

fn backend() -> UpdateBackend {
    UpdateBackend {
        check: newer,
        stage: staged,
        install: installed,
        relaunch: relaunched,
    }
}

fn counts() -> (usize, usize) {
    (INSTALLS.with(Cell::get), RELAUNCHES.with(Cell::get))
}

/// An app with a library, the given backend, and About open. The library
/// folder is removed when the guard drops, pass or fail.
fn about_with(
    name: &str,
    backend: UpdateBackend,
) -> (
    Entity<LuminaApp>,
    &'static mut VisualTestContext,
    TempLibrary,
) {
    INSTALLS.with(|n| n.set(0));
    RELAUNCHES.with(|n| n.set(0));
    let (dir, engine) = seed_library(name, 1);
    let cx = Box::leak(Box::new(crate::tests::TestApp::single()));
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.updates.backend = backend;
        app.open_about(cx);
    });
    render_ui(vcx);
    (view, vcx, dir)
}

/// ⌘K, `query`, ↵ (from the About page: Esc closes settings first).
fn run_command(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, query: &str) {
    focus_app(vcx, view);
    keystrokes(vcx, "escape secondary-k");
    type_text(vcx, query);
    keystrokes(vcx, "enter");
}

fn phase(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> UpdatePhase {
    view.update(vcx, |app, _| app.updates.phase.clone())
}

fn status_shows(vcx: &mut VisualTestContext, text: &str) -> bool {
    marker(vcx, format!("preferences-update-status-text:{text}"))
}

#[core::prelude::v1::test]
fn check_downloads_the_update_and_restart_installs_and_relaunches() {
    let (view, vcx, _dir) = about_with("update_restart", backend());
    click(vcx, "preferences-check-updates");
    assert_eq!(
        phase(&view, vcx),
        UpdatePhase::Ready(StagedUpdate::for_tests("99.0.0")),
        "a newer release is downloaded and waits to install"
    );
    assert!(
        status_shows(
            vcx,
            "Lumina 99.0.0 is ready. It installs when you restart or quit."
        ),
        "About says the update is ready"
    );
    assert!(
        marker(vcx, "toast-action-text:Restart".into()),
        "a toast offers Restart"
    );
    assert_eq!(
        counts(),
        (0, 0),
        "nothing installs before the user restarts or quits"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.config.last_update_check_secs.is_some(),
            "a successful check is remembered for the daily check"
        )
    });

    click(vcx, "preferences-restart-to-update");
    assert_eq!(
        counts(),
        (1, 1),
        "Restart installs once and relaunches once"
    );
    view.update(vcx, |app, _| {
        assert!(app.downloads.quit_requested, "Restart quits the app");
        assert_eq!(app.updates.phase, UpdatePhase::Idle);
    });
}

#[core::prelude::v1::test]
fn later_installs_at_the_next_quit_without_relaunching() {
    let (view, vcx, _dir) = about_with("update_later", backend());
    click(vcx, "preferences-check-updates");
    assert!(matches!(phase(&view, vcx), UpdatePhase::Ready(_)));
    focus_app(vcx, &view);
    vcx.dispatch_action(crate::actions::QuitApp);
    vcx.run_until_parked();
    assert_eq!(
        counts(),
        (1, 0),
        "a plain quit installs the waiting update but doesn't reopen"
    );
}

#[core::prelude::v1::test]
fn cancelled_restart_does_not_reopen_on_a_later_quit() {
    let (view, vcx, _dir) = about_with("update_cancel_restart", backend());
    click(vcx, "preferences-check-updates");
    // Restart to Update met the running-downloads prompt, and the user chose Cancel.
    view.update(vcx, |app, cx| {
        app.updates.relaunch = true;
        app.dismiss_downloads_prompt(cx);
    });
    focus_app(vcx, &view);
    vcx.dispatch_action(crate::actions::QuitApp);
    vcx.run_until_parked();
    assert_eq!(
        counts(),
        (1, 0),
        "after a cancelled restart, a later quit installs but does not reopen"
    );
}

#[core::prelude::v1::test]
fn any_quit_installs_the_waiting_update() {
    let (view, vcx, _dir) = about_with("update_app_quit", backend());
    click(vcx, "preferences-check-updates");
    assert!(matches!(phase(&view, vcx), UpdatePhase::Ready(_)));
    // Not through ⌘Q: the Dock, logout and shutdown reach GPUI's shutdown
    // (applicationWillTerminate on macOS) without the Quit action.
    // App-level, not through the window: shutdown closes it.
    vcx.cx.update(|cx| cx.shutdown());
    assert_eq!(
        counts(),
        (1, 0),
        "a quit that bypasses the Quit action still installs the update"
    );
}

#[core::prelude::v1::test]
fn check_and_restart_from_the_palette() {
    let (view, vcx, _dir) = about_with("update_keys", backend());
    run_command(&view, vcx, "check for upd");
    assert!(
        matches!(phase(&view, vcx), UpdatePhase::Ready(_)),
        "⌘K Check for Updates finds and downloads the update"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "⌘K Check for Updates shows the answer on About"
        )
    });
    run_command(&view, vcx, "restart to");
    assert_eq!(
        counts(),
        (1, 1),
        "⌘K Restart to Update installs and relaunches"
    );
}

fn no_update(_: &AtomicBool) -> Result<Release> {
    Ok(release(env!("CARGO_PKG_VERSION")))
}

fn offline(_: &AtomicBool) -> Result<Release> {
    Err(anyhow!(lumina_core::update::CHECK_FAILED))
}

fn unsigned(_: &Release, _: &AtomicBool) -> Result<StagedUpdate> {
    Err(anyhow!(lumina_core::update::install::UNSIGNED))
}

#[core::prelude::v1::test]
fn check_reports_up_to_date_offline_and_uninstallable() {
    let (view, vcx, _dir) = about_with(
        "update_outcomes",
        UpdateBackend {
            check: no_update,
            ..backend()
        },
    );
    click(vcx, "preferences-check-updates");
    assert!(
        status_shows(vcx, "Lumina is up to date."),
        "the same version is not an update"
    );

    view.update(vcx, |app, _| app.updates.backend.check = offline);
    click(vcx, "preferences-check-updates");
    assert!(
        status_shows(vcx, lumina_core::update::CHECK_FAILED),
        "a failed check says so in plain words"
    );

    view.update(vcx, |app, _| {
        app.updates.backend.check = newer;
        app.updates.backend.stage = unsigned;
    });
    click(vcx, "preferences-check-updates");
    assert!(status_shows(vcx, lumina_core::update::install::UNSIGNED));
    assert!(
        vcx.debug_bounds("preferences-release-page").is_some(),
        "an update this copy can't install offers the release page instead"
    );
    assert_eq!(counts(), (0, 0));
}

#[core::prelude::v1::test]
fn daily_check_is_opt_in_and_quiet_when_offline() {
    let (view, vcx, _dir) = about_with(
        "update_daily",
        UpdateBackend {
            check: offline,
            ..backend()
        },
    );
    view.update(vcx, |app, cx| {
        assert!(
            !app.config.auto_update_check,
            "the daily check is off by default"
        );
        app.auto_check_for_updates(cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        phase(&view, vcx),
        UpdatePhase::Idle,
        "off means no check at launch"
    );

    click(vcx, "preferences-auto-updates");
    view.update(vcx, |app, cx| {
        assert!(app.config.auto_update_check, "Check Daily turns it on");
        app.auto_check_for_updates(cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        phase(&view, vcx),
        UpdatePhase::Idle,
        "a failed daily check stays quiet (it isn't something the user asked for just now)"
    );
    view.update(vcx, |app, _| {
        assert!(app.toast.current().is_none());
        assert!(
            app.config.last_update_check_secs.is_some(),
            "a failed attempt counts too, so offline launches don't retry every time"
        );
    });

    // That attempt was just now: no second check today, even with an update out.
    view.update(vcx, |app, cx| {
        app.updates.backend.check = newer;
        app.auto_check_for_updates(cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        phase(&view, vcx),
        UpdatePhase::Idle,
        "at most one check a day"
    );

    // A day later the daily check runs, downloads, and offers Restart.
    view.update(vcx, |app, cx| {
        app.config.last_update_check_secs =
            app.config.last_update_check_secs.map(|t| t - 24 * 60 * 60);
        app.auto_check_for_updates(cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        matches!(phase(&view, vcx), UpdatePhase::Ready(_)),
        "once a day has passed, the daily check finds and downloads the update"
    );
    assert!(
        marker(vcx, "toast-action-text:Restart".into()),
        "a ready update from the daily check offers Restart"
    );

    // Turning it off again, by keyboard.
    run_command(&view, vcx, "stop checking");
    view.update(vcx, |app, _| assert!(!app.config.auto_update_check));
}

#[core::prelude::v1::test]
fn check_daily_reverts_when_the_save_fails() {
    let (view, vcx, _dir) = about_with("update_daily_save", backend());
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "preferences-auto-updates");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert!(
            !app.config.auto_update_check,
            "a failed save puts the old choice back"
        );
        assert!(app.error_message.is_some(), "and the banner says why");
    });
}

fn broken(_: &StagedUpdate) -> Result<()> {
    Err(anyhow!(lumina_core::update::install::INSTALL_FAILED))
}

#[core::prelude::v1::test]
fn failed_install_on_restart_keeps_lumina_open() {
    let (view, vcx, _dir) = about_with(
        "update_install_fails",
        UpdateBackend {
            install: broken,
            ..backend()
        },
    );
    click(vcx, "preferences-check-updates");
    click(vcx, "preferences-restart-to-update");
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.quit_requested,
            "a failed install doesn't quit"
        );
        assert_eq!(
            app.error_message.as_deref(),
            Some(lumina_core::update::install::INSTALL_FAILED),
            "the banner says the current version is unchanged"
        );
    });
    assert_eq!(counts(), (0, 0), "no relaunch after a failed install");
}

#[core::prelude::v1::test]
fn about_tab_walk_reaches_check_and_restart() {
    let (view, vcx, _dir) = about_with("update_tab_walk", backend());
    focus_app(vcx, &view);
    keystrokes(vcx, "tab tab tab");
    let focus = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.settings_focus.map(|b| b.id()))
    };
    assert_eq!(
        focus(&view, vcx),
        Some("preferences-check-updates"),
        "Tab walks License, Acknowledgements, then Check for Updates"
    );
    keystrokes(vcx, "enter");
    assert!(matches!(phase(&view, vcx), UpdatePhase::Ready(_)));
    assert_eq!(
        focus(&view, vcx),
        Some("preferences-restart-to-update"),
        "the focus follows the button as it becomes Restart to Update"
    );
    keystrokes(vcx, "tab space");
    view.update(vcx, |app, _| {
        assert!(
            app.config.auto_update_check,
            "Space on Check Daily turns it on"
        )
    });
    keystrokes(vcx, "shift-tab enter");
    assert_eq!(
        counts(),
        (1, 1),
        "↵ on Restart to Update installs and relaunches"
    );
}
