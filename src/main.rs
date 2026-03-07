#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Represents a set of menu IDs.
#[derive(strum::EnumIter, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MenuId {
    Settings = 1001,
    ScanQrCode = 1002,
    ImportNodeFile = 1003,
    New = 1004,
    Run = 1005,
    Stop = 1006,
    Open = 1007,
    Quit = 1008,
    ViewDetails = 3001,
    ExportNode = 3002,
    ShowQrCode = 3003,
    Delete = 3004,
    Copy = 3005,
    Paste = 3006,
    About = 4001,
}

impl From<MenuId> for i32 {
    fn from(id: MenuId) -> i32 {
        id as i32
    }
}

impl TryFrom<i32> for MenuId {
    type Error = std::io::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            x if x == MenuId::Settings as i32 => Ok(MenuId::Settings),
            x if x == MenuId::ScanQrCode as i32 => Ok(MenuId::ScanQrCode),
            x if x == MenuId::ImportNodeFile as i32 => Ok(MenuId::ImportNodeFile),
            x if x == MenuId::New as i32 => Ok(MenuId::New),
            x if x == MenuId::Run as i32 => Ok(MenuId::Run),
            x if x == MenuId::Stop as i32 => Ok(MenuId::Stop),
            x if x == MenuId::Open as i32 => Ok(MenuId::Open),
            x if x == MenuId::Quit as i32 => Ok(MenuId::Quit),
            x if x == MenuId::ViewDetails as i32 => Ok(MenuId::ViewDetails),
            x if x == MenuId::ExportNode as i32 => Ok(MenuId::ExportNode),
            x if x == MenuId::ShowQrCode as i32 => Ok(MenuId::ShowQrCode),
            x if x == MenuId::Delete as i32 => Ok(MenuId::Delete),
            x if x == MenuId::Copy as i32 => Ok(MenuId::Copy),
            x if x == MenuId::Paste as i32 => Ok(MenuId::Paste),
            x if x == MenuId::About as i32 => Ok(MenuId::About),
            _ => Err(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
        }
    }
}

mod about_dlg;
mod core;
mod dataview;
mod details_dlg;
mod logger;
mod logview;
mod menu_actions;
mod model;
mod selection_ctx;
mod settings;
mod settings_dlg;
mod show_qrcode_dlg;
mod util;

use model::{ServerList, create_server_tree_model};
pub(crate) use overtls::Config as ServerNode;
use settings::{MAIN_ICON, WindowConfig, create_bitmap_from_memory};
use std::{cell::RefCell, rc::Rc, sync::Arc, sync::Mutex};
use wxdragon::prelude::*;

// Toolbar tool IDs (distinct from menu IDs)
const ID_TOOL_OVERTLS: Id = ID_HIGHEST + 101;
const ID_TOOL_TUN2PROXY: Id = ID_HIGHEST + 102;
const ID_TOOL_HTTPPROXY: Id = ID_HIGHEST + 103;

fn main() -> std::io::Result<()> {
    // #[cfg(debug_assertions)]
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();

    let cfg = Arc::new(Mutex::new(settings::load_settings()));

    if cfg.lock().unwrap().run_as_admin.unwrap_or_default() && !run_as::is_elevated() {
        let status = restart_as_admin()?;
        std::process::exit(status.code().unwrap_or_default());
    }

    let logging_settings = cfg.lock().unwrap().logging.clone().unwrap_or_default();
    let (tx, logging_rx) = std::sync::mpsc::channel();
    if let Err(e) = log::set_boxed_logger(Box::new(logging_settings.create_logger(tx))) {
        log::warn!("Failed to set logger: {e}");
    }
    // Note: No longer use log::set_max_level, as it is now controlled by the Logger internally
    log::set_max_level(log::LevelFilter::Trace);

    let log_queue = std::sync::Arc::new(Mutex::new(Vec::new()));
    let log_queue_thread = log_queue.clone();
    std::thread::spawn(move || {
        for msg in logging_rx {
            log_queue_thread.lock().unwrap().push(msg);
        }
    });

    let cfg_clone = cfg.clone();
    let _ = wxdragon::main(move |_| {
        // Build model once from settings.servers
        let mut nodes = cfg_clone.lock().unwrap().servers.clone();

        // Demo seed data: when servers key is missing (None), add two example nodes
        if nodes.is_none() {
            let mut seed1_client = overtls::ClientConfig::default();
            seed1_client.client_id = Some("client-001".to_string());
            seed1_client.server_host = "example.com".to_string();
            seed1_client.server_port = 443;
            seed1_client.server_domain = Some("example.com".to_string());
            let seed1 = ServerNode {
                remarks: Some("Sample Server 1".to_string()),
                tunnel_path: overtls::TunnelPath::Single("/".to_string()),
                client: Some(seed1_client),
                ..Default::default()
            };
            let mut seed2_client = overtls::ClientConfig::default();
            seed2_client.server_host = "127.0.0.1".to_string();
            seed2_client.server_port = 8080;
            seed2_client.disable_tls = Some(true); // indicate TLS disabled
            let seed2 = ServerNode {
                remarks: Some("Local Dev".to_string()),
                tunnel_path: overtls::TunnelPath::Single("/dev".to_string()),
                client: Some(seed2_client),
                ..Default::default()
            };
            nodes = Some(vec![seed1, seed2]);
        }
        let nodes = nodes.unwrap_or_default().into_iter().map(|n| Rc::new(RefCell::new(n))).collect();
        let data = Rc::new(RefCell::new(ServerList { nodes }));
        let model = create_server_tree_model(data);

        let win_cfg = cfg_clone.lock().unwrap().window.as_ref().cloned().unwrap_or_default();

        let frame = Frame::builder()
            .with_title(settings::APP_TITLE)
            .with_position(win_cfg.get_point())
            .with_size(win_cfg.get_size())
            .build();

        let icon_bitmap = create_bitmap_from_memory(MAIN_ICON, Some((48, 48))).unwrap();
        frame.set_icon(&icon_bitmap);

        // --- Status Bar Setup ---
        StatusBar::builder(&frame)
            .with_fields_count(3)
            .with_status_widths(vec![-1, 150, 100])
            .add_initial_text(0, "Ready")
            .add_initial_text(1, "Center Field")
            .add_initial_text(2, "Right Field")
            .build();

        // --- ToolBar Setup ---
        let tb_style = ToolBarStyle::Text | ToolBarStyle::Default;
        if let Some(toolbar) = frame.create_tool_bar(Some(tb_style), ID_ANY as i32) {
            let icon_size = ArtProvider::get_native_dip_size_hint(ArtClient::Toolbar);

            // OverTLS tool (icon: New or fallback)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::New, ArtClient::Toolbar, None) {
                toolbar.add_tool_bundle(ID_TOOL_OVERTLS, "OverTLS", &bundle, "Start OverTLS (SOCKS5)");
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::New, ArtClient::Toolbar, None) {
                toolbar.add_tool(ID_TOOL_OVERTLS, "OverTLS", &icon, "Start OverTLS (SOCKS5)");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_tool(ID_TOOL_OVERTLS, "OverTLS", &bmp, "Start OverTLS (SOCKS5)");
            }

            // Tun2Proxy tool (icon: FileOpen or fallback)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::FileOpen, ArtClient::Toolbar, None) {
                toolbar.add_tool_bundle(ID_TOOL_TUN2PROXY, "Tun2Proxy", &bundle, "Start Tun2Proxy");
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::FileOpen, ArtClient::Toolbar, None) {
                toolbar.add_tool(ID_TOOL_TUN2PROXY, "Tun2Proxy", &icon, "Start Tun2Proxy");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_tool(ID_TOOL_TUN2PROXY, "Tun2Proxy", &bmp, "Start Tun2Proxy");
            }

            // HTTP Proxy tool (icon: FileSave or fallback)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::FileSave, ArtClient::Toolbar, None) {
                toolbar.add_tool_bundle(ID_TOOL_HTTPPROXY, "HTTP Proxy", &bundle, "Start HTTP Proxy");
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::FileSave, ArtClient::Toolbar, None) {
                toolbar.add_tool(ID_TOOL_HTTPPROXY, "HTTP Proxy", &icon, "Start HTTP Proxy");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_tool(ID_TOOL_HTTPPROXY, "HTTP Proxy", &bmp, "Start HTTP Proxy");
            }

            toolbar.realize();
        }

        // Create popup menu for taskbar icon
        let mut tray_icon_menu = Menu::builder()
            .append_item(MenuId::Open.into(), "Open Application", "Open the main application window")
            .append_separator()
            .append_item(MenuId::Settings.into(), "Settings", "Open application settings")
            .append_item(MenuId::About.into(), "About", "About this application")
            .append_separator()
            .append_item(MenuId::Quit.into(), "Quit", "Quit the application")
            .build();
        let taskbar = TaskBarIcon::builder().with_icon_type(TaskBarIconType::CustomStatusItem).build();
        taskbar.set_popup_menu(&mut tray_icon_menu);
        let frame_taskbar = frame;
        let cfg_for_taskbar = cfg_clone.clone();
        taskbar.on_menu(move |event| {
            let menu_id = event.get_id();
            match menu_id {
                x if x == MenuId::Open as i32 => {
                    log::info!("📂 Open Application clicked!");
                    frame_taskbar.show(true);
                    frame_taskbar.iconize(false);
                    frame_taskbar.raise();
                }
                x if x == MenuId::Settings as i32 => {
                    log::info!("⚙️ Settings clicked!");
                    settings_dlg::settings_dlg(&frame_taskbar, &cfg_for_taskbar);
                }
                x if x == MenuId::About as i32 => {
                    log::info!("ℹ️ About clicked!");
                    about_dlg::show_about_dialog(&frame_taskbar);
                }
                x if x == MenuId::Quit as i32 => {
                    log::info!("🚪 Quit clicked!");
                    frame_taskbar.close(true);
                }
                _ => {
                    log::warn!("Unknown menu item clicked: {menu_id}");
                }
            }
        });

        let success = taskbar.set_icon(&icon_bitmap, "OverTLS server node manager");

        if success && taskbar.is_icon_installed() {
            log::info!("TaskBarIcon successfully installed in system tray.");
        } else {
            log::error!("Failed to set taskbar icon.");
        }

        // --- Menu Bar Setup ---
        // Main menu
        let main_menu = Menu::builder()
            .append_item(MenuId::Settings.into(), "Settings", "Open application settings")
            .append_separator()
            .append_item(MenuId::ScanQrCode.into(), "Scan QR Code\tCtrl+Shift+Q", "Scan QR code from screen")
            .append_item(MenuId::ImportNodeFile.into(), "Import Node File", "Import node file")
            .append_item(MenuId::New.into(), "New", "Create new node")
            .append_separator()
            .append_item(MenuId::Run.into(), "Run\tF5", "Run node")
            .append_item(MenuId::Stop.into(), "Stop\tShift+F5", "Stop node")
            .append_separator()
            .append_item(MenuId::Quit.into(), "Quit\tCtrl+Q", "Quit the application")
            .build();

        // Node menu
        let node_menu = Menu::builder()
            .append_item(MenuId::ViewDetails.into(), "View Details", "View node details")
            .append_item(MenuId::ExportNode.into(), "Export Node", "Export node")
            .append_item(MenuId::ShowQrCode.into(), "Show QR Code", "Show QR code for node")
            .append_separator()
            .append_item(MenuId::Delete.into(), "Delete\tDel", "Delete node")
            .append_separator()
            .append_item(MenuId::Copy.into(), "Copy\tCtrl+C", "Copy node")
            .append_item(MenuId::Paste.into(), "Paste\tCtrl+V", "Paste node")
            .build();

        // Help menu
        let help_menu = Menu::builder()
            .append_item(MenuId::About.into(), "About", "Show about dialog")
            .build();

        let menubar = MenuBar::builder()
            .append(main_menu, "Main")
            .append(node_menu, "Node")
            .append(help_menu, "Help")
            .build();
        frame.set_menu_bar(menubar);

        // Dynamically enable/disable Node menu items when the menu bar opens
        // Disable actions that require a selection if none is present
        let frame_for_menu_open = frame;
        frame.on_menu_opened(move |event: wxdragon::MenuEventData| {
            // Only handle the menubar case here; popup menus use a different path
            if event.is_popup() {
                log::info!("Popup menu opened, skipping dynamic enable/disable.");
                return;
            }
            if let Some(mbar) = frame_for_menu_open.get_menu_bar() {
                let has_sel = selection_ctx::has_pending_details();
                // Items that require a selection
                let gated = [
                    MenuId::ViewDetails,
                    MenuId::ExportNode,
                    MenuId::ShowQrCode,
                    MenuId::Delete,
                    MenuId::Copy,
                    MenuId::Run,
                ];
                for id in gated {
                    // Enable only if there is a pending selection
                    let _ = mbar.enable_item(id.into(), has_sel);
                }
            }
        });

        let frame_for_menu = frame;
        let model_for_menu = model.clone();
        let cfg_for_menu = cfg_clone.clone();
        frame.on_menu(move |event| {
            let id = event.get_id();
            if id == ID_TOOL_OVERTLS {
                menu_actions::start_overtls_only(&frame_for_menu, &model_for_menu, &cfg_for_menu);
                return;
            }
            if id == ID_TOOL_TUN2PROXY {
                menu_actions::start_tun2proxy_only(&frame_for_menu, &model_for_menu, &cfg_for_menu);
                return;
            }
            if id == ID_TOOL_HTTPPROXY {
                menu_actions::start_http_proxy_only(&frame_for_menu, &model_for_menu, &cfg_for_menu);
                return;
            }
            menu_actions::handle_menu_command(&frame_for_menu, &model_for_menu, id, &cfg_for_menu);
        });

        // clone config for use in close/destroy handlers
        let cfg_for_close = cfg_clone.clone();

        let frame_clone = frame.clone();
        frame.on_close(move |evt| {
            if let wxdragon::WindowEventData::General(event) = &evt {
                // Record current position/size before hiding – otherwise the window will be
                // hidden and get_position() returns (-1,-1) which ends up in settings.
                let pos = frame_clone.get_position();
                let size = frame_clone.get_size();
                // only store positive coordinates; hide/minimized windows return (-1,-1)
                if pos.x >= 0 && pos.y >= 0 && size.width > 0 && size.height > 0 {
                    let win = WindowConfig::new(pos, size);
                    cfg_for_close.lock().unwrap().window = Some(win);
                } else {
                    log::warn!("Skipping write of invalid window geometry ({:?}, {:?})", pos, size);
                }

                if event.can_veto() {
                    // If the close event is the window's default behavior (not from the taskbar menu or main menu)
                    // we veto the close and hide the window instead
                    log::debug!("Close event vetoed, hiding window instead of closing.");
                    event.veto();
                    frame_clone.show(false);
                }
            }
        });

        let model_for_destroy = model.clone();
        let cfg_for_destroy = cfg_clone.clone();
        frame.on_destroy(move |_data| {
            // Persist current servers from the model back to settings
            if let Some(servers) = model_for_destroy.with_userdata_mut::<Rc<RefCell<ServerList>>, Vec<ServerNode>>(|list_rc| {
                list_rc.borrow().nodes.iter().map(|rc| rc.borrow().clone()).collect()
            }) {
                cfg_for_destroy.lock().unwrap().servers = Some(servers);
            }

            // Clean up the TaskBarIcon, it's important to call destroy() to remove the icon from the system tray,
            // or we can't exit the application main loop.
            taskbar.destroy();

            // Clean up the tray icon menu to release rust closures attached to menu items
            tray_icon_menu.destroy_menu();
        });

        // --- Main Panel Layout ---
        let main_panel = Panel::builder(&frame).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();

        // Integrate DataView module (top, expands)
        let dataview_panel = dataview::create_data_view_panel(&main_panel, &model, &frame);
        sizer.add(&dataview_panel, 1, SizerFlag::Expand | SizerFlag::All, settings::WIDGET_MARGIN);

        // Integrate LogView module (bottom, fixed height)
        let logview_panel = logview::LogViewPanel::new(&main_panel);
        // Register the TextCtrl in UI-thread-local storage for callbacks
        logview::LOG_TEXT_CTRL.with(|cell| {
            *cell.borrow_mut() = Some(logview_panel.text_ctrl);
        });
        sizer.add(&logview_panel.panel, 0, SizerFlag::Expand | SizerFlag::All, settings::WIDGET_MARGIN);

        // Pump log_queue into the LogView TextCtrl using UI-thread callbacks
        {
            let ui_log_queue = log_queue.clone();
            std::thread::spawn(move || {
                // Throttle updates a bit to avoid overwhelming the UI
                const SLEEP_MS: u64 = 120;
                const MAX_LOG_LINES: usize = 1000;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(SLEEP_MS));

                    // Drain any pending log tuples into a local batch
                    let mut batch: Vec<(log::Level, String, String)> = Vec::new();
                    if let Ok(mut q) = ui_log_queue.lock()
                        && !q.is_empty()
                    {
                        batch.extend(q.drain(..));
                    }

                    if batch.is_empty() {
                        continue;
                    }

                    // Pre-format text in the background thread; Strings are Send
                    let appended = {
                        let mut lines = String::new();
                        for (level, module, msg) in batch.into_iter() {
                            // Example: "[2025-06-01T12:34:56Z INFO module] message\n"
                            let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                            lines.push_str(&format!("[{ts} {:<5} {}] {}\n", level, module, msg));
                        }
                        lines
                    };

                    // Apply text update on the UI thread using a ring buffer (stable trimming)
                    // Respect user's auto-scroll preference from settings
                    let cfg_for_autoscroll = cfg_clone.clone();
                    wxdragon::call_after(Box::new(move || {
                        let auto_scroll = cfg_for_autoscroll
                            .lock()
                            .ok()
                            .and_then(|c| c.logging.clone())
                            .and_then(|ls| ls.log_auto_scroll)
                            .unwrap_or_default();
                        logview::ui_append_logs(appended, MAX_LOG_LINES, auto_scroll);
                    }));
                }
            });
        }

        main_panel.set_sizer(sizer, true);

        frame.show(true);
    });

    // Save settings on exit
    settings::save_settings(&cfg.lock().unwrap());
    Ok(())
}

pub fn restart_as_admin() -> std::io::Result<std::process::ExitStatus> {
    log::debug!("Not running as admin, trying to elevate...");
    let status = run_as::restart_self_elevated(None, true, false, Some(std::time::Duration::from_secs(10)))?;
    Ok(status.unwrap_or_default())
}
