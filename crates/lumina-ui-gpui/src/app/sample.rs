//! Explore a Sample Library (welcome screen, ⌥↵): generated gradient photos
//! in `<data dir>/lumina/Sample Library`, opened as an ordinary library.

use std::path::PathBuf;

use gpui::Context;

use crate::app::LuminaApp;
use crate::i18n::t;

impl LuminaApp {
    /// Explore a Sample Library (welcome screen).
    pub fn open_sample_library(&mut self, cx: &mut Context<Self>) {
        // Tests count the request: the real folder is in the user's
        // Application Support (AGENTS §2). They call `open_sample_library_at`.
        #[cfg(test)]
        {
            SAMPLE_OPENS.with(|n| n.set(n.get() + 1));
            let _ = cx;
        }
        #[cfg(not(test))]
        {
            let Some(data_dir) = dirs::data_dir() else {
                self.set_error(t("welcome.sample_no_folder"), cx);
                return;
            };
            self.open_sample_library_at(data_dir.join("lumina").join("Sample Library"), cx);
        }
    }

    /// Writes the sample photos into `dest` off the UI thread, then opens it.
    /// A second request while the photos are being written is ignored.
    pub(crate) fn open_sample_library_at(&mut self, dest: PathBuf, cx: &mut Context<Self>) {
        if self.welcome.generating_sample {
            return;
        }
        self.welcome.generating_sample = true;
        // Moves when another library open is asked for, even before that
        // open (in the background) has landed.
        let generation = self.welcome.retry_generation;
        let target = dest.clone();
        let task = cx
            .background_executor()
            .spawn(async move { lumina_core::sample::write_sample_library(&target) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // The app may be gone; then there is nothing to open.
            let _ = this.update(cx, |app, cx| {
                app.finish_sample_library(dest, generation, result, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_sample_library(
        &mut self,
        dest: PathBuf,
        generation: u64,
        result: anyhow::Result<usize>,
        cx: &mut Context<Self>,
    ) {
        self.welcome.generating_sample = false;
        cx.notify();
        match result {
            // The user opened another library meanwhile: keep it.
            Ok(_) if generation != self.welcome.retry_generation => {
                log::info!("sample library ready; another library was opened meanwhile");
            }
            Ok(_) => self.open_library(dest, cx),
            Err(err) => self.set_error(format!("{}: {err:#}", t("welcome.sample_failed")), cx),
        }
    }
}

#[cfg(test)]
thread_local! {
    /// "Explore a Sample Library" opens asked for on this test thread.
    pub(crate) static SAMPLE_OPENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
