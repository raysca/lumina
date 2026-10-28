//! Minimal file logger + panic hook.
//!
//! Logs go to the platform data directory (Lumina/logs/lumina.log) and to
//! stderr. The file is rotated once it exceeds 5 MB. Logging is initialized
//! once at process start; library code uses the standard log macros.

use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, Once};

struct FileLogger {
    file: Mutex<Option<File>>,
    level: LevelFilter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let line = format!(
            "{} [{:<5}] {}: {}
",
            timestamp,
            record.level(),
            record.target(),
            record.args()
        );
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

/// Path of the active log file (useful for diagnostics).
pub fn log_file_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lumina")
        .join("logs")
        .join("lumina.log")
}

/// Installs the logger and a panic hook. Safe to call more than once.
pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let level = std::env::var("LUMINA_LOG")
            .ok()
            .and_then(|value| value.parse::<LevelFilter>().ok())
            .unwrap_or(LevelFilter::Info);

        let path = log_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.len() > 5 * 1024 * 1024 {
                let rotated = path.with_extension("log.1");
                let _ = fs::remove_file(&rotated);
                let _ = fs::rename(&path, &rotated);
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();

        let logger = FileLogger {
            file: Mutex::new(file),
            level,
        };
        log::set_max_level(level);
        let _ = log::set_boxed_logger(Box::new(logger));

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("panic: {info}");
            previous_hook(info);
        }));

        log::info!(
            "lumina starting (version {}, log file {})",
            env!("CARGO_PKG_VERSION"),
            path.display()
        );
    });
}
