use crate::OverTlsNode;
use fltk::{
    button::{Button, CheckButton},
    enums::{Align, Event, Key},
    frame::Frame,
    group::Flex,
    input::Input,
    prelude::{ButtonExt, GroupExt, InputExt, WidgetBase, WidgetExt},
    window::Window,
};
use overtls::{ClientConfig, TunnelPath};

macro_rules! add_row_input {
    ($flex:expr, $label:expr, $input:ident) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let $input = Input::default();
        row.fixed(&lbl, 126);
        row.fixed(&$input, 360);
        row.end();
        $flex.fixed(&row, 30);
        $input
    }};
}
macro_rules! add_row_check {
    ($flex:expr, $label:expr, $check:ident) => {{
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label($label);
        lbl.set_align(Align::Right | Align::Inside);
        let $check = CheckButton::default();
        row.fixed(&lbl, 126);
        row.fixed(&$check, 360);
        row.end();
        $flex.fixed(&row, 30);
        $check
    }};
}

pub fn show_node_details(win: &Window, node_cfg: Option<OverTlsNode>, tx: std::sync::mpsc::Sender<Option<OverTlsNode>>) {
    let dialog_w = 500;
    let dialog_h = 360;
    let x = win.x() + (win.w() - dialog_w) / 2;
    let y = win.y() + (win.h() - dialog_h) / 2;

    let title = match &node_cfg {
        None => "New Node".to_string(),
        Some(cfg) => match &cfg.remarks {
            Some(s) if !s.is_empty() => format!("Node details of '{s}'"),
            _ => "Node without remarks".to_string(),
        },
    };

    let mut dlg = Window::new(x, y, dialog_w, dialog_h, &*title);

    let mut flex = Flex::default_fill().column();
    flex.fixed(&dlg, dialog_h);

    let mut remarks = add_row_input!(flex, "Remarks", remarks);
    let mut tunnel_path = add_row_input!(flex, "Tunnel Path", tunnel_path);
    let mut disable_tls = add_row_check!(flex, "Disable TLS", disable_tls);
    let mut client_id = add_row_input!(flex, "Client ID", client_id);
    let mut server_host = add_row_input!(flex, "Server Host", server_host);
    let mut server_port = add_row_input!(flex, "Server Port", server_port);
    let mut server_domain = add_row_input!(flex, "Server Domain", server_domain);
    let mut cafile = add_row_input!(flex, "CA File/Content", cafile);
    let mut dangerous_mode = add_row_check!(flex, "Dangerous Mode", dangerous_mode);

    if let Some(cfg) = &node_cfg {
        remarks.set_value(cfg.remarks.as_ref().map_or("", |v| v));
        tunnel_path.set_value(cfg.tunnel_path.to_string().as_str());
        if let Some(client) = &cfg.client {
            disable_tls.set_value(client.disable_tls.unwrap_or(false));
            client_id.set_value(client.client_id.as_ref().map_or("", |v| v));
            server_host.set_value(client.server_host.as_str());
            server_port.set_value(&client.server_port.to_string());
            server_domain.set_value(client.server_domain.as_ref().map_or("", |v| v));
            cafile.set_value(client.cafile.as_ref().map_or("", |v| v));
            dangerous_mode.set_value(client.dangerous_mode.unwrap_or(false));
        }
    }

    let mut submit_btn = Button::default().with_label("Submit");
    flex.fixed(&submit_btn, 40);

    dlg.end();
    dlg.show();

    let mut dlg_cb = dlg.clone();
    let tx_cb = tx.clone();
    submit_btn.set_callback(move |_b| {
        let mut client = ClientConfig::default();

        client.disable_tls = Some(disable_tls.value());
        client.client_id = if client_id.value().is_empty() {
            None
        } else {
            Some(client_id.value())
        };
        client.server_host = server_host.value();
        client.server_port = server_port.value().parse().unwrap_or(443);
        client.server_domain = if server_domain.value().is_empty() {
            None
        } else {
            Some(server_domain.value())
        };
        client.cafile = if cafile.value().is_empty() { None } else { Some(cafile.value()) };
        client.dangerous_mode = Some(dangerous_mode.value());

        let config = OverTlsNode {
            remarks: if remarks.value().is_empty() { None } else { Some(remarks.value()) },
            tunnel_path: TunnelPath::Single(tunnel_path.value()),
            client: Some(client),
            ..OverTlsNode::default()
        };
        let _ = tx_cb.send(Some(config));
        dlg_cb.hide();
    });

    // Esc key to close the dialog and return None
    let mut dlg_esc = dlg.clone();
    let tx_esc = tx.clone();
    dlg.handle(move |_, ev| {
        if ev == Event::KeyDown && fltk::app::event_key() == Key::Escape {
            let _ = tx_esc.send(None);
            dlg_esc.hide();
            return true;
        }
        false
    });

    // Closing the dialog using the window's close button
    let mut dlg_close = dlg.clone();
    let tx_close = tx.clone();
    dlg.set_callback(move |_w| {
        let _ = tx_close.send(None);
        dlg_close.hide();
    });
}
