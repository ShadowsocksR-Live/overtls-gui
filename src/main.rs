// To remove the console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::content_table::refresh_table;
use fltk::{
    app, dialog,
    enums::{Event, Shortcut},
    menu::{MenuBar, MenuFlag},
    prelude::{GroupExt, MenuExt, WidgetBase, WidgetExt, WindowExt},
    terminal::Terminal,
    window::Window,
};
use std::cell::RefCell;
use std::rc::Rc;

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
pub(crate) const COMMON_DLG_W: i32 = 400;
pub(crate) const COMMON_DLG_H: i32 = 100;
pub(crate) const LOG_HEIGHT: i32 = 240;
pub(crate) const MAX_LOG_LINES: usize = 1000;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // #[cfg(debug_assertions)]
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(e) = log::set_boxed_logger(Box::new(logger::Logger::new(tx))) {
        eprintln!("Failed to set logger: {e}");
    }
    log::set_max_level(log::LevelFilter::Debug);

    let state = states_manager::load_app_state();
    let tun2proxy_enable = state.system_settings.clone().unwrap_or_default().tun2proxy_enable.unwrap_or(true);
    let state = Rc::new(RefCell::new(state));

    if tun2proxy_enable && !run_as::is_elevated() {
        let status = core::restart_as_admin()?;
        std::process::exit(status.code().unwrap_or_default());
    }

    let remote_nodes = Rc::new(RefCell::new(state.borrow().remote_nodes.clone()));
    let current_node_index = Rc::new(RefCell::new(state.borrow().current_node_index));

    let app = app::App::default();

    let ws = state.borrow().window.clone();
    let title = format!("OverTLS clients manager for {}", util::host_os_name());
    let mut win = Window::new(ws.x, ws.y, ws.w, ws.h, title.as_str());

    let mut menubar = MenuBar::new(0, 0, ws.w, MENUBAR_HEIGHT, "");

    let mut table = content_table::create_table(&current_node_index, &remote_nodes, &win);

    refresh_table(&mut table, &mut win, remote_nodes.borrow().len());

    let w = win.clone();
    let state_clone = state.clone();
    menubar.add("&File/Settings", Shortcut::None, MenuFlag::MenuDivider, move |_m| {
        let settings = state_clone.borrow().system_settings.clone().unwrap_or_default();
        if let Some(new_settings) = settings_dialog::show_settings_dialog(&w, &settings) {
            let tun2proxy_enable = new_settings.tun2proxy_enable.unwrap_or_default();
            state_clone.borrow_mut().system_settings = Some(new_settings);
            if tun2proxy_enable && !run_as::is_elevated() {
                if let Ok(status) = core::restart_as_admin() {
                    log::debug!("Restarted as admin with status code {status}, exiting current instance.");
                    app::quit();
                } else {
                    let x = w.x() + (w.width() - COMMON_DLG_W) / 2;
                    let y = w.y() + (w.height() - COMMON_DLG_H) / 2;
                    dialog::alert(x, y, "Failed to restart as admin.");
                }
            }
        }
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add(
        "&File/Scan QR Code from screen\t",
        Shortcut::Ctrl | 'r',
        MenuFlag::Normal,
        move |_m| {
            let x = w.x() + (w.w() - COMMON_DLG_W) / 2;
            let y = w.y() + (w.h() - COMMON_DLG_H) / 2;
            match paste_operations::screenshot_qr_import() {
                Ok(config) => {
                    remote_nodes_clone.borrow_mut().push(config);
                    refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
                    dialog::message(x, y, "QR Code scanned and imported!");
                }
                Err(e) => dialog::alert(x, y, &format!("Failed to import QR Code: {e}")),
            }
        },
    );

    let remote_nodes_clone = remote_nodes.clone();
    let state_clone = state.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&File/Import Node from File", Shortcut::None, MenuFlag::Normal, move |_menu| {
        let dlg_w = 600;
        let dlg_h = 400;
        let x = w.x() + (w.w() - dlg_w) / 2;
        let y = w.y() + (w.h() - dlg_h) / 2;
        let origin_path = state_clone
            .borrow()
            .current_selection_path
            .clone()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap()));
        // Show file chooser dialog and import config file
        let path = util::file_chooser_open_file(x, y, dlg_w, dlg_h, "Select config file", "*.json", origin_path.to_str());
        if let Some(path) = &path {
            match overtls::Config::from_config_file(path) {
                Ok(config) => {
                    if let Some(parent_dir) = std::path::Path::new(path).parent() {
                        state_clone.borrow_mut().set_current_path(parent_dir);
                    }
                    remote_nodes_clone.borrow_mut().push(config);
                    refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
                }
                Err(e) => {
                    dialog::alert(x, y, &format!("Import failed: {e}"));
                }
            }
        }
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&File/New\t", Shortcut::Ctrl | 'n', MenuFlag::MenuDivider, move |_m| {
        if let Some(node) = crate::node_details_dialog::show_node_details(&w, None) {
            remote_nodes_clone.borrow_mut().push(node);
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        }
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
    let w = win.clone();
    menubar.add("&File/Run", Shortcut::None, MenuFlag::Normal, move |_m| {
        let x = w.x() + (w.w() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.h() - COMMON_DLG_H) / 2;
        let Some(idx) = *current_node_index_run.borrow() else {
            dialog::alert(x, y, "Please select a node first.");
            return;
        };
        let Some(mut config) = remote_nodes_run.borrow().get(idx).cloned() else {
            dialog::alert(x, y, "Selected node not found.");
            return;
        };
        // Stop node first if it's running
        if running_token_run.lock().unwrap().is_some() {
            dialog::alert(x, y, "A node is already running. Please stop it first.");
            return;
        }

        let system_settings = state_clone.borrow().system_settings.clone().unwrap_or_default();
        let tun2proxy_enable = system_settings.tun2proxy_enable.unwrap_or(false);
        if tun2proxy_enable && !run_as::is_elevated() {
            dialog::alert(x, y, "Requires admin privileges. Please restart the application as administrator.");
            return;
        }

        core::merge_system_settings_to_node_config(&system_settings, &mut config);

        if let Err(e) = config.check_correctness(false) {
            dialog::alert(x, y, &format!("Configuration error: {e}"));
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
    let w = win.clone();
    menubar.add("&File/Stop", Shortcut::None, MenuFlag::MenuDivider, move |_m| {
        let x = w.x() + (w.w() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.h() - COMMON_DLG_H) / 2;
        if let Err(e) = stop_running_node(&running_token_stop, &running_handle_stop) {
            dialog::alert(x, y, &format!("Failed to stop running node: {e}"));
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

    menubar.add("&File/Quit\t", Shortcut::Ctrl | 'q', MenuFlag::Normal, move |_| {
        app::quit();
    });

    // --- Edit menu group: View Details ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Edit/View Details", Shortcut::None, MenuFlag::Normal, move |_menu| {
        let x = w.x() + (w.w() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.h() - COMMON_DLG_H) / 2;
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            dialog::alert(x, y, "No node selected.");
            return;
        };
        let Some(cfg) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            dialog::alert(x, y, "Selected node not found.");
            return;
        };
        if let Some(node) = crate::node_details_dialog::show_node_details(&w, Some(cfg)) {
            remote_nodes_clone.borrow_mut()[selected_row] = node;
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        }
    });

    // --- Edit menu group: View QR Code ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let w = win.clone();
    menubar.add("&Edit/Show QR Code", Shortcut::None, MenuFlag::MenuDivider, move |_menu| {
        let x = w.x() + (w.width() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.height() - COMMON_DLG_H) / 2;
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            dialog::alert(x, y, "No node selected.");
            return;
        };
        let Some(cfg) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            dialog::alert(x, y, "Selected node not found.");
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
                dialog::alert(x, y, &format!("Failed to show QR Code: {e}"));
            }
        } else {
            dialog::alert(x, y, "Failed to generate SSR URL for QR Code.");
        }
    });

    // --- Edit menu group: Delete ---
    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Edit/Delete", Shortcut::None, MenuFlag::MenuDivider, move |_menu| {
        let x = w.x() + (w.w() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.h() - COMMON_DLG_H) / 2;
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            dialog::alert(x, y, "No node selected.");
            return;
        };
        if selected_row > remote_nodes_clone.borrow().len() {
            dialog::alert(x, y, "Selected node not found.");
            return;
        }
        let title = remote_nodes_clone
            .borrow()
            .get(selected_row)
            .map(|c| c.remarks.clone().unwrap_or_default())
            .unwrap_or_default();
        let confirm = dialog::choice2(x, y, &format!("Are you sure you want to delete node: '{title}'?"), "Yes", "No", "");
        if confirm == Some(0) {
            remote_nodes_clone.borrow_mut().remove(selected_row);
            *current_node_index_clone.borrow_mut() = None;
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        }
    });

    let current_node_index_clone = current_node_index.clone();
    let remote_nodes_clone = remote_nodes.clone();
    let w = win.clone();
    menubar.add("&Edit/Copy\t", Shortcut::Ctrl | 'c', MenuFlag::Normal, move |_menu| {
        log::trace!("Copy event triggered");
        let x = w.x() + (w.width() - COMMON_DLG_W) / 2;
        let y = w.y() + (w.height() - COMMON_DLG_H) / 2;
        let Some(selected_row) = *current_node_index_clone.borrow() else {
            dialog::alert(x, y, "No node selected.");
            return;
        };
        let Some(node) = remote_nodes_clone.borrow().get(selected_row).cloned() else {
            dialog::alert(x, y, "Selected node not found.");
            return;
        };
        if let Ok(text) = &node.generate_ssr_url() {
            app::copy(text);
            let name = node.remarks.clone().unwrap_or_default();
            dialog::message(x, y, &format!("Node '{name}'s URL copied to clipboard"));
        } else {
            dialog::alert(x, y, "Failed to generate URL for the selected node.");
        }
    });

    let remote_nodes_clone = remote_nodes.clone();
    let mut table_clone = table.clone();
    let mut w = win.clone();
    menubar.add("&Edit/Paste\t", Shortcut::Ctrl | 'v', MenuFlag::Normal, move |_menu| {
        if let Ok(config) = paste_operations::paste() {
            remote_nodes_clone.borrow_mut().push(config);
            refresh_table(&mut table_clone, &mut w, remote_nodes_clone.borrow().len());
        } else {
            let x = w.x() + (w.width() - COMMON_DLG_W) / 2;
            let y = w.y() + (w.height() - COMMON_DLG_H) / 2;
            dialog::alert(x, y, "No valid configuration found in clipboard.");
        }
    });

    let win_clone = win.clone();
    menubar.add("&Help/About", Shortcut::None, MenuFlag::Normal, move |_| {
        let x = win_clone.x() + (win_clone.width() - COMMON_DLG_W) / 2;
        let y = win_clone.y() + (win_clone.height() - COMMON_DLG_H) / 2;
        dialog::message(x, y, "This is a demo menu!");
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

    // Use Terminal widget to display logs
    let mut log_terminal = Terminal::new(0, win.h() - LOG_HEIGHT, win.w(), LOG_HEIGHT, None);
    log_terminal.set_history_rows(MAX_LOG_LINES as i32);
    win.add(&log_terminal);

    // Log receiving thread, only operates on TextBuffer
    let log_queue = std::sync::Arc::new(Mutex::new(Vec::new()));
    let log_queue_thread = log_queue.clone();
    std::thread::spawn(move || {
        for msg in rx {
            log_queue_thread.lock().unwrap().push(msg);
            fltk::app::awake();
        }
    });

    while app.wait() {
        // Handle tray menu events
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == show_item.id() {
                win.show();
            } else if event.id == quit_item.id() {
                app::quit();
                break;
            }
        }

        // Append logs from the queue to the Terminal
        if let Ok(mut logs) = log_queue.lock() {
            for msg in logs.drain(..) {
                let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                let level_str = format!("{:<5}", msg.0.to_string());
                let color = match msg.0 {
                    log::Level::Error => "\x1b[31m", // red
                    log::Level::Warn => "\x1b[33m",  // yellow
                    log::Level::Info => "\x1b[32m",  // green
                    log::Level::Debug => "\x1b[37m", // gray
                    log::Level::Trace => "\x1b[36m", // cyan
                };
                let color_end = "\x1b[0m";
                let line = format!("[{ts} {color}{level_str}{color_end} {}] {}\n", msg.1, msg.2);
                log_terminal.append(&line);
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
