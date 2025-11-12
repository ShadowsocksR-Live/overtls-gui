use crate::settings::{Config, ICON_SIZE, MAIN_ICON, center_rect, create_bitmap_from_memory, save_settings};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use wxdragon::prelude::*;

pub fn settings_dlg(frame_clone: &dyn WxWidget, cfg: &Arc<Mutex<Config>>) {
    let (w, h) = (600, 400);
    let (x, y) = center_rect(frame_clone, w, h);

    // Create a generic dialog using the new builder
    let dialog = Dialog::builder(frame_clone, "Settings")
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_position(x, y)
        .with_size(w, h)
        .build();

    let icon_bitmap = create_bitmap_from_memory(MAIN_ICON, Some((ICON_SIZE, ICON_SIZE))).unwrap();
    dialog.set_icon(&icon_bitmap);

    // Add main panel to the dialog
    let panel = Panel::builder(&dialog).build();

    // Create Notebook for tabs
    let notebook = Notebook::builder(&panel).build();

    let save_result = Arc::new(AtomicBool::new(false));

    // Create tab pages using separate functions
    let common_panel = create_common_tab(&notebook, cfg, save_result.clone());
    let overtls_panel = create_overtls_tab(&notebook, cfg, save_result.clone());
    let tun2proxy_panel = create_tun2proxy_tab(&notebook, cfg, save_result.clone());
    let httpproxy_panel = create_httpproxy_tab(&notebook, cfg, save_result.clone());
    let logging_panel = create_logging_tab(&notebook, cfg, save_result.clone());

    let image_list = ImageList::new(16, 16, true, 4);
    let info_icon = ArtProvider::get_bitmap(ArtId::Information, ArtClient::Menu, Some(Size::new(16, 16))).unwrap();
    image_list.add_bitmap(&info_icon);
    let question_icon = ArtProvider::get_bitmap(ArtId::Removable, ArtClient::Menu, Some(Size::new(16, 16))).unwrap();
    image_list.add_bitmap(&question_icon);
    let goup_icon = ArtProvider::get_bitmap(ArtId::GoUp, ArtClient::Menu, Some(Size::new(16, 16))).unwrap();
    image_list.add_bitmap(&goup_icon);
    let addbookmark_icon = ArtProvider::get_bitmap(ArtId::AddBookmark, ArtClient::Menu, Some(Size::new(16, 16))).unwrap();
    image_list.add_bitmap(&addbookmark_icon);

    notebook.set_image_list(image_list);

    // Add tabs to notebook
    notebook.add_page(&common_panel, "Common", true, Some(0));
    notebook.add_page(&overtls_panel, "OverTLS", false, Some(1));
    notebook.add_page(&tun2proxy_panel, "Tun2proxy", false, Some(1));
    notebook.add_page(&httpproxy_panel, "HttpProxy", false, Some(2));
    notebook.add_page(&logging_panel, "Logging", false, Some(3));

    // OK & Cancel buttons
    let ok_button = Button::builder(&panel).with_label("OK").with_id(ID_OK).build();
    let cancel_button = Button::builder(&panel).with_label("Cancel").with_id(ID_CANCEL).build();
    let dialog_clone = dialog.clone();
    let save_result_for_ok = save_result.clone();
    ok_button.on_click(move |_data| {
        save_result_for_ok.store(true, std::sync::atomic::Ordering::SeqCst);
        dialog_clone.end_modal(ID_OK);
    });

    // Layout the panel content
    let panel_sizer = BoxSizer::builder(Orientation::Vertical).build();
    panel_sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let btn_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    btn_sizer.add(&cancel_button, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    btn_sizer.add(&ok_button, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    panel_sizer.add_sizer(&btn_sizer, 0, SizerFlag::AlignCentre | SizerFlag::All, 0);
    panel.set_sizer(panel_sizer, true);

    let cfg_for_panel_destroy = cfg.clone();
    let save_result_for_panel_destroy = save_result.clone();
    panel.on_destroy(move |_evt| {
        if save_result_for_panel_destroy.load(std::sync::atomic::Ordering::SeqCst) {
            let cfg = cfg_for_panel_destroy.clone();

            wxdragon::call_after(Box::new(move || {
                let run_as_admin = cfg.lock().unwrap().run_as_admin.unwrap_or_default();
                log::info!("Settings panel destroyed, settings committed. Run as admin: {run_as_admin}");

                if run_as_admin && !run_as::is_elevated() {
                    // Persist the latest settings prior to restart
                    save_settings(&cfg.lock().unwrap());
                    if let Ok(status) = crate::restart_as_admin() {
                        log::debug!("Restarted as admin with status code {status}, exiting current instance.");
                        ::std::process::exit(status.code().unwrap_or_default());
                    }
                }
            }));
        }
    });

    // Layout the dialog
    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    // Show the dialog modally
    let result = dialog.show_modal();
    dialog.destroy();
    log::info!("Dialog returned: {result}");
    if result == ID_OK {
        log::info!("Settings dialog confirmed with OK.");
    }
}

fn create_overtls_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>, save_result: Arc<AtomicBool>) -> Panel {
    let panel = Panel::builder(parent).build();

    // Label size for alignment
    let label_size = Size::new(150, -1);

    let mut over_tls_settings = cfg.lock().unwrap().over_tls.clone().unwrap_or_default();

    // Listen Host
    let host_label = StaticText::builder(&panel)
        .with_label("Listen Host:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let host_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    host_input.set_value(&over_tls_settings.listen_host);

    // Listen Port
    let port_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Listen Port:")
        .with_size(label_size)
        .build();
    let port_input = SpinCtrl::builder(&panel)
        .with_initial_value(over_tls_settings.listen_port as i32)
        .with_min_value(1)
        .with_max_value(u16::MAX as i32)
        .build();

    // Listen User
    let user_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Listen User:")
        .with_size(label_size)
        .build();
    let user_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    if let Some(user) = &over_tls_settings.listen_user {
        user_input.set_value(user);
    }

    // Listen Password
    let password_label = StaticText::builder(&panel)
        .with_label("Listen Password:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let password_input = TextCtrl::builder(&panel)
        .with_size(Size::new(200, -1))
        .with_style(TextCtrlStyle::Password)
        .build();
    if let Some(password) = &over_tls_settings.listen_password {
        password_input.set_value(password);
    }

    // Connection Pool Max Size
    let pool_label = StaticText::builder(&panel)
        .with_label("Connection Pool Max Size:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let pool_input = SpinCtrl::builder(&panel)
        .with_initial_value(over_tls_settings.pool_max_size as i32)
        .with_min_value(10)
        .with_max_value(10000)
        .with_size(Size::new(100, -1))
        .build();

    // Cache DNS Label + CheckBox
    let cache_dns_label = StaticText::builder(&panel)
        .with_label("    ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let cache_dns_checkbox = CheckBox::builder(&panel)
        .with_value(over_tls_settings.cache_dns)
        .with_label("Cache DNS")
        .build();

    // Using FlexGridSizer for proper left-right alignment
    let grid = FlexGridSizer::builder(7, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&host_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&host_input, 0, SizerFlag::Expand, 0);
    grid.add(&port_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&port_input, 0, SizerFlag::Expand, 0);
    grid.add(&user_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&user_input, 0, SizerFlag::Expand, 0);
    grid.add(&password_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&password_input, 0, SizerFlag::Expand, 0);
    grid.add(&pool_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&pool_input, 0, SizerFlag::Expand, 0);
    grid.add(&cache_dns_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&cache_dns_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let cfg = cfg.clone();
    panel.on_destroy(move |_evt| {
        if save_result.load(std::sync::atomic::Ordering::SeqCst) {
            over_tls_settings.listen_host = host_input.get_value();
            over_tls_settings.listen_port = port_input.value() as u16;
            over_tls_settings.listen_user = {
                let val = user_input.get_value();
                if val.is_empty() { None } else { Some(val) }
            };
            over_tls_settings.listen_password = {
                let val = password_input.get_value();
                if val.is_empty() { None } else { Some(val) }
            };
            over_tls_settings.pool_max_size = pool_input.value() as usize;
            over_tls_settings.cache_dns = cache_dns_checkbox.get_value();

            cfg.lock().unwrap().over_tls = Some(over_tls_settings.clone());
        }
    });

    panel
}

fn create_common_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>, save_result: Arc<AtomicBool>) -> Panel {
    let panel = Panel::builder(parent).build();

    // Simple layout: one checkbox for 'Run as administrator'
    let label_size = Size::new(150, -1);
    let spacer_label = StaticText::builder(&panel)
        .with_label("    ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();

    let run_admin_checkbox = CheckBox::builder(&panel)
        .with_label("Run as administrator (root) privileges")
        .with_value(cfg.lock().unwrap().run_as_admin.unwrap_or(false))
        .build();

    let grid = FlexGridSizer::builder(1, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&spacer_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&run_admin_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let cfg = cfg.clone();
    panel.on_destroy(move |_evt| {
        if save_result.load(std::sync::atomic::Ordering::SeqCst) {
            cfg.lock().unwrap().run_as_admin = if run_admin_checkbox.get_value() { Some(true) } else { None };
        }
    });

    panel
}

fn create_tun2proxy_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>, save_result: Arc<AtomicBool>) -> Panel {
    let panel = Panel::builder(parent).build();

    let label_size = Size::new(150, -1);

    let mut tun2proxy_settings = cfg.lock().unwrap().tun2proxy.clone().unwrap_or_default();

    // Exit on Fatal Error
    let exit_label = StaticText::builder(&panel)
        .with_label("   ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let exit_checkbox = CheckBox::builder(&panel)
        .with_value(tun2proxy_settings.exit_on_fatal_error)
        .with_label("Exit on Fatal Error")
        .build();

    // Max Sessions
    let max_sessions_label = StaticText::builder(&panel)
        .with_label("Max Sessions:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let max_sessions_input = SpinCtrl::builder(&panel)
        .with_initial_value(tun2proxy_settings.max_sessions as i32)
        .with_min_value(1)
        .with_max_value(10000)
        .build();

    // Remote DNS Address
    let dns_addr_label = StaticText::builder(&panel)
        .with_label("Remote DNS Address:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let dns_addr_input = TextCtrl::builder(&panel)
        .with_size(Size::new(200, -1))
        .with_value(&tun2proxy_settings.dns_address)
        .build();

    // DNS Strategy (dropdown)
    let dns_strategy_label = StaticText::builder(&panel)
        .with_label("DNS Strategy:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let dns_strategy_choices = vec!["virtual", "over-tcp", "direct"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<String>>();
    let dns_strategy_selection = match tun2proxy_settings.dns_strategy.as_str() {
        "virtual" => Some(0),
        "over-tcp" => Some(1),
        "direct" => Some(2),
        _ => Some(1),
    };
    let dns_strategy_choice = Choice::builder(&panel)
        .with_choices(dns_strategy_choices)
        .with_selection(dns_strategy_selection)
        .with_size(Size::new(200, -1))
        .build();

    // Using FlexGridSizer for proper left-right alignment
    let grid = FlexGridSizer::builder(5, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&exit_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&exit_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);
    grid.add(&max_sessions_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&max_sessions_input, 0, SizerFlag::Expand, 0);
    grid.add(&dns_addr_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&dns_addr_input, 0, SizerFlag::Expand, 0);
    grid.add(&dns_strategy_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&dns_strategy_choice, 0, SizerFlag::Expand, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let cfg = cfg.clone();
    panel.on_destroy(move |_evt| {
        if save_result.load(std::sync::atomic::Ordering::SeqCst) {
            tun2proxy_settings.exit_on_fatal_error = exit_checkbox.get_value();
            tun2proxy_settings.max_sessions = max_sessions_input.value() as usize;
            tun2proxy_settings.dns_address = dns_addr_input.get_value();
            tun2proxy_settings.dns_strategy = match dns_strategy_choice.get_selection() {
                Some(0) => "virtual".to_string(),
                Some(1) => "over-tcp".to_string(),
                Some(2) => "direct".to_string(),
                _ => "over-tcp".to_string(),
            };

            cfg.lock().unwrap().tun2proxy = Some(tun2proxy_settings.clone());
        }
    });

    panel
}

fn create_httpproxy_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>, save_result: Arc<AtomicBool>) -> Panel {
    let http_proxy_settings = cfg.lock().unwrap().http_proxy.clone().unwrap_or_default();

    let panel = Panel::builder(parent).build();

    let label_size = Size::new(170, -1);

    // Source Type (dropdown)
    let source_type_label = StaticText::builder(&panel)
        .with_label("Source Type:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let source_type_choices = vec!["http", "socks5"].into_iter().map(String::from).collect::<Vec<String>>();
    let source_type_choice = Choice::builder(&panel)
        .with_choices(source_type_choices)
        .with_selection(Some(0))
        .with_size(Size::new(120, -1))
        .build();
    source_type_choice.enable(false);

    // Listen Addr
    let listen_addr_label = StaticText::builder(&panel)
        .with_label("Listen Addr:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let listen_addr_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    listen_addr_input.set_value(&http_proxy_settings.listen_address_port);

    // Server Addr
    let server_addr_label = StaticText::builder(&panel)
        .with_label("SOCKS5 Server Addr:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let server_addr_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    server_addr_input.set_value(&http_proxy_settings.s5_server_address_port);

    // Username
    let username_label = StaticText::builder(&panel)
        .with_label("Username:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let username_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    username_input.set_value(&http_proxy_settings.username.unwrap_or_default());

    // Password
    let password_label = StaticText::builder(&panel)
        .with_label("Password:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let password_input = TextCtrl::builder(&panel)
        .with_size(Size::new(200, -1))
        .with_style(TextCtrlStyle::Password)
        .build();
    password_input.set_value(&http_proxy_settings.password.unwrap_or_default());

    let grid = FlexGridSizer::builder(6, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&source_type_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&source_type_choice, 0, SizerFlag::Expand, 0);
    grid.add(&listen_addr_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&listen_addr_input, 0, SizerFlag::Expand, 0);
    grid.add(&server_addr_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&server_addr_input, 0, SizerFlag::Expand, 0);
    grid.add(&username_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&username_input, 0, SizerFlag::Expand, 0);
    grid.add(&password_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&password_input, 0, SizerFlag::Expand, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let cfg = cfg.clone();
    panel.on_destroy(move |_evt| {
        if save_result.load(std::sync::atomic::Ordering::SeqCst) {
            let new_settings = crate::settings::HttpProxySettings {
                listen_address_port: listen_addr_input.get_value(),
                s5_server_address_port: server_addr_input.get_value(),
                username: {
                    let val = username_input.get_value();
                    if val.is_empty() { None } else { Some(val) }
                },
                password: {
                    let val = password_input.get_value();
                    if val.is_empty() { None } else { Some(val) }
                },
            };
            cfg.lock().unwrap().http_proxy = Some(new_settings);
        }
    });

    panel
}

fn create_logging_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>, save_result: Arc<AtomicBool>) -> Panel {
    let logging_settings = cfg.lock().unwrap().logging.clone().unwrap_or_default();

    let panel = Panel::builder(parent).build();

    let label_size = Size::new(180, -1);
    let choice_size = Size::new(200, -1);
    let log_levels = vec!["Off", "Error", "Warn", "Info", "Debug", "Trace"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<String>>();

    let global_label = StaticText::builder(&panel)
        .with_label("Global Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();

    let global_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .global_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let rustls_label = StaticText::builder(&panel)
        .with_label("Rustls Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let rustls_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .rustls_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let tokio_label = StaticText::builder(&panel)
        .with_label("Tokio_tungstenite Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let tokio_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .tokio_tungstenite_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let tungstenite_label = StaticText::builder(&panel)
        .with_label("Tungstenite Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let tungstenite_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .tungstenite_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let ipstack_label = StaticText::builder(&panel)
        .with_label("Ipstack Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let ipstack_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .ipstack_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let overtls_label = StaticText::builder(&panel)
        .with_label("OverTls Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let overtls_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .overtls_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    let tun2proxy_label = StaticText::builder(&panel)
        .with_label("Tun2proxy Log Level:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let tun2proxy_choice = Choice::builder(&panel)
        .with_choices(log_levels.clone())
        .with_selection(
            logging_settings
                .tun2proxy_log_level
                .as_ref()
                .and_then(|s| log_levels.iter().position(|x| x == s).map(|i| i as u32)),
        )
        .with_size(choice_size)
        .build();

    // Log Auto Scroll
    let auto_scroll_label = StaticText::builder(&panel)
        .with_label("    ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let auto_scroll_checkbox = CheckBox::builder(&panel)
        .with_value(logging_settings.log_auto_scroll.unwrap_or_default())
        .with_label("Log Auto Scroll")
        .build();

    let grid = FlexGridSizer::builder(8, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&global_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&global_choice, 0, SizerFlag::Expand, 0);
    grid.add(&rustls_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&rustls_choice, 0, SizerFlag::Expand, 0);
    grid.add(&tokio_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&tokio_choice, 0, SizerFlag::Expand, 0);
    grid.add(&tungstenite_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&tungstenite_choice, 0, SizerFlag::Expand, 0);
    grid.add(&ipstack_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&ipstack_choice, 0, SizerFlag::Expand, 0);
    grid.add(&overtls_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&overtls_choice, 0, SizerFlag::Expand, 0);
    grid.add(&tun2proxy_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&tun2proxy_choice, 0, SizerFlag::Expand, 0);
    grid.add(&auto_scroll_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&auto_scroll_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let cfg = cfg.clone();
    panel.on_destroy(move |_evt| {
        if save_result.load(std::sync::atomic::Ordering::SeqCst) {
            let new_settings = crate::settings::LoggingSettings {
                global_log_level: global_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                rustls_log_level: rustls_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                tokio_tungstenite_log_level: tokio_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                tungstenite_log_level: tungstenite_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                ipstack_log_level: ipstack_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                overtls_log_level: overtls_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                tun2proxy_log_level: tun2proxy_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
                log_auto_scroll: if auto_scroll_checkbox.get_value() { Some(true) } else { None },
            };
            cfg.lock().unwrap().logging = Some(new_settings);
        }
    });

    panel
}
