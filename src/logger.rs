use std::sync::mpsc::Sender;

/// A simple logger that sends log messages through a channel.
/// The messages are tuples of (level, module_path, message).
pub struct Logger {
    pub sender: Sender<(log::Level, String, String)>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
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
    /// Creates a new Logger with the given sender.
    pub fn new(sender: Sender<(log::Level, String, String)>) -> Self {
        Logger { sender }
    }
}
