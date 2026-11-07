//! Slideshow (plan 5 C2): plays the current view in the immersive viewer,
//! one slide per `slideshow_interval_secs`, from the selection (or the
//! start). The timer is a GPUI task, one per slide, so a pause, a step or
//! the end simply drops or replaces it. Videos show their poster (the
//! viewer never autoplays) and Space pauses the show, not the video.
//!
//! While it runs, its keys come before every viewer and global shortcut:
//! Space, ←/→ and Esc act, ⌘Q and ⌘, pass, and everything else is absorbed,
//! so no palette, dialog or mark key reaches the slide or the selection
//! hidden behind it.

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::selection_by_id::HeldSelection;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::theme;

/// How long a slide fades in (skipped under Reduce Motion).
const SLIDE_IN: Duration = Duration::from_millis(220);

/// A running slideshow; `None` on the app when there is none.
pub(crate) struct SlideshowState {
    pub(crate) paused: bool,
    interval: Duration,
    /// The pending advance; dropping it cancels it.
    task: Option<Task<()>>,
    /// Bumped per slide, so each one gets its own fade.
    slide: u64,
    /// The selection before the show, by id, put back by Esc and Back.
    previous: HeldSelection,
    /// The view's row count when the show started.
    count: usize,
    /// The window the slideshow made full screen, if it did.
    fullscreen: Option<AnyWindowHandle>,
}

impl LuminaApp {
    /// ⌥⌘S, View > Slideshow, ⌘K Slideshow.
    pub fn start_slideshow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.slideshow.is_some() || self.compare.is_some() || self.grid_state.is_none() {
            return;
        }
        let count = self.navigation_count();
        if count == 0 {
            self.show_toast(t("slideshow.empty"), cx);
            return;
        }
        // ponytail: reads the selected rows once (all of them after ⌘A on a
        // large view); a selection kept by id in core is the upgrade.
        let previous = self.hold_selection(&self.get_selected_ids(), 0);
        // PiP would take the slides' videos; the slideshow takes the screen.
        self.close_pip(cx);
        self.select_index(self.selected_index.filter(|&i| i < count).unwrap_or(0), cx);
        let Some(item) = self.selected_item.clone() else {
            return;
        };
        self.search_focused = false;
        self.open_viewer(item, cx);
        let fullscreen = (!window.is_fullscreen()).then(|| window.window_handle());
        self.enter_immersive_fullscreen(Some(window), cx);
        // Start clean: the HUD comes back on pointer movement.
        self.immersive_hud_visible = false;
        self.hud_idle = Default::default();
        self.slideshow = Some(SlideshowState {
            paused: false,
            interval: self.config.slideshow_interval(),
            task: None,
            slide: 0,
            previous,
            count,
            fullscreen,
        });
        self.schedule_slide(cx);
        cx.notify();
    }

    /// ⌘K Slideshow: the palette has no `Window` to make full screen, so
    /// this starts it on the next effect cycle in the window this app is
    /// the root of.
    pub(crate) fn start_slideshow_deferred(&mut self, cx: &mut Context<Self>) {
        let me = cx.entity_id();
        let Some(handle) = cx
            .windows()
            .into_iter()
            .filter_map(|window| window.downcast::<LuminaApp>())
            .find(|window| window.entity(cx).is_ok_and(|app| app.entity_id() == me))
        else {
            return;
        };
        cx.defer(move |cx| {
            // A window closed meanwhile has nothing to show.
            let _ = handle.update(cx, |app, window, cx| app.start_slideshow(window, cx));
        });
    }

    /// Esc, Back, or an emptied view: leaves the slideshow and the viewer
    /// and puts the selection back by id (cleared when none of it is left).
    pub(crate) fn end_slideshow(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.drop_slideshow(cx) else {
            return;
        };
        self.close_viewer(cx);
        let mut held = state.previous;
        let count = self.navigation_count();
        // The show writes nothing (its keys and menus are off), so only
        // another writer (watcher, scan) moved rows: one that added or
        // removed `k` rows moved the rest by at most `k`.
        // ponytail: the net count change is a lower bound. Rows both
        // inserted and removed in one show under-widen the window by up to
        // 2·min(inserts, removes), and an outside write that reorders rows
        // without changing the count (a sidecar rating under a Rating sort)
        // is not seen at all. Held items that fall outside the window then
        // drop out of the selection (a safe clear, never a wrong item); a
        // core position-of-id query is the upgrade.
        held.widen(count.abs_diff(state.count));
        if held.window(count).is_none() {
            self.clear_selection(cx);
            return;
        }
        self.reselect_held(held, cx);
        // `reselect_held` swaps the shown item only when its focus left the
        // view; here the shown item is still the last slide.
        let Some(item) = self.selected_index.and_then(|i| self.navigation_item(i)) else {
            self.clear_selection(cx);
            return;
        };
        if self.selected_item.as_ref().map(|shown| shown.id) != Some(item.id) {
            self.select_item_internal(item, cx);
        }
        self.refresh_selected_tags();
        self.schedule_session_save(cx);
    }

    /// Discards the slideshow without touching the selection (the viewer
    /// closed some other way, or the library is switching), and leaves the
    /// full screen it entered. Returns the ended state.
    pub(crate) fn drop_slideshow(&mut self, cx: &mut Context<Self>) -> Option<SlideshowState> {
        let mut state = self.slideshow.take()?;
        // Its window is left below (deferred), so none here.
        self.exit_immersive_fullscreen(None, cx);
        if let Some(handle) = state.fullscreen.take() {
            // Deferred: this may run inside that window's own update.
            cx.defer(move |cx| {
                // A closed window has no full screen to leave.
                let _ = handle.update(cx, |_, window, _| {
                    if window.is_fullscreen() {
                        window.toggle_fullscreen();
                    }
                });
            });
        }
        cx.notify();
        Some(state)
    }

    /// Arms the advance for the current slide, replacing any pending one.
    fn schedule_slide(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.slideshow.as_mut().filter(|s| !s.paused) else {
            return;
        };
        let interval = state.interval;
        state.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(interval).await;
            // The app may be gone; nothing to advance then.
            let _ = this.update(cx, |app, cx| app.advance_slide(cx));
        }));
    }

    /// The timer fired: the next slide, the first one with Loop on, or a
    /// stop on the last one.
    fn advance_slide(&mut self, cx: &mut Context<Self>) {
        if self.slideshow.is_none() {
            return;
        }
        let count = self.navigation_count();
        if !self.is_fullscreen_viewer || count == 0 {
            self.end_slideshow(cx);
            return;
        }
        let at = self.selected_index.unwrap_or(0);
        if at + 1 < count {
            self.show_slide(at + 1, cx);
        } else if self.config.slideshow_loop {
            self.show_slide(0, cx);
        } else if let Some(state) = self.slideshow.as_mut() {
            state.paused = true;
            state.task = None;
            cx.notify();
        }
    }

    fn show_slide(&mut self, index: usize, cx: &mut Context<Self>) {
        let delta = index as isize - self.selected_index.unwrap_or(0) as isize;
        self.navigate_selection(delta, cx);
        if let Some(state) = self.slideshow.as_mut() {
            state.slide += 1;
        }
        self.schedule_slide(cx);
        cx.notify();
    }

    fn toggle_slideshow_pause(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.slideshow.as_mut() else {
            return;
        };
        state.paused = !state.paused;
        state.task = None;
        self.schedule_slide(cx);
        cx.notify();
    }

    /// The slideshow's keys; `false` when none runs or the key passes on
    /// (⌘Q, and ⌘, which pauses the show under Preferences).
    pub(crate) fn handle_slideshow_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slideshow.is_none() {
            return false;
        }
        let m = &keystroke.modifiers;
        let key = keystroke.key.as_str();
        let cmd = m.secondary(cx) || m.control;
        let plain = !cmd && !m.alt && !m.shift && !m.function;
        match key {
            "q" if cmd => return false,
            "," if cmd => {
                if let Some(state) = self.slideshow.as_mut().filter(|s| !s.paused) {
                    state.paused = true;
                    state.task = None;
                }
                return false;
            }
            "escape" => self.end_slideshow(cx),
            "space" if plain => self.toggle_slideshow_pause(cx),
            "left" | "right" if plain => {
                let at = self.selected_index.unwrap_or(0);
                let next = if key == "left" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(self.navigation_count().saturating_sub(1))
                };
                if next != at {
                    self.show_slide(next, cx);
                }
            }
            _ => {}
        }
        true
    }

    /// ⌘K Slideshow Interval: 3, 5 or 10 seconds, used from the next start.
    pub(crate) fn set_slideshow_interval(&mut self, secs: u8, cx: &mut Context<Self>) {
        let old = std::mem::replace(&mut self.config.slideshow_interval_secs, secs);
        if self.save_config() {
            self.show_toast(t(interval_key(secs)), cx);
        } else {
            self.config.slideshow_interval_secs = old;
        }
        cx.notify();
    }

    /// ⌘K Slideshow: Loop.
    pub(crate) fn toggle_slideshow_loop(&mut self, cx: &mut Context<Self>) {
        self.config.slideshow_loop = !self.config.slideshow_loop;
        if self.save_config() {
            let key = if self.config.slideshow_loop {
                "slideshow.loop_on"
            } else {
                "slideshow.loop_off"
            };
            self.show_toast(t(key), cx);
        } else {
            self.config.slideshow_loop = !self.config.slideshow_loop;
        }
        cx.notify();
    }

    /// The viewer, framed for a running slideshow: each slide fades in
    /// (not under Reduce Motion) and a paused show says so.
    pub(crate) fn render_slideshow_stage(&self, viewer: AnyElement) -> AnyElement {
        let Some(state) = self.slideshow.as_ref() else {
            return viewer;
        };
        let slide = if crate::motion::reduce_motion() {
            viewer
        } else {
            crate::motion::fade_in(
                div()
                    .debug_selector(|| "slideshow-fade".to_string())
                    .absolute()
                    .inset_0()
                    .child(viewer),
                ElementId::NamedInteger("slideshow-slide".into(), state.slide),
                SLIDE_IN,
            )
            .into_any_element()
        };
        div()
            .debug_selector(|| "slideshow-view".to_string())
            .absolute()
            .inset_0()
            .bg(theme::bg_viewer())
            .child(slide)
            .when(state.paused, |stage| {
                stage.child(
                    div()
                        .absolute()
                        .bottom(px(32.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            theme::hud_panel("slideshow-paused")
                                .debug_selector(|| "slideshow-paused".to_string())
                                .px_3()
                                .py_1()
                                .text_sm()
                                .text_color(theme::text())
                                .child(t("slideshow.paused")),
                        ),
                )
            })
            .into_any_element()
    }
}

/// The toast key for an interval.
pub(crate) fn interval_key(secs: u8) -> &'static str {
    match secs {
        3 => "command.slideshow_interval_3",
        10 => "command.slideshow_interval_10",
        _ => "command.slideshow_interval_5",
    }
}
