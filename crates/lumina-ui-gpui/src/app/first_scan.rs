//! "Building your library" (v3): progress for the first scan of an empty
//! catalog. Rescans of a populated library keep the status-bar count.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::Context;
use lumina_core::{MediaId, MediaItem, MediaType};

use crate::app::LuminaApp;

/// Thumbnails in the JUST ADDED strip.
pub const JUST_ADDED: usize = 8;

#[derive(Debug, Clone)]
pub struct FirstScanState {
    pub started: Instant,
    /// When the file-reading pass began (after counting finished), so ETA
    /// reflects reading rate rather than the pre-count walk.
    pub reading_started: Option<Instant>,
    /// Media files counted so far during the pre-count walk (`total == 0`).
    pub counting_found: usize,
    /// Media files counted before the walk; 0 until the count arrives.
    pub total: usize,
    /// Files the walk has reached (read or skipped).
    pub processed: usize,
    /// File walk finished; waiting for catalog finalization (`ScanComplete`).
    pub finishing: bool,
    pub current_file: Option<PathBuf>,
    pub photos: usize,
    pub videos: usize,
    pub raw: usize,
    pub skipped: usize,
    /// Newest first.
    pub just_added: Vec<MediaId>,
}

impl FirstScanState {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            reading_started: None,
            counting_found: 0,
            total: 0,
            processed: 0,
            finishing: false,
            current_file: None,
            photos: 0,
            videos: 0,
            raw: 0,
            skipped: 0,
            just_added: Vec::new(),
        }
    }

    pub fn add_items(&mut self, items: &[MediaItem]) {
        for item in items {
            match item.media_type {
                MediaType::Video => self.videos += 1,
                MediaType::Raw => self.raw += 1,
                _ => self.photos += 1,
            }
        }
        self.just_added
            .splice(0..0, items.iter().rev().take(JUST_ADDED).map(|i| i.id));
        self.just_added.truncate(JUST_ADDED);
    }

    /// 0.0–1.0 once the total is known.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.processed as f32 / self.total as f32).min(1.0)
        }
    }

    /// Linear estimate from the rate so far; None until there is a rate.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        if self.processed == 0 || self.total <= self.processed {
            return None;
        }
        let anchor = self.reading_started.unwrap_or(self.started);
        let elapsed = now.duration_since(anchor).as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }
        let per_file = elapsed / self.processed as f64;
        Some(Duration::from_secs_f64(
            per_file * (self.total - self.processed) as f64,
        ))
    }

    /// "120 of 480 · about 12 s left"
    pub fn progress_line(&self, now: Instant) -> String {
        if self.finishing {
            return "Finishing catalog…".to_string();
        }
        if self.total == 0 {
            return if self.counting_found > 0 {
                format!(
                    "Counting files… {} found",
                    crate::i18n::count(self.counting_found)
                )
            } else {
                "Counting files…".to_string()
            };
        }
        let mut line = format!(
            "{} of {}",
            crate::i18n::count(self.processed),
            crate::i18n::count(self.total)
        );
        if let Some(left) = self.remaining(now) {
            let secs = left.as_secs().max(1);
            line.push_str(&if secs < 90 {
                format!(" · about {secs} s left")
            } else {
                format!(" · about {} min left", secs.div_ceil(60))
            });
        }
        line
    }
}

impl Default for FirstScanState {
    fn default() -> Self {
        Self::new()
    }
}

impl LuminaApp {
    /// Use Pictures Folder (welcome screen).
    pub fn open_pictures_library(&mut self, cx: &mut Context<Self>) {
        // Tests count the request: opening the real Pictures folder would
        // write a catalog into the user's home (AGENTS §2).
        #[cfg(test)]
        {
            PICTURE_OPENS.with(|n| n.set(n.get() + 1));
            let _ = cx;
        }
        #[cfg(not(test))]
        {
            let pics_dir = dirs::picture_dir()
                .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
                .unwrap_or_else(|| PathBuf::from("."));
            self.open_library(pics_dir, cx);
        }
    }

    /// Open Library: show the grid while the scan carries on.
    pub fn dismiss_first_scan(&mut self, cx: &mut Context<Self>) {
        self.first_scan = None;
        cx.notify();
    }

    /// Stop Scan: cancel and show what was found so far.
    pub fn stop_first_scan(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.grid_state.as_mut() {
            state.engine.cancel_scan();
            state.is_scanning = false;
            state.scan_total = 0;
            state.scan_counting = 0;
        }
        self.first_scan = None;
        self.show_toast("Scan stopped. Rescan (⌘R) to finish later.", cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn progress_line_counts_and_estimates() {
        let mut s = FirstScanState::new();
        assert_eq!(s.progress_line(s.started), "Counting files…");
        s.total = 100;
        s.processed = 25;
        let later = s.started + Duration::from_secs(10);
        assert_eq!(s.progress_line(later), "25 of 100 · about 30 s left");
        assert!((s.fraction() - 0.25).abs() < 1e-6);
        s.processed = 1;
        assert_eq!(
            s.progress_line(later),
            "1 of 100 · about 17 min left",
            "long waits read in minutes"
        );
    }
}

#[cfg(test)]
thread_local! {
    /// "Use Pictures Folder" opens asked for on this test thread.
    pub(crate) static PICTURE_OPENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
