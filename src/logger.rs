use std::collections::HashMap;

pub type LogSender = std::sync::mpsc::Sender<(log::Level, String, String)>;

/// A simple logger that sends log messages through a channel.
/// The messages are tuples of (level, module_path, message).
#[derive(Debug, Clone)]
pub struct Logger {
    pub sender: LogSender,
    /// Module-specific log level filters
    /// Key: module name (can be partial), Value: maximum allowed log level
    pub module_filters: HashMap<String, log::LevelFilter>,
    /// Default log level for modules not in the filter list
    pub default_level: log::LevelFilter,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let log_level = metadata.level();
        let target = metadata.target();
        let root_module = target.split("::").next().unwrap_or(target);
        if let Some(max_level) = self.module_filters.get(root_module) {
            return log_level <= *max_level;
        }
        log_level <= self.default_level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let _ = self.sender.send((
                record.level(),
                record.module_path().unwrap_or_default().to_string(),
                record.args().to_string(),
            ));
        }
    }

    fn flush(&self) {}
}

impl Logger {
    /// Adds or updates a module filter
    #[allow(dead_code)]
    pub fn add_module_filter(&mut self, module: String, level: log::LevelFilter) {
        self.module_filters.insert(module, level);
    }

    /// Removes a module filter
    #[allow(dead_code)]
    pub fn remove_module_filter(&mut self, module: &str) {
        self.module_filters.remove(module);
    }

    /// Sets the default log level
    #[allow(dead_code)]
    pub fn set_default_level(&mut self, level: log::LevelFilter) {
        self.default_level = level;
    }
}
