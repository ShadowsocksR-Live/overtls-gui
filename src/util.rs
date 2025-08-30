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

pub fn thread_handle_join_with_timeout<T>(handle: std::thread::JoinHandle<T>, timeout_ms: u64) -> Option<T> {
    let start = std::time::Instant::now();
    loop {
        if handle.is_finished() {
            return handle.join().ok();
        }
        if start.elapsed() > std::time::Duration::from_millis(timeout_ms) {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

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
