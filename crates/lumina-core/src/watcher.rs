use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::{EngineEvent, Scanner};

pub struct LibraryWatcher {
    stop_flag: Arc<AtomicBool>,
}

impl LibraryWatcher {
    pub fn start<P: AsRef<Path>>(
        library_root: P,
        event_tx: Sender<EngineEvent>,
    ) -> Result<Self> {
        let path = library_root.as_ref().to_path_buf();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = stop_flag.clone();

        std::thread::Builder::new()
            .name("lumina-watcher".to_string())
            .spawn(move || {
                let tx = event_tx.clone();
                let scanner = Scanner::new(path.clone(), tx.clone());
                let mut debouncer = match new_debouncer(
                    Duration::from_millis(500),
                    move |res: DebounceEventResult| {
                        if let Ok(events) = res {
                            for event in events {
                                let p = &event.path;
                                if p.is_file() {
                                    if let Some(item) = scanner.inspect_media_file(p) {
                                        let _ = tx.send(EngineEvent::ItemAdded(item));
                                    }
                                }
                            }
                        }
                    },
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Failed to create notify debouncer: {:?}", e);
                        return;
                    }
                };

                if let Err(e) = debouncer
                    .watcher()
                    .watch(&path, RecursiveMode::Recursive)
                {
                    eprintln!("Failed to watch library root {:?}: {:?}", path, e);
                    return;
                }

                while !flag_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(250));
                }
            })
            .context("Failed to spawn library watcher thread")?;

        Ok(Self { stop_flag })
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
