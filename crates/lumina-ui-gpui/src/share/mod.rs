//! Share sheet (macOS only; `main.rs` compiles this module out elsewhere).
//!
//! Hands the selected originals to `NSSharingServicePicker`
//! (`macos_share.m`). Lumina makes no network call: the services that send
//! the files are the system's. Tests record requests in [`SHARE_REQUESTS`]
//! instead of opening the real picker.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use gpui::{Context, Pixels, Point};

use lumina_core::MediaId;

use crate::app::LuminaApp;

#[cfg(not(test))]
extern "C" {
    /// Shows the picker for `count` NUL-terminated paths at `(x, y)` in
    /// window points (top-left origin); NaN means the window centre. Main
    /// thread only. Returns false when there was no window to show it in.
    fn lumina_share_files(
        paths: *const *const std::ffi::c_char,
        count: i32,
        x: f64,
        y: f64,
    ) -> bool;
}

/// Most items one share may hold.
pub(crate) const SHARE_MAX: usize = 500;

impl LuminaApp {
    /// A dialog, edit mode, compare, a slideshow or Similar shots is up: the
    /// selection is hidden or about to change, so nothing may be shared from
    /// under it.
    fn share_blocked(&self) -> bool {
        self.import_blocked_by_modal()
            || self.edits.session.is_some()
            || self.slideshow.is_some()
            || (self.compare.is_some() && !self.is_fullscreen_viewer)
            || self.similar_shots_shown()
    }

    /// Opens the system share picker for the selected originals, anchored at
    /// `anchor` (window points) or the window centre. The file checks run
    /// off the UI thread (a sleeping drive must not freeze it); missing
    /// files are left out, and when every one is missing a toast says so
    /// instead. A result for a library or selection that has since changed
    /// is dropped.
    pub fn share_selection(&mut self, anchor: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        if self.share_blocked() {
            return;
        }
        // Counted before any row is read: ⌘A on 100k items must not fetch.
        let count = if self.selected_indices.is_empty() {
            usize::from(self.selected_item.is_some())
        } else {
            self.selected_indices.len()
        };
        if count == 0 {
            return;
        }
        if count > SHARE_MAX {
            let max = SHARE_MAX.to_string();
            self.show_toast(crate::i18n::t_with("share.too_many", &[&max]), cx);
            return;
        }
        // One catalog read of at most SHARE_MAX rows; only the disk checks
        // (a sleeping or offline drive) go off the UI thread.
        let items = self.get_selected_items();
        let ids: Vec<MediaId> = items.iter().map(|item| item.id).collect();
        let paths: Vec<PathBuf> = items.into_iter().map(|item| item.path).collect();
        let total = paths.len();
        let generation = self.library_generation;
        let check = cx
            .background_executor()
            .spawn(async move { paths.into_iter().filter(|p| p.exists()).collect::<Vec<_>>() });
        cx.spawn(async move |this, cx| {
            let present = check.await;
            // The app may be gone; nothing to share then.
            let _ = this.update(cx, |this, cx| {
                // Another library opened or the selection moved meanwhile:
                // the result is for something no longer selected.
                if this.library_generation != generation || this.get_selected_ids() != ids {
                    return;
                }
                // A dialog or presentation may have opened meanwhile.
                if this.share_blocked() {
                    return;
                }
                if present.is_empty() {
                    if total > 0 {
                        this.show_toast(missing_message(total), cx);
                    }
                    return;
                }
                this.present_share_picker(&present, anchor, cx);
            });
        })
        .detach();
    }

    /// Hands `paths` (all present on disk) to the picker, or records them
    /// under test.
    fn present_share_picker(
        &mut self,
        paths: &[PathBuf],
        anchor: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        let c_paths = c_paths(paths);
        #[cfg(test)]
        {
            let _ = cx;
            SHARE_REQUESTS.with(|requests| {
                let sent = c_paths
                    .iter()
                    .map(|c| PathBuf::from(std::ffi::OsStr::from_bytes(c.as_bytes())))
                    .collect();
                requests.borrow_mut().push((sent, anchor));
            });
        }
        #[cfg(not(test))]
        {
            if c_paths.is_empty() {
                return;
            }
            let pointers: Vec<*const std::ffi::c_char> =
                c_paths.iter().map(|c| c.as_ptr()).collect();
            let (x, y) = anchor.map_or((f64::NAN, f64::NAN), |p| {
                (f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
            });
            // At most SHARE_MAX entries, so the count fits.
            let count = i32::try_from(pointers.len()).unwrap_or(i32::MAX);
            // SAFETY: `c_paths` owns every string and outlives the call;
            // `count` never exceeds `pointers.len()`. This runs in an entity
            // update on the main thread, which the bridge requires.
            let shown = unsafe { lumina_share_files(pointers.as_ptr(), count, x, y) };
            if !shown {
                self.show_toast(crate::i18n::t("share.no_window"), cx);
            }
        }
    }
}

/// "The selected file is missing…" or "The N selected files are missing…".
fn missing_message(count: usize) -> String {
    if count == 1 {
        crate::i18n::t("share.missing_one")
    } else {
        crate::i18n::t_with("share.missing_many", &[&count.to_string()])
    }
}

/// NUL-terminated copies of `paths`. A path with an interior NUL cannot be
/// passed to AppKit, so it is skipped and logged rather than panicking.
fn c_paths(paths: &[PathBuf]) -> Vec<CString> {
    paths
        .iter()
        .filter_map(|path| {
            CString::new(path.as_os_str().as_bytes())
                .inspect_err(|_| log::warn!("share: skipping a path with a NUL byte: {path:?}"))
                .ok()
        })
        .collect()
}

/// One recorded request: the paths sent and the anchor (None: window centre).
#[cfg(test)]
pub(crate) type ShareRequest = (Vec<PathBuf>, Option<Point<Pixels>>);

#[cfg(test)]
thread_local! {
    /// Share requests on this test thread: the paths sent and the anchor.
    pub(crate) static SHARE_REQUESTS: std::cell::RefCell<Vec<ShareRequest>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn interior_nul_path_is_skipped_not_fatal() {
        let good = PathBuf::from("/tmp/a.jpg");
        let bad = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/b\0.jpg"));
        let out = c_paths(&[bad, good]);
        assert_eq!(
            out,
            vec![CString::new("/tmp/a.jpg").unwrap()],
            "a NUL-bearing path is dropped and the rest still go"
        );
    }
}
