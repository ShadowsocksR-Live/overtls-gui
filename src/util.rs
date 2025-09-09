use std::path::PathBuf;

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

// ===============================================================================================

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

// ===============================================================================================

pub fn file_chooser_open_file(title: &str, default_path: Option<&str>, filter: &str, filter_exts: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_directory(default_path.unwrap_or("."))
        .add_filter(filter, filter_exts)
        .pick_file()
}

pub fn file_chooser_save_file(title: &str, default_path: Option<&str>, filter: &str, filter_exts: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_directory(default_path.unwrap_or("."))
        .add_filter(filter, filter_exts)
        .save_file()
}

pub fn load_icon_from_bytes(bytes: &[u8]) -> std::io::Result<tray_icon::Icon> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| std::io::Error::other(format!("Failed to load icon from memory: {e}")))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    tray_icon::Icon::from_rgba(rgba, width, height).map_err(|e| std::io::Error::other(format!("Failed to create tray icon: {e}")))
}

pub const MAIN_ICON_BYTES: &[u8] = include_bytes!("../assets/main.png");

pub fn get_embedded_main_icon() -> std::io::Result<fltk::image::PngImage> {
    fltk::image::PngImage::from_data(MAIN_ICON_BYTES).map_err(|e| std::io::Error::other(format!("Failed to load embedded icon: {e}")))
}
