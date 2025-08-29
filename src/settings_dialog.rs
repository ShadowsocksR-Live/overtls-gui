use crate::states_manager::SystemSettings;
use fltk::{
    button::{Button, CheckButton},
    enums::Align,
    frame::Frame,
    group::Flex,
    input::Input,
    prelude::{ButtonExt, GroupExt, InputExt, MenuExt, WidgetBase, WidgetExt},
    window::Window,
};

use tun2proxy::{ArgDns, ValueEnum};

// "virtual|over-tcp|direct"
fn tun2proxy_dns_strategy_options() -> String {
    ArgDns::value_variants()
        .iter()
        .map(|v| v.to_possible_value().unwrap().get_name().to_string())
        .collect::<Vec<_>>()
        .join("|")
}

fn tun2proxy_dns_strategy_index(dns: ArgDns) -> usize {
    ArgDns::value_variants().iter().position(|x| *x == dns).unwrap_or(1)
}

fn tun2proxy_dns_strategy_by_index(index: usize) -> ArgDns {
    ArgDns::value_variants().get(index).cloned().unwrap_or(tun2proxy::ArgDns::OverTcp)
}

macro_rules! add_row_input {
    ($label:expr, $input:ident, $flex:expr) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let $input = Input::default();
        row.fixed(&lbl, 200);
        row.fixed(&$input, 360);
        row.end();
        $flex.fixed(&row, 30);
        $input
    }};
}
macro_rules! add_row_check {
    ($label:expr, $check:ident, $flex:expr) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let $check = CheckButton::default();
        row.fixed(&lbl, 200);
        row.fixed(&$check, 360);
        row.end();
        $flex.fixed(&row, 30);
        $check
    }};
}
macro_rules! add_row_choice {
    ($label:expr, $choice:ident, $flex:expr, $options:expr) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let mut $choice = fltk::menu::Choice::default();
        $choice.add_choice($options);
        $choice.set_value(0);
        row.fixed(&lbl, 200);
        row.fixed(&$choice, 360);
        row.end();
        $flex.fixed(&row, 30);
        $choice
    }};
}
macro_rules! add_row_spin {
    ($label:expr, $spin:ident, $flex:expr, $min:expr, $max:expr, $step:expr) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let mut $spin = fltk::misc::Spinner::default();
        $spin.set_minimum($min);
        $spin.set_maximum($max);
        $spin.set_step($step);
        row.fixed(&lbl, 200);
        row.fixed(&$spin, 360);
        row.end();
        $flex.fixed(&row, 30);
        $spin
    }};
}

/// Pop up the settings dialog, and send the result via channel to avoid idle closure accumulation.
pub fn show_settings_dialog(win: &Window, system_settings: &SystemSettings, tx: std::sync::mpsc::Sender<SystemSettings>) {
    let dialog_w = 600;
    let dialog_h = 320;
    let x = win.x() + (win.width() - dialog_w) / 2;
    let y = win.y() + (win.height() - dialog_h) / 2;
    let mut dlg = Window::new(x, y, dialog_w, dialog_h, "Settings");
    let mut tabs = fltk::group::Tabs::new(0, 0, dialog_w, dialog_h, "");
    tabs.set_tab_align(Align::Top);

    // Common Tab
    let tab_common = fltk::group::Group::new(0, 25, dialog_w, dialog_h - 25, "Common");
    let mut flex_common = Flex::default_fill().column();
    flex_common.fixed(&tab_common, dialog_h - 25);

    let mut listen_host = add_row_input!("Listen Host", listen_host, flex_common);
    let mut listen_port = add_row_input!("Listen Port", listen_port, flex_common);
    let mut listen_user = add_row_input!("Listen User", listen_user, flex_common);
    let mut listen_password = add_row_input!("Listen Password", listen_password, flex_common);
    let mut pool_max_size = add_row_input!("Connection Pool Max Size", pool_max_size, flex_common);
    let mut cache_dns = add_row_check!("Cache DNS", cache_dns, flex_common);

    tab_common.end();

    // Tun2proxy Tab
    let tab_tun2proxy = fltk::group::Group::new(0, 25, dialog_w, dialog_h - 25, "Tun2proxy");
    let mut flex_tun2proxy = Flex::default_fill().column();
    flex_tun2proxy.fixed(&tab_tun2proxy, dialog_h - 25);

    let mut tun2proxy_enable = add_row_check!("Enable Tun2proxy", tun2proxy_enable, flex_tun2proxy);
    let mut exit_on_fatal_error = add_row_check!("Exit on Fatal Error", exit_on_fatal_error, flex_tun2proxy);
    let mut max_sessions = add_row_spin!("Max Sessions", max_sessions, flex_tun2proxy, 50.0, 300.0, 1.0);
    let mut remote_dns_address = add_row_input!("Remote DNS Address", remote_dns_address, flex_tun2proxy);
    let mut dns_strategy = add_row_choice!("DNS Strategy", dns_strategy, flex_tun2proxy, &tun2proxy_dns_strategy_options());

    tab_tun2proxy.end();

    tabs.end();

    // ============================= end of tab layouts =============================

    // Set initial values from system_settings
    listen_host.set_value(&system_settings.listen_host);
    listen_port.set_value(&system_settings.listen_port.to_string());
    listen_user.set_value(system_settings.listen_user.as_deref().unwrap_or(""));
    listen_password.set_value(system_settings.listen_password.as_deref().unwrap_or(""));
    pool_max_size.set_value(&system_settings.pool_max_size.to_string());
    cache_dns.set_value(system_settings.cache_dns);

    let tun2proxy_cfg = system_settings.tun2proxy.clone().unwrap_or_default();

    // Tun2proxy default values
    tun2proxy_enable.set_value(system_settings.tun2proxy_enable.unwrap_or_default());
    exit_on_fatal_error.set_value(tun2proxy_cfg.exit_on_fatal_error);
    max_sessions.set_value(tun2proxy_cfg.max_sessions as f64);
    remote_dns_address.set_value(tun2proxy_cfg.dns_addr.to_string().as_str());
    dns_strategy.set_value(tun2proxy_dns_strategy_index(tun2proxy_cfg.dns) as i32);

    let mut submit_btn = Button::new(dialog_w / 2 - 60, dialog_h - 45, 120, 35, "Submit");
    dlg.end();
    dlg.show();

    let mut dlg_cb = dlg.clone();
    submit_btn.set_callback(move |_b| {
        let listen_host_val = listen_host.value();
        let listen_port_val = listen_port.value().parse().unwrap_or(0);
        let listen_user_val = if listen_user.value().is_empty() {
            None
        } else {
            Some(listen_user.value())
        };
        let listen_password_val = if listen_password.value().is_empty() {
            None
        } else {
            Some(listen_password.value())
        };
        let pool_max_size_val = pool_max_size.value().parse().unwrap_or(8);
        let cache_dns_val = cache_dns.value();

        // Tun2proxy Tab values
        let tun2proxy_enable_val = tun2proxy_enable.value();
        dbg!(tun2proxy_enable_val);
        let exit_on_fatal_error_val = exit_on_fatal_error.value();
        let max_sessions_val = max_sessions.value() as usize;
        let remote_dns_address_val = remote_dns_address.value();
        let dns_strategy_val = dns_strategy.value();

        let tun2proxy_cfg = Some(tun2proxy::Args {
            exit_on_fatal_error: exit_on_fatal_error_val,
            max_sessions: max_sessions_val,
            dns: tun2proxy_dns_strategy_by_index(dns_strategy_val as usize),
            dns_addr: remote_dns_address_val.parse().unwrap_or("8.8.8.8".parse().unwrap()),
            ..tun2proxy::Args::default()
        });

        let new_settings = SystemSettings {
            listen_host: listen_host_val,
            listen_port: listen_port_val,
            listen_user: listen_user_val,
            listen_password: listen_password_val,
            pool_max_size: pool_max_size_val,
            cache_dns: cache_dns_val,
            tun2proxy_enable: Some(tun2proxy_enable_val),
            tun2proxy: tun2proxy_cfg,
        };
        let _ = tx.send(new_settings);
        dlg_cb.hide();
    });
}
