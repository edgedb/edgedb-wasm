use crate::worker;

wit_bindgen_wasmtime::export!("./wit/log_v1.wit");

pub use log_v1::add_to_linker;


impl Into<log::Level> for log_v1::Level {
    fn into(self) -> log::Level {
        use log::Level as T;
        use log_v1::Level as S;

        match self {
            S::Error => T::Error,
            S::Warn => T::Warn,
            S::Debug => T::Debug,
            S::Info => T::Info,
            S::Trace => T::Trace,
        }
    }
}

fn convert_level(value: log::LevelFilter) -> Option<log_v1::Level> {
    use log::LevelFilter as S;
    use log_v1::Level as T;

    match value {
        S::Off => None,
        S::Error => Some(T::Error),
        S::Warn => Some(T::Warn),
        S::Debug => Some(T::Debug),
        S::Info => Some(T::Info),
        S::Trace => Some(T::Trace),
    }
}

impl log_v1::LogV1 for worker::State {
    fn log(&mut self, value: log_v1::LogRecord) {
        let target =
            format!("wasm::{}::{}::{}", self.tenant, self.worker, value.target);
        let meta = log::MetadataBuilder::new()
            .target(&target)
            .level(value.level.into())
            .build();
        log::logger().log(&log::Record::builder()
            .metadata(meta)
            .args(format_args!("{}", value.message))
            .line(value.line)
            .file(value.file)
            .module_path(value.module_path)
            .build());
    }
    fn max_level(&mut self) -> Option<log_v1::Level> {
        convert_level(log::max_level())
    }
}
