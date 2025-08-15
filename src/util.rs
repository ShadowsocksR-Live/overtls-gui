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

pub fn file_chooser_open_file(x: i32, y: i32, w: i32, h: i32, title: &str, filter: &str, default_path: Option<&str>) -> Option<String> {
    let mut chooser = fltk::dialog::FileChooser::new(
        default_path.unwrap_or("."),          // directory
        filter,                               // filter or pattern
        fltk::dialog::FileChooserType::Multi, // chooser type
        title,                                // title
    );
    chooser.show();

    use fltk::prelude::WidgetExt;
    chooser.window().set_pos(x, y);
    chooser.window().set_size(w, h);

    // Block until user picks something. (The other way to do this is to use a callback())
    while chooser.shown() {
        fltk::app::wait();
    }
    // User hit cancel?
    if chooser.value(1).is_none() {
        log::trace!("(User hit 'Cancel')");
        return None;
    }
    // Print what the user picked
    log::trace!("--------------------");
    log::trace!("DIRECTORY: '{}'", chooser.directory().unwrap());
    log::trace!("    VALUE: '{}'", chooser.value(1).unwrap()); // value starts at 1!
    log::trace!("    COUNT: {} files selected", chooser.count());
    // Multiple files? Show all of them
    if chooser.count() > 1 {
        for t in 1..=chooser.count() {
            log::trace!(" VALUE[{}]: '{}'", t, chooser.value(t).unwrap());
        }
    }

    Some(chooser.value(1).unwrap())
}
