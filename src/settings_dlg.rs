use crate::settings::{
    AppSettings, ICON_SIZE, LocalServerSettings, LoggingSettings, MAIN_ICON, center_rect, create_bitmap_from_memory, save_settings,
};
use std::sync::{Arc, Mutex};
use wxdragon::prelude::*;

pub fn settings_dlg(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>) {
    let (w, h) = (600, 500);
    let (x, y) = center_rect(parent, w, h);

    // Create a generic dialog using the new builder
    let dialog = Dialog::builder(parent, "Settings")
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

    let tun2proxy_settings = cfg.lock().unwrap().tun2proxy.clone().unwrap_or_default();

    // Create tab pages and their readers (each page can return its own struct)
    let (common_panel, common_read) = create_common_tab(&notebook, cfg);
    let (local_panel, local_read) = create_local_settings_tab(&notebook, cfg);
    let (tun2proxy_panel, tun2proxy_read) = create_tun2proxy_tab(&notebook, &tun2proxy_settings);
    let (logging_panel, logging_read) = create_logging_tab(&notebook, cfg);

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
    notebook.add_page(&local_panel, "Local Server", false, Some(1));
    notebook.add_page(&tun2proxy_panel, "Tun2proxy", false, Some(1));
    notebook.add_page(&logging_panel, "Logging", false, Some(2));

    // OK & Cancel buttons
    let ok_button = Button::builder(&panel).with_label("OK").with_id(ID_OK).build();
    let cancel_button = Button::builder(&panel).with_label("Cancel").with_id(ID_CANCEL).build();
    let dialog_clone = dialog;
    ok_button.on_click(move |_data| {
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

    // Layout the dialog
    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    // Show the dialog modally
    let result = dialog.show_modal();
    log::info!("Dialog returned: {result}");
    if result == ID_OK {
        log::info!("Settings dialog confirmed with OK.");

        // Read values from each page and update config
        let mut cfg_lock = cfg.lock().unwrap();

        // Common
        let (run_as_admin_checked, refresh_interval_minutes, single_instance_port) = common_read();
        cfg_lock.run_as_admin = if run_as_admin_checked { Some(true) } else { None };
        cfg_lock.subscription_refresh_interval_minutes = Some(refresh_interval_minutes);
        cfg_lock.single_instance_port = Some(single_instance_port);

        // Local Server
        let new_local: LocalServerSettings = local_read();
        cfg_lock.local_settings = Some(new_local);

        // Tun2proxy
        let new_tun2proxy: tun2proxy::Args = tun2proxy_read();
        cfg_lock.tun2proxy = Some(new_tun2proxy);

        // Logging (also mark if changed)
        let prev_logging = cfg_lock.logging.clone().unwrap_or_default();
        let new_logging: LoggingSettings = logging_read();
        let logging_changed = !new_logging.is_log_level_equal(&prev_logging);
        cfg_lock.logging = Some(new_logging);

        drop(cfg_lock);

        let run_as_admin = cfg.lock().unwrap().run_as_admin.unwrap_or_default();
        log::info!("Settings panel destroyed, settings committed. Run as admin: {run_as_admin}");

        if run_as_admin && !run_as::is_elevated() {
            // Persist the latest settings prior to restart
            save_settings(&cfg.lock().unwrap());
            let mut s = 0;
            if let Ok(status) = crate::restart_as_admin() {
                log::debug!("Restarted as admin with status code {status}, exiting current instance.");
                s = status.code().unwrap_or_default();
            }
            ::std::process::exit(s);
        } else if logging_changed {
            save_settings(&cfg.lock().unwrap());

            // Restart Required, Logging level changes will take effect after restart.
            let dlg = MessageDialog::builder(
                parent,
                "Logging level changes require application restart to take effect. Restart now?",
                "Restart Required",
            )
            .build();
            let _ = dlg.show_modal();
            dlg.destroy();

            let mut s = 0;
            if let Ok(Some(status)) = run_as::restart_self(None, false) {
                s = status.code().unwrap_or_default();
            }
            ::std::process::exit(s);
        } else {
            save_settings(&cfg.lock().unwrap());
        }
    }
}

fn create_local_settings_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>) -> (Panel, impl Fn() -> LocalServerSettings + 'static) {
    let panel = Panel::builder(parent).build();

    // Label size for alignment
    let label_size = Size::new(150, -1);

    let local_settings = cfg.lock().unwrap().local_settings.clone().unwrap_or_default();

    // Listen Host
    let host_label = StaticText::builder(&panel)
        .with_label("Listen Host:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let host_input = TextCtrl::builder(&panel).with_size(Size::new(200, -1)).build();
    host_input.set_value(&local_settings.listen_host);

    // Listen Port
    let port_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Listen Port:")
        .with_size(label_size)
        .build();
    let port_input = SpinCtrl::builder(&panel)
        .with_initial_value(local_settings.listen_port as i32)
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
    if let Some(user) = &local_settings.listen_user {
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
    if let Some(password) = &local_settings.listen_password {
        password_input.set_value(password);
    }

    // Connection Pool Max Size
    let pool_label = StaticText::builder(&panel)
        .with_label("Connection Pool Max Size:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let pool_input = SpinCtrl::builder(&panel)
        .with_initial_value(local_settings.pool_max_size as i32)
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
        .with_value(local_settings.cache_dns)
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

    // Build a reader closure to return OverTlsSettings from current inputs
    let reader = {
        move || LocalServerSettings {
            listen_host: host_input.get_value(),
            listen_port: port_input.value() as u16,
            listen_user: {
                let val = user_input.get_value();
                if val.is_empty() { None } else { Some(val) }
            },
            listen_password: {
                let val = password_input.get_value();
                if val.is_empty() { None } else { Some(val) }
            },
            pool_max_size: pool_input.value() as usize,
            cache_dns: cache_dns_checkbox.get_value(),
        }
    };

    (panel, reader)
}

fn create_common_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>) -> (Panel, impl Fn() -> (bool, u64, u16) + 'static) {
    let panel = Panel::builder(parent).build();

    // Common settings: run as admin and subscription refresh interval
    let label_size = Size::new(260, -1);
    let spacer_label = StaticText::builder(&panel)
        .with_label("    ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(Size::new(150, -1))
        .build();

    let run_admin_checkbox = CheckBox::builder(&panel)
        .with_label("Run as administrator (root) privileges")
        .with_value(cfg.lock().unwrap().run_as_admin.unwrap_or(false))
        .build();

    let refresh_interval_label = StaticText::builder(&panel)
        .with_label("Interval for automatically\nrefreshing subscriptions (minutes)")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();

    let refresh_interval_input = SpinCtrl::builder(&panel)
        .with_initial_value(cfg.lock().unwrap().subscription_refresh_interval_minutes.unwrap_or(10) as i32)
        .with_min_value(1)
        .with_max_value(1440)
        .with_size(Size::new(160, -1))
        .build();

    let single_instance_port_label = StaticText::builder(&panel)
        .with_label("Single instance listening port:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();

    let single_instance_port_input = SpinCtrl::builder(&panel)
        .with_initial_value(
            cfg.lock()
                .unwrap()
                .single_instance_port
                .unwrap_or(crate::single_instance::DEFAULT_SINGLE_INSTANCE_LISTEN_PORT) as i32,
        )
        .with_min_value(1)
        .with_max_value(u16::MAX as i32)
        .with_size(Size::new(160, -1))
        .build();

    let grid = FlexGridSizer::builder(3, 2).with_vgap(20).with_hgap(16).build();
    grid.add(&spacer_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&run_admin_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);
    let flag = SizerFlag::AlignRight | SizerFlag::AlignCenterVertical;
    grid.add(&refresh_interval_label, 0, flag, 0);
    grid.add(&refresh_interval_input, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);
    grid.add(&single_instance_port_label, 0, flag, 0);
    grid.add(
        &single_instance_port_input,
        0,
        SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical,
        0,
    );

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    // Reader returns the values currently shown on the Common tab.
    let reader = {
        move || {
            (
                run_admin_checkbox.get_value(),
                refresh_interval_input.value() as u64,
                single_instance_port_input.value() as u16,
            )
        }
    };

    (panel, reader)
}

fn create_tun2proxy_tab(parent: &dyn WxWidget, tun2proxy_settings: &tun2proxy::Args) -> (Panel, impl Fn() -> tun2proxy::Args + 'static) {
    let panel = Panel::builder(parent).build();

    let label_size = Size::new(150, -1);

    let default_socks5_addr = tun2proxy_settings.proxy.addr.to_string();

    // Target SOCKS5 Address:Port
    let socks5_addr_label = StaticText::builder(&panel)
        .with_label("Target SOCKS5 Address:Port:")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let socks5_addr_input = TextCtrl::builder(&panel)
        .with_size(Size::new(200, -1))
        .with_value(&default_socks5_addr)
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
        .with_value(&tun2proxy_settings.dns_addr.to_string())
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
    let dns_strategy_selection = Some(tun2proxy_settings.dns as u32);
    let dns_strategy_choice = Choice::builder(&panel)
        .with_choices(dns_strategy_choices)
        .with_selection(dns_strategy_selection)
        .with_size(Size::new(200, -1))
        .build();

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

    // Bypass list label and multiline text control
    let bypass_label = StaticText::builder(&panel)
        .with_label("Bypass IPs (CIDR):")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let bypass_input = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(200, 80))
        .build();
    // populate initial bypass lines
    if !tun2proxy_settings.bypass.is_empty() {
        let text = tun2proxy_settings
            .bypass
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        bypass_input.set_value(&text);
    }

    // Using FlexGridSizer for proper left-right alignment
    let grid = FlexGridSizer::builder(7, 2).with_vgap(10).with_hgap(16).build();
    grid.add(&socks5_addr_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&socks5_addr_input, 0, SizerFlag::Expand, 0);
    grid.add(&max_sessions_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&max_sessions_input, 0, SizerFlag::Expand, 0);
    grid.add(&dns_addr_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&dns_addr_input, 0, SizerFlag::Expand, 0);
    grid.add(&dns_strategy_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&dns_strategy_choice, 0, SizerFlag::Expand, 0);
    grid.add(&bypass_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&bypass_input, 0, SizerFlag::Expand, 0);
    grid.add(&exit_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&exit_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    let reader = {
        move || {
            let sel = dns_strategy_choice.get_selection().unwrap_or(1);
            let dns = match sel {
                0 => tun2proxy::ArgDns::Virtual,
                1 => tun2proxy::ArgDns::OverTcp,
                2 => tun2proxy::ArgDns::Direct,
                _ => tun2proxy::ArgDns::OverTcp,
            };

            // Apply Target SOCKS5 address:port into proxy settings
            let target_addr: std::net::SocketAddr = socks5_addr_input.get_value().parse().unwrap();
            let proxy = tun2proxy::ArgProxy {
                proxy_type: tun2proxy::ProxyType::Socks5,
                addr: target_addr,
                ..Default::default()
            };

            // parse bypass list from multiline text area
            let bypass_vec = bypass_input
                .get_value()
                .lines()
                .filter_map(|line| {
                    let t = line.trim();
                    if t.is_empty() {
                        None
                    } else {
                        match t.parse() {
                            Ok(cid) => Some(cid),
                            Err(_) => {
                                log::warn!("Invalid CIDR entered in bypass list: {}", t);
                                None
                            }
                        }
                    }
                })
                .collect::<Vec<_>>();

            tun2proxy::Args {
                proxy,
                exit_on_fatal_error: exit_checkbox.get_value(),
                max_sessions: max_sessions_input.value() as usize,
                dns_addr: dns_addr_input.get_value().parse().unwrap(),
                dns,
                bypass: bypass_vec,
                ..Default::default()
            }
        }
    };

    (panel, reader)
}

fn create_logging_tab(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>) -> (Panel, impl Fn() -> LoggingSettings + 'static) {
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

    let color_output_label = StaticText::builder(&panel)
        .with_label("    ")
        .with_style(StaticTextStyle::AlignRight)
        .with_size(label_size)
        .build();
    let color_output_checkbox = CheckBox::builder(&panel)
        .with_value(logging_settings.log_color_output.unwrap_or_default())
        .with_label("Colored log output")
        .build();

    let grid = FlexGridSizer::builder(9, 2).with_vgap(10).with_hgap(16).build();
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
    grid.add(&color_output_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&color_output_checkbox, 0, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add_sizer(&grid, 0, SizerFlag::Expand | SizerFlag::All, 16);
    panel.set_sizer(sizer, true);

    // Reader closure for LoggingSettings
    let reader = {
        let log_levels = log_levels.clone();
        move || LoggingSettings {
            global_log_level: global_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            rustls_log_level: rustls_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            tokio_tungstenite_log_level: tokio_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            tungstenite_log_level: tungstenite_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            ipstack_log_level: ipstack_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            overtls_log_level: overtls_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            tun2proxy_log_level: tun2proxy_choice.get_selection().and_then(|i| log_levels.get(i as usize).cloned()),
            log_auto_scroll: if auto_scroll_checkbox.get_value() { Some(true) } else { None },
            log_color_output: if color_output_checkbox.get_value() { Some(true) } else { None },
        }
    };

    (panel, reader)
}
