#[allow(dead_code)]
pub const fn host_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "Unknown OS"
    }
}

#[derive(Debug)]
pub enum ThreadJoinError {
    Timeout,
    Panic(Box<dyn std::any::Any + Send + 'static>),
}

impl std::fmt::Display for ThreadJoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadJoinError::Timeout => write!(f, "Thread join timed out"),
            ThreadJoinError::Panic(p) => {
                if let Some(s) = p.downcast_ref::<String>() {
                    write!(f, "Thread panicked: {s}")
                } else if let Some(s) = p.downcast_ref::<&str>() {
                    write!(f, "Thread panicked: {s}")
                } else if let Some(s) = p.downcast_ref::<std::io::Error>() {
                    write!(f, "Thread panicked: {s}")
                } else {
                    write!(f, "Thread panicked with unknown type")
                }
            }
        }
    }
}

impl std::error::Error for ThreadJoinError {}

pub fn thread_handle_join_with_timeout<T>(handle: std::thread::JoinHandle<T>, timeout_ms: u64) -> Result<T, ThreadJoinError> {
    let start = std::time::Instant::now();
    loop {
        if handle.is_finished() {
            return handle.join().map_err(ThreadJoinError::Panic);
        }
        if start.elapsed() > std::time::Duration::from_millis(timeout_ms) {
            return Err(ThreadJoinError::Timeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
