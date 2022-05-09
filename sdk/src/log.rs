//! Logging for WebAsssembly application
//!
//! This module is mostly re-export of the [`log`](mod@log) crate. The most
//! important exception is that logging is expected to be controlled from host.
//! So logger and max-level changes are allowed via this API.
//!
#![cfg_attr(feature="host", allow(dead_code))]

pub use log::{debug, error, info, log, log_enabled, trace, warn};
pub use log::{Record, RecordBuilder, Metadata, MetadataBuilder};
pub use log::{Level, LevelFilter, STATIC_MAX_LEVEL};
pub use log::{logger, max_level};

wit_bindgen_rust::import!("../wit/edgedb_log_v1.wit");

use edgedb_log_v1 as v1;

static mut LOGGER: HostLogger = HostLogger {
    max_level: log::STATIC_MAX_LEVEL,
};

struct HostLogger {
    max_level: log::LevelFilter,
}

impl From<log::Level> for v1::Level {
    fn from(value: log::Level) -> v1::Level {
        use v1::Level as T;
        use log::Level as S;

        match value {
            S::Error => T::Error,
            S::Warn => T::Warn,
            S::Debug => T::Debug,
            S::Info => T::Info,
            S::Trace => T::Trace,
        }
    }
}

fn convert_filter(value: Option<v1::Level>) -> log::LevelFilter {
    use log::LevelFilter as T;
    use v1::Level as S;

    match value {
        None => T::Off,
        Some(S::Error) => T::Error,
        Some(S::Warn) => T::Warn,
        Some(S::Debug) => T::Debug,
        Some(S::Info) => T::Info,
        Some(S::Trace) => T::Trace,
    }
}

#[cfg(not(feature="host"))]
pub(crate) fn init() {
    let level = convert_filter(v1::max_level());
    unsafe {
        // not sure if safe all all platforms
        LOGGER.max_level = level;

        log::set_logger(&LOGGER).expect("init_logging");
    }
    log::set_max_level(level);
}

impl log::Log for HostLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            v1::log(v1::LogRecord {
                target: record.target(),
                level: record.level().into(),
                message: &record.args().to_string(),
                line: record.line(),
                file: record.file(),
                module_path: record.module_path(),
            });
        }
    }

    fn flush(&self) {}
}
