// SPDX-License-Identifier: Apache-2.0
//! Local file logging + diagnostics. **No telemetry** — everything stays on disk.
//! Logs go to a single size-capped file under the user's data dir (rotated to
//! `voz.log.old` past 5 MB). Once a `log` logger is installed, whisper.cpp/ggml's
//! own messages (routed via `whisper_rs::install_logging_hooks` in the transcriber)
//! land here too. A panic hook records worker-thread panics instead of losing them.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use voz_core::Settings;

const MAX_BYTES: u64 = 5 * 1024 * 1024;

struct FileLogger {
    file: Mutex<File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.level() <= self.level
    }
    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(
                f,
                "[{:<5}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }
    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

/// Path to the current log file (`$XDG_DATA_HOME/voz/voz.log`).
#[must_use]
pub fn log_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("voz")
        .join("voz.log")
}

/// Install the file logger + panic hook. Idempotent-ish (a second call is a no-op
/// because `set_boxed_logger` fails once one is set).
pub fn init() {
    let path = log_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    // Rotate if the log got large.
    if std::fs::metadata(&path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
        let logger = Box::new(FileLogger {
            file: Mutex::new(file),
            level: log::LevelFilter::Info,
        });
        if log::set_boxed_logger(logger).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    }
    // Record panics (e.g. a worker thread) rather than losing them silently.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default(info);
    }));
    log::info!(
        "voz {} starting on {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    );
}

/// A redacted diagnostics blob safe to share for support. Deliberately omits the
/// save-folder path and any transcript content — only versions, environment, and
/// non-sensitive settings.
#[must_use]
pub fn diagnostics(settings: &Settings) -> String {
    let gpu = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,driver_version", "--format=csv,noheader"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "none detected".into());
    let gpu_build = if cfg!(feature = "cuda") {
        "cuda"
    } else if cfg!(feature = "vulkan") {
        "vulkan"
    } else {
        "cpu"
    };
    format!(
        "Voz {ver}\n\
         OS: {os} ({arch})\n\
         GPU: {gpu}\n\
         Build backend: {gpu_build}\n\
         Acceleration: {accel:?}\n\
         Model: {model}\n\
         Refine backend: {backend:?}\n\
         Default source: {source:?}\n\
         Log: {log}\n",
        ver = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        accel = settings.transcription.accel,
        model = settings.transcription.model,
        backend = settings.refine.backend,
        source = settings.sources.default_source,
        log = log_path().display(),
    )
}
