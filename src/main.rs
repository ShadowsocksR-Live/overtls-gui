// To remove the console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::{content_table::refresh_table, node_details_dialog::show_node_details};
use fltk::{
    enums::{Event, Shortcut},
    menu::{MenuBar, MenuFlag},
    prelude::{DisplayExt, GroupExt, MenuExt, WidgetBase, WidgetExt, WindowExt},
    window::Window,
};
use std::rc::Rc;
use std::{cell::RefCell, sync::mpsc::Receiver};

pub(crate) use overtls::Config as OverTlsNode;

pub(crate) type OverTlsNodeReceivers = std::sync::Arc<std::sync::Mutex<Vec<(Option<usize>, Receiver<Option<OverTlsNode>>)>>>;

mod content_table;
mod core;
mod logger;
mod node_details_dialog;
mod paste_operations;
mod qr_code_dialog;
mod settings_dialog;
mod states_manager;
mod util;

pub(crate) const MENUBAR_HEIGHT: i32 = 30;
pub(crate) const LOG_HEIGHT: i32 = 240;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // #[cfg(debug_assertions)]
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let (tx, rx) = std::sync::mpsc::channel();

    let state = states_manager::load_app_state();
    let system_settings = state.system_settings.clone().unwrap_or_default();

    if let Err(e) = log::set_boxed_logger(Box::new(system_settings.create_logger(tx))) {
        eprintln!("Failed to set logger: {e}");
    }
    // Note: No longer use log::set_max_level, as it is now controlled by the Logger internally
    log::set_max_level(log::LevelFilter::Trace);

    let tun2proxy_enable = system_settings.tun2proxy_enable.unwrap_or(false);
    let state = Rc::new(RefCell::new(state));

    if tun2proxy_enable && !run_as::is_elevated() {
        let status = core::restart_as_admin()?;
        std::process::exit(status.code().unwrap_or_default());
    }

    let remote_nodes = Rc::new(RefCell::new(state.borrow().remote_nodes.clone()));
    let current_node_index = Rc::new(RefCell::new(state.borrow().current_node_index));

    // Popup window event-driven queue
    let node_details_receivers: OverTlsNodeReceivers = Arc::new(Mutex::new(Vec::new()));

    let _app = ::fltk::app::App::default();

    let ws = state.borrow().window.clone();
    let title = format!("OverTLS clients manager for {}", util::host_os_name());
    let mut win = Window::new(ws.x, ws.y, ws.w, ws.h, title.as_str());

    let mut menubar = MenuBar::new(0, 0, ws.w, MENUBAR_HEIGHT, "");

    let mut table = content_table::create_table(&current_node_index, &remote_nodes, &win, node_details_receivers.clone());

    refresh_table(&mut table, &mut win, remote_nodes.borrow().len());

    let (settings_tx, settings_rx) = std::sync::mpsc::channel();
    let w = win.clone();
    let state_clone = state.clone();
    menubar.add("&Main/Settings", Shortcut::None, MenuFlag::MenuDivider, move |_m| {
        let settings = state_clone.borrow().system_settings.clone().unwrap_or_default();
        settings_dialog::show_settings_dialog(&w, &settings, settings_tx.clone());
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add(
        "&Main/Scan QR Code from screen\t",
        Shortcut::Ctrl | 'r',
        MenuFlag::Normal,
        move |_m| match paste_operations::screenshot_qr_import() {
            Ok(config) => {
                remote_nodes_clone.borrow_mut().push(config);
                refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
                rfd::MessageDialog::new()
                    .set_title("Success")
                    .set_description("QR Code scanned and imported successfully!")
                    .set_level(rfd::MessageLevel::Info)
                    .show();
            }
            Err(e) => {
                rfd::MessageDialog::new()
                    .set_title("Error")
                    .set_description(format!("Failed to import QR Code: {e}"))
                    .set_level(rfd::MessageLevel::Error)
                    .show();
            }
        },
    );

    let remote_nodes_clone = remote_nodes.clone();
    let state_clone = state.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Main/Import Node File\t", Shortcut::Ctrl | 'o', MenuFlag::Normal, move |_m| {
        let origin_path = state_clone
            .borrow()
            .current_selection_path
            .clone()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap()));
        // Show file chooser dialog and import config file
        let path = util::file_chooser_open_file("Select config file", origin_path.to_str(), "JSON File", &["json"]);
        if let Some(path) = &path {
            match OverTlsNode::from_config_file(path) {
                Ok(config) => {
                    if let Some(parent_dir) = std::path::Path::new(path).parent() {
                        state_clone.borrow_mut().set_current_path(parent_dir);
                    }
                    remote_nodes_clone.borrow_mut().push(config);
                    refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
                }
                Err(e) => {
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description(format!("Import failed: {e}"))
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                }
            }
        }
    });

    // let remote_nodes_clone = remote_nodes.clone();
    // let mut table_clone = table.clone();
    let w = win.clone();
    let node_details_receivers_clone = node_details_receivers.clone();
    menubar.add("&Main/New\t", Shortcut::Ctrl | 'n', MenuFlag::MenuDivider, move |_m| {
        let (tx, rx) = std::sync::mpsc::channel();
        show_node_details(&w, None, tx);
        node_details_receivers_clone.lock().unwrap().push((None, rx));
    });

    // --- Run/Stop menu actions ---
    use std::sync::{Arc, Mutex};

    // To control across threads
    let running_token: Arc<Mutex<Option<overtls::CancellationToken>>> = Arc::new(Mutex::new(None));
    let running_handle: Arc<Mutex<Option<std::thread::JoinHandle<std::io::Result<()>>>>> = Arc::new(Mutex::new(None));

    let current_node_index_run = current_node_index.clone();
    let remote_nodes_run = remote_nodes.clone();
    let running_token_run = running_token.clone();
    let running_handle_run = running_handle.clone();
    let state_clone = state.clone();
    menubar.add("&Main/Run", Shortcut::None, MenuFlag::Normal, move |_m| {
        let Some(idx) = *current_node_index_run.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Please select a node first.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let Some(mut config) = remote_nodes_run.borrow().get(idx).cloned() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        // Stop node first if it's running
        if running_token_run.lock().unwrap().is_some() {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("A node is already running. Please stop it first.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }

        let system_settings = state_clone.borrow().system_settings.clone().unwrap_or_default();
        let tun2proxy_enable = system_settings.tun2proxy_enable.unwrap_or_default();
        if tun2proxy_enable && !run_as::is_elevated() {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Requires admin privileges. Please restart the application as administrator.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }

        core::merge_system_settings_to_node_config(&system_settings, &mut config);

        if let Err(e) = config.check_correctness(false) {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description(format!("Configuration error: {e}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }

        let tun2proxy_args = core::cook_tun2proxy_config(&system_settings, &config);

        let title = config.remarks.clone().unwrap_or_default();
        let token = overtls::CancellationToken::new();
        *running_token_run.lock().unwrap() = Some(token.clone());
        let handle = std::thread::spawn(move || core::main_task_block(config, tun2proxy_args, token));
        *running_handle_run.lock().unwrap() = Some(handle);
        log::debug!("Node '{title}' is starting...");
    });

    let running_token_stop = running_token.clone();
    let running_handle_stop = running_handle.clone();
    menubar.add("&Main/Stop", Shortcut::None, MenuFlag::MenuDivider, move |_m| {
        if let Err(e) = stop_running_node(&running_token_stop, &running_handle_stop) {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description(format!("Failed to stop running node: {e}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
    });

    fn stop_running_node(
        running_token: &Arc<Mutex<Option<overtls::CancellationToken>>>,
        running_handle: &Arc<Mutex<Option<std::thread::JoinHandle<std::io::Result<()>>>>>,
    ) -> std::io::Result<()> {
        let mut err_info = None;
        let f1 = |e| std::io::Error::other(format!("running_token lock error: {e}"));
        if let Some(token) = running_token.lock().map_err(f1)?.take() {
            token.cancel();
        } else {
            err_info = Some("No running node.");
        }
        let f2 = |e| std::io::Error::other(format!("running_handle lock error: {e}"));
        if let Some(handle) = running_handle.lock().map_err(f2)?.take()
            && util::thread_handle_join_with_timeout(handle, 1000).is_none()
        {
            err_info = Some("Node thread did not finish in 1 second, force exit.");
        }
        err_info.map(|e| Err(std::io::Error::other(e))).unwrap_or(Ok(()))
    }

    menubar.add("&Main/Quit\t", Shortcut::Ctrl | 'q', MenuFlag::Normal, move |_| {
        ::fltk::app::quit();
    });

    // --- Node menu group: View Details ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let w = win.clone();
    let node_details_receivers_clone = node_details_receivers.clone();
    menubar.add("&Node/View Details", Shortcut::None, MenuFlag::Normal, move |_menu| {
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("No node selected.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let Some(cfg) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        show_node_details(&w, Some(cfg), tx);
        node_details_receivers_clone.lock().unwrap().push((Some(selected_row), rx));
    });

    // --- Node menu group: Export Node ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let state_clone = state.clone();
    menubar.add("&Node/Export Node", Shortcut::None, MenuFlag::Normal, move |_menu| {
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("No node selected.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let Some(cfg) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let origin_path = state_clone
            .borrow()
            .current_selection_path
            .clone()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap()));
        // export node to JSON file
        let save_path = util::file_chooser_save_file("Export Node as JSON", origin_path.to_str(), "JSON File", &["json"]);
        let Some(path) = save_path else {
            return;
        };
        match serde_json::to_string_pretty(&cfg) {
            Ok(json_str) => {
                if std::fs::write(&path, json_str).is_ok() {
                    log::debug!("Node exported to: {}", path.display());
                    state_clone.borrow_mut().set_current_path(path.parent().unwrap_or(&origin_path));
                } else {
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description("Failed to write node file.")
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                }
            }
            Err(e) => {
                rfd::MessageDialog::new()
                    .set_title("Error")
                    .set_description(format!("Failed to serialize node: {e}"))
                    .set_level(rfd::MessageLevel::Error)
                    .show();
            }
        }
    });

    // --- Node menu group: View QR Code ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let w = win.clone();
    menubar.add("&Node/Show QR Code", Shortcut::None, MenuFlag::MenuDivider, move |_menu| {
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("No node selected.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let Some(cfg) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        // Generate the SSR URL for the node and display it as a QR code
        if let Ok(ssr_url) = cfg.generate_ssr_url() {
            let name = cfg.remarks.clone().unwrap_or_default();
            let title = if name.is_empty() {
                "Node QR Code".to_string()
            } else {
                format!("Node QR Code - '{name}'")
            };
            if let Err(e) = qr_code_dialog::qr_code_dialog(&w, &title, &ssr_url) {
                rfd::MessageDialog::new()
                    .set_title("Error")
                    .set_description(format!("Failed to show QR Code: {e}"))
                    .set_level(rfd::MessageLevel::Error)
                    .show();
            }
        } else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Failed to generate URL for QR Code.")
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
    });

    // --- Node menu group: Delete ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Node/Delete", Shortcut::None, MenuFlag::MenuDivider, move |_menu| {
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("No node selected.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        if selected_row > remote_nodes_clone.borrow().len() {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }
        let title = remote_nodes_clone
            .borrow()
            .get(selected_row)
            .map(|c| c.remarks.clone().unwrap_or_default())
            .unwrap_or_default();
        let confirm = rfd::MessageDialog::new()
            .set_title("Confirm Deletion")
            .set_description(format!("Are you sure you want to delete node: '{title}'?"))
            .set_buttons(rfd::MessageButtons::OkCancel)
            .set_level(rfd::MessageLevel::Warning)
            .show();
        if confirm == rfd::MessageDialogResult::Ok {
            remote_nodes_clone.borrow_mut().remove(selected_row);
            *current_node_index_clone.borrow_mut() = None;
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        }
    });

    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    menubar.add("&Node/Copy\t", Shortcut::Ctrl | 'c', MenuFlag::Normal, move |_menu| {
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("No node selected.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        let Some(node) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Selected node not found.")
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        };
        if let Ok(text) = &node.generate_ssr_url() {
            ::fltk::app::copy(text);
            let name = node.remarks.clone().unwrap_or_default();
            rfd::MessageDialog::new()
                .set_title("Copied")
                .set_description(format!("Node '{name}'s URL copied to clipboard"))
                .set_level(rfd::MessageLevel::Info)
                .show();
        } else {
            rfd::MessageDialog::new()
                .set_title("Error")
                .set_description("Failed to generate URL for the selected node.")
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Node/Paste\t", Shortcut::Ctrl | 'v', MenuFlag::Normal, move |_menu| {
        if let Ok(config) = paste_operations::paste() {
            remote_nodes_clone.borrow_mut().push(config);
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        } else {
            rfd::MessageDialog::new()
                .set_title("Paste")
                .set_description("No valid configuration found in clipboard.")
                .set_level(rfd::MessageLevel::Warning)
                .show();
        }
    });

    menubar.add("&Help/About", Shortcut::None, MenuFlag::Normal, move |_| {
        let v = env!("CARGO_PKG_VERSION");
        use chrono::Datelike;
        let year = chrono::Utc::now().year();
        let d = format!("OverTLS GUI Client Manager\nVersion {v}\n\nA simple GUI manager for OverTLS clients.\n\n(c) {year} ssrlive");
        rfd::MessageDialog::new()
            .set_title("About")
            .set_description(d)
            .set_level(rfd::MessageLevel::Info)
            .show();
    });

    win.end();
    win.resizable(&table); // win.resizable(&win);

    win.set_callback(move |win| {
        win.iconize();
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    win.handle(move |_, ev| {
        if ev == Event::Resize {
            let h = w.height() - MENUBAR_HEIGHT - LOG_HEIGHT;
            content_table::update_table_size(&mut table_clone, w.width(), h);
            true // Indicate that the event was handled
        } else if ev == Event::DndEnter || ev == Event::DndDrag || ev == Event::DndRelease {
            true
        } else if ev == Event::Paste {
            let new_configs = paste_operations::files_drag_n_drop();
            if new_configs.is_empty() {
                return false; // No new configs to add
            }
            for config in new_configs {
                remote_nodes_clone.borrow_mut().push(config);
            }
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
            true
        } else {
            false
        }
    });

    let icon = tray_icon::Icon::from_rgba(
        vec![
            255, 0, 0, 255, // Red pixel
            0, 255, 0, 255, // Green pixel
            0, 0, 255, 255, // Blue pixel
            255, 255, 0, 255, // Yellow pixel
        ],
        2,
        2,
    )?;

    let show_item = tray_icon::menu::MenuItem::new("Show main window", true, None);
    let quit_item = tray_icon::menu::MenuItem::new("Quit", true, None);

    let tray_menu = tray_icon::menu::Menu::with_items(&[&show_item, &quit_item])?;

    let _tray_icon = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Just a demo tray icon")
        .with_icon(icon)
        .build()?;

    win.show();

    use fltk::enums::{Color, Font};
    use fltk::text::{StyleTableEntry, TextBuffer, TextDisplay};
    // Create a text display for logs on the bottom of the main window
    let mut log_display = TextDisplay::new(0, win.h() - LOG_HEIGHT, win.w(), LOG_HEIGHT, None);
    let mut log_buffer = TextBuffer::default();
    let mut style_buffer = TextBuffer::default();
    log_display.set_buffer(Some(log_buffer.clone()));
    log_display.set_color(Color::Black);
    win.add(&log_display);

    // Define style table: A=Red, B=Yellow, C=Green, D=Gray, E=Blue
    let style_table = [
        StyleTableEntry {
            color: Color::Red,
            font: Font::Courier,
            size: 12,
        }, // A
        StyleTableEntry {
            color: Color::Yellow,
            font: Font::Courier,
            size: 12,
        }, // B
        StyleTableEntry {
            color: Color::Green,
            font: Font::Courier,
            size: 12,
        }, // C
        StyleTableEntry {
            color: Color::Light1,
            font: Font::Courier,
            size: 12,
        }, // D
        StyleTableEntry {
            color: Color::Blue,
            font: Font::Courier,
            size: 12,
        }, // E
    ];
    let style_map = |level: &log::Level| match level {
        log::Level::Error => 'A',
        log::Level::Warn => 'B',
        log::Level::Info => 'C',
        log::Level::Debug => 'D',
        log::Level::Trace => 'E',
    };

    let log_queue = std::sync::Arc::new(Mutex::new(Vec::new()));
    let log_queue_thread = log_queue.clone();
    std::thread::spawn(move || {
        for msg in rx {
            log_queue_thread.lock().unwrap().push(msg);
            fltk::app::awake();
        }
    });

    while ::fltk::app::wait() {
        // Handle tray menu events
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == show_item.id() {
                win.show();
            } else if event.id == quit_item.id() {
                ::fltk::app::quit();
                break;
            }
        }

        // Deal with settings dialog results
        while let Ok(new_settings) = settings_rx.try_recv() {
            let tun2proxy_enable = new_settings.tun2proxy_enable.unwrap_or_default();
            state.borrow_mut().system_settings = Some(new_settings);
            if tun2proxy_enable && !run_as::is_elevated() {
                if let Ok(status) = core::restart_as_admin() {
                    log::debug!("Restarted as admin with status code {status}, exiting current instance.");
                    ::fltk::app::quit();
                } else {
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description("Failed to restart as admin.")
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                }
            }
            log::info!("Settings updated. Log filter changes will take effect after restart.");
        }

        // Handle results from node details dialogs
        node_details_receivers.lock().unwrap().retain(|(row_opt, rx)| {
            match rx.try_recv() {
                Ok(Some(details)) => {
                    if let Some(row) = row_opt {
                        remote_nodes.borrow_mut()[*row] = details; // Editing existing node
                    } else {
                        remote_nodes.borrow_mut().push(details); // New node
                    }
                    refresh_table(&mut table, &mut win, remote_nodes.borrow().len());
                    false // remove
                }
                Ok(None) => false,                                 // user cancelled, remove
                Err(std::sync::mpsc::TryRecvError::Empty) => true, // retain
                Err(_) => false,                                   // channel closed, remove
            }
        });

        // Append logs to TextDisplay with highligting
        if let Ok(mut logs) = log_queue.lock() {
            let mut new_log_added = false;
            for msg in logs.drain(..) {
                let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                let level_char = style_map(&msg.0);
                let line = format!("[{ts} {:<5} {}] {}\n", msg.0, msg.1, msg.2);
                log_buffer.append(&line);
                style_buffer.append(&level_char.to_string().repeat(line.len()));
                new_log_added = true;
            }
            if new_log_added {
                // Self-defined maximum log lines
                const MAX_LOG_LINES: usize = 1000;
                let text = log_buffer.text();
                let log_lines: Vec<&str> = text.lines().collect();
                if log_lines.len() > MAX_LOG_LINES {
                    let start = log_lines.len() - MAX_LOG_LINES;
                    let new_text = log_lines[start..].join("\n") + "\n";
                    // Calculate character positions for style buffer
                    let mut char_start = 0;
                    for log_lines_i in log_lines.iter().take(start) {
                        char_start += log_lines_i.len() + 1; // +1 for '\n'
                    }
                    let mut char_end = char_start;
                    for log_lines_i in log_lines.iter().skip(start) {
                        char_end += log_lines_i.len() + 1;
                    }
                    let style_text = style_buffer.text();
                    let new_style = if char_end <= style_text.len() {
                        &style_text[char_start..char_end]
                    } else if char_start < style_text.len() {
                        &style_text[char_start..]
                    } else {
                        ""
                    };
                    log_buffer.set_text(&new_text);
                    style_buffer.set_text(new_style);
                }
                log_display.set_highlight_data(style_buffer.clone(), style_table);
                let lines = log_buffer.count_lines(0, log_buffer.length());
                log_display.scroll(lines, 0);
            }
        }
    }

    state.borrow_mut().remote_nodes = remote_nodes.borrow().clone();
    state.borrow_mut().window.refresh_window(&win);
    state.borrow_mut().current_node_index = *current_node_index.borrow();

    states_manager::save_app_state(&state.borrow())?;

    if let Err(e) = stop_running_node(&running_token, &running_handle) {
        log::debug!("Failed to stop running node: {e}");
    }

    Ok(())
}
