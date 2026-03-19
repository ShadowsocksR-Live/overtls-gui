#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Represents a set of menu IDs.
#[derive(strum::EnumIter, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MenuId {
    Settings = 1001,
    ScanQrCode = 1002,
    ImportNodeFile = 1003,
    New = 1004,
    OverTls = 1005,
    Tun2proxy = 1006,
    HttpProxy = 1007,
    Open = 1008,
    Quit = 1009,
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
            x if x == MenuId::OverTls as i32 => Ok(MenuId::OverTls),
            x if x == MenuId::Tun2proxy as i32 => Ok(MenuId::Tun2proxy),
            x if x == MenuId::HttpProxy as i32 => Ok(MenuId::HttpProxy),
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

// single-instance helper logic extracted to its own module
mod single_instance;

use model::{ServerList, create_server_tree_model};
pub(crate) use overtls::Config as ServerNode;
use settings::{MAIN_ICON, WindowConfig, create_bitmap_from_memory};
use std::{cell::RefCell, rc::Rc, sync::Arc, sync::Mutex};
use wxdragon::prelude::*;

// Toolbar tool IDs (distinct from menu IDs)
const ID_TOOL_OVERTLS: Id = MenuId::OverTls as Id; // can reuse menu ID since it's the same action
const ID_TOOL_TUN2PROXY: Id = MenuId::Tun2proxy as Id; // can reuse menu ID since it's the same action
const ID_TOOL_HTTPPROXY: Id = MenuId::HttpProxy as Id; // can reuse menu ID since it's the same action
const ID_TOOL_SETTINGS: Id = MenuId::Settings as Id; // simple button to open settings

fn main() -> std::io::Result<()> {
    // #[cfg(debug_assertions)]
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();

    let cfg = Arc::new(Mutex::new(settings::load_settings()));

    if cfg.lock().unwrap().run_as_admin.unwrap_or_default() && !run_as::is_elevated() {
        let status = restart_as_admin()?;
        std::process::exit(status.code().unwrap_or_default());
    }

    // --- single-instance detection / activation -----------------------------
    let Ok(activation_listener) = crate::single_instance::acquire() else {
        // an Err return signals that another instance was present and already
        // notified; just exit quietly.
        return Ok(());
    };

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

    // holder for the activation timer.  storing it in an `Arc` outside the
    // closure ensures the timer lives for the lifetime of the application
    // without needing to `mem::forget` it.
    let timer_holder: Rc<RefCell<Option<Timer<Frame>>>> = Rc::new(RefCell::new(None));
    let timer_holder_clone = timer_holder.clone();

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

        // if we bound the activation port successfully earlier, start a
        // background thread to accept connections and send a simple signal over
        // a channel. the UI thread will poll the receiver via a wx timer and
        // perform the actual raise operation, which avoids moving the `Frame`
        // across thread boundaries.
        if let Some(listener) = activation_listener {
            // spawn helper returns receiver we can poll
            let act_rx = crate::single_instance::spawn_activation_listener(listener);

            // timer on the UI thread polls the receiver and raises the window
            let timer = Timer::new(&frame);
            timer.on_tick(move |_evt| {
                if act_rx.try_recv().is_ok() {
                    restore_main_window(&frame);
                }
            });
            // choose a small interval so activation is responsive but not busy
            timer.start(150, false);
            // save timer in outer Rc so it stays alive
            *timer_holder_clone.borrow_mut() = Some(timer);
        }

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
        // Use flat style to ensure separators are drawn visibly
        let tb_style = ToolBarStyle::Text | ToolBarStyle::Default | ToolBarStyle::Flat;
        // keep a handle so we can toggle state later when tools are clicked
        let toolbar_opt = frame.create_tool_bar(Some(tb_style), ID_ANY as i32);
        if let Some(toolbar) = &toolbar_opt {
            let icon_size = ArtProvider::get_native_dip_size_hint(ArtClient::Toolbar);

            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::HelpSettings, ArtClient::Toolbar, None) {
                if let Some(bmp) = bundle.get_bitmap_for(&frame) {
                    toolbar.add_tool(ID_TOOL_SETTINGS, "Settings", &bmp, "Open Settings");
                }
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::HelpSettings, ArtClient::Toolbar, None) {
                toolbar.add_tool(ID_TOOL_SETTINGS, "Settings", &icon, "Open Settings");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_tool(ID_TOOL_SETTINGS, "Settings", &bmp, "Open Settings");
            }

            // toolbar.add_separator();
            let sep: StaticLine = StaticLine::builder(toolbar)
                .with_size(Size::new(1, icon_size.height + 8))
                .with_style(StaticLineStyle::Vertical)
                .build();
            sep.set_background_color(colours::gray::GRAY_600);
            sep.set_foreground_color(colours::gray::GRAY_600);
            toolbar.add_control(&sep);

            // OverTLS tool (toggle)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::New, ArtClient::Toolbar, None) {
                if let Some(bmp) = bundle.get_bitmap_for(&frame) {
                    toolbar.add_check_tool(ID_TOOL_OVERTLS, "OverTLS", &bmp, "Start OverTLS (SOCKS5)");
                }
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::New, ArtClient::Toolbar, None) {
                toolbar.add_check_tool(ID_TOOL_OVERTLS, "OverTLS", &icon, "Start OverTLS (SOCKS5)");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_check_tool(ID_TOOL_OVERTLS, "OverTLS", &bmp, "Start OverTLS (SOCKS5)");
            }

            // Tun2Proxy tool (toggle)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::FileOpen, ArtClient::Toolbar, None) {
                if let Some(bmp) = bundle.get_bitmap_for(&frame) {
                    toolbar.add_check_tool(ID_TOOL_TUN2PROXY, "Tun2Proxy", &bmp, "Start Tun2Proxy");
                }
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::FileOpen, ArtClient::Toolbar, None) {
                toolbar.add_check_tool(ID_TOOL_TUN2PROXY, "Tun2Proxy", &icon, "Start Tun2Proxy");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_check_tool(ID_TOOL_TUN2PROXY, "Tun2Proxy", &bmp, "Start Tun2Proxy");
            }

            // HTTP Proxy tool (toggle)
            if let Some(bundle) = ArtProvider::get_bitmap_bundle(ArtId::FileSave, ArtClient::Toolbar, None) {
                if let Some(bmp) = bundle.get_bitmap_for(&frame) {
                    toolbar.add_check_tool(ID_TOOL_HTTPPROXY, "HTTP Proxy", &bmp, "Start HTTP Proxy");
                }
            } else if let Some(icon) = ArtProvider::get_bitmap(ArtId::FileSave, ArtClient::Toolbar, None) {
                toolbar.add_check_tool(ID_TOOL_HTTPPROXY, "HTTP Proxy", &icon, "Start HTTP Proxy");
            } else if let Ok(bmp) = create_bitmap_from_memory(MAIN_ICON, Some((icon_size.width as u32, icon_size.height as u32))) {
                toolbar.add_check_tool(ID_TOOL_HTTPPROXY, "HTTP Proxy", &bmp, "Start HTTP Proxy");
            }

            toolbar.realize();
            // if the process is not elevated, tun2proxy cannot run; disable the tool
            if !run_as::is_elevated() {
                toolbar.enable_tool(ID_TOOL_TUN2PROXY, false);
                toolbar.set_tool_short_help(ID_TOOL_TUN2PROXY, "Requires administrator privileges");
            }

            // ensure initial button states match any existing services
            sync_toolbar(toolbar);
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
                    restore_main_window(&frame_taskbar);
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

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        taskbar.on_left_down(move |_event| {
            log::info!("Taskbar icon clicked, toggling main window visibility.");
            toggle_main_window_from_tray(&frame_taskbar);
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
            .append_check_item(MenuId::OverTls.into(), "OverTls\tF5", "Run OverTls node")
            .append_check_item(MenuId::Tun2proxy.into(), "Tun2proxy\tShift+F5", "Tun2proxy service")
            .append_check_item(MenuId::HttpProxy.into(), "HttpProxy\tCtrl+F5", "HttpProxy service")
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

        // ensure the menu items have the correct checked state at startup as well
        if let Some(mbar) = frame.get_menu_bar() {
            sync_menu(&mbar);
        }

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
                ];
                for id in gated {
                    // Enable only if there is a pending selection
                    let _ = mbar.enable_item(id.into(), has_sel);
                }

                // also update the checked state of our three toggle actions
                sync_menu(&mbar);
            }
        });

        let model_for_menu = model.clone();
        let cfg_for_menu = cfg_clone.clone();
        frame.on_menu(move |event| {
            let id = event.get_id();
            // special handling for the three toggle tools
            if id == ID_TOOL_OVERTLS {
                if core::is_overtls_running() {
                    let _ = core::stop_overtls_only();
                } else {
                    core::start_overtls_only(&frame, &model_for_menu, &cfg_for_menu);
                }
            } else if id == ID_TOOL_TUN2PROXY {
                if !run_as::is_elevated() {
                    // action not allowed without admin rights
                    MessageDialog::builder(&frame, "Tun2Proxy requires administrator privileges.", "Permission Denied")
                        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
                        .build()
                        .show_modal();
                } else if core::is_tun2proxy_running() {
                    let _ = core::stop_tun2proxy_only();
                } else {
                    core::start_tun2proxy_only(&frame, &cfg_for_menu);
                }
            } else if id == ID_TOOL_HTTPPROXY {
                if core::is_http_proxy_running() {
                    let _ = core::stop_http_proxy_only();
                } else {
                    core::start_http_proxy_only(&frame, &cfg_for_menu);
                }
            } else {
                menu_actions::handle_menu_command(&frame, &model_for_menu, id, &cfg_for_menu);
            }

            // each time we're about to handle something, refresh toolbar state
            if let Some(tb) = &toolbar_opt {
                sync_toolbar(tb);
            }
            // and keep the menubar entries checked appropriately as well
            if let Some(mbar) = frame.get_menu_bar() {
                sync_menu(&mbar);
            }
        });

        // clone config for use in close/destroy handlers
        let cfg_for_close = cfg_clone.clone();

        frame.on_close(move |evt| {
            if let wxdragon::WindowEventData::General(event) = &evt {
                // Record current position/size before hiding – otherwise the window will be
                // hidden and get_position() returns (-1,-1) which ends up in settings.
                let pos = frame.get_position();
                let size = frame.get_size();
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
                    frame.show(false);
                }
            }
        });

        let model_for_destroy = model.clone();
        let cfg_for_destroy = cfg_clone.clone();
        frame.on_destroy(move |_data| {
            core::stop_all_services().ok(); // best effort to stop any running services before exit

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

// helper to update all three toggle buttons according to actual running state
fn sync_toolbar(tb: &wxdragon::widgets::ToolBar) {
    let running = core::is_overtls_running();
    tb.toggle_tool(ID_TOOL_OVERTLS, running);
    tb.set_tool_short_help(ID_TOOL_OVERTLS, if running { "Stop OverTLS" } else { "Start OverTLS (SOCKS5)" });

    let t2p = core::is_tun2proxy_running();
    tb.toggle_tool(ID_TOOL_TUN2PROXY, t2p);
    tb.set_tool_short_help(ID_TOOL_TUN2PROXY, if t2p { "Stop Tun2Proxy" } else { "Start Tun2Proxy" });

    let http = core::is_http_proxy_running();
    tb.toggle_tool(ID_TOOL_HTTPPROXY, http);
    tb.set_tool_short_help(ID_TOOL_HTTPPROXY, if http { "Stop HTTP Proxy" } else { "Start HTTP Proxy" });
}

// helper to update the checked state of the same three actions on the main menu
fn sync_menu(mb: &wxdragon::menus::MenuBar) {
    if !run_as::is_elevated() {
        // If not elevated, ensure the Tun2Proxy menu item is disabled
        mb.enable_item(MenuId::Tun2proxy.into(), false);
    }

    mb.check_item(MenuId::OverTls.into(), core::is_overtls_running());
    mb.check_item(MenuId::Tun2proxy.into(), core::is_tun2proxy_running());
    mb.check_item(MenuId::HttpProxy.into(), core::is_http_proxy_running());
}

fn restore_main_window(frame: &Frame) {
    frame.show(true);
    frame.iconize(false);
    frame.raise();
    frame.set_focus();
}

#[allow(dead_code)]
fn toggle_main_window_from_tray(frame: &Frame) {
    if frame.is_shown() && !frame.is_iconized() {
        frame.show(false);
    } else {
        restore_main_window(frame);
    }
}
