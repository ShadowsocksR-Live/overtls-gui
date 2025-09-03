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

pub fn load_icon<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<tray_icon::Icon> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .map_err(|e| std::io::Error::other(format!("Failed to open icon path {e}")))?
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .map_err(|e| std::io::Error::other(format!("Failed to create tray icon: {e}")))
}

/// Get the path to the application icon (assets/main.png) relative to the executable.
pub fn get_main_icon_path() -> std::io::Result<PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| std::io::Error::other("Failed to get executable directory"))?;
    let icon_path = exe_dir.join("assets").join("main.png");
    if icon_path.exists() {
        Ok(icon_path)
    } else {
        Err(std::io::Error::other(format!("Icon file not found at {icon_path:?}")))
    }
}

pub fn set_window_icon<P: AsRef<std::path::Path>>(window: &mut fltk::window::Window, icon_path: P) -> std::io::Result<()> {
    let mut f = std::fs::File::open(icon_path.as_ref())?;
    use std::io::Read;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let png = fltk::image::PngImage::from_data(&buf).map_err(|e| std::io::Error::other(format!("Failed to load icon data: {e}")))?;
    use fltk::prelude::WindowExt;
    window.set_icon(Some(png));
    Ok(())
}
