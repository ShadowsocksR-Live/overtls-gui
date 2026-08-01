use crate::settings::{AnyTlsNode, ICON_SIZE, MAIN_ICON, NodeType, OverTlsNode, any_tls_node, center_rect, over_tls_node};
use crate::{ServerNode, settings::create_bitmap_from_memory};
use wxdragon::prelude::*;

/// Show details dialog.
/// - If `node_opt` is provided, controls are initialized from it (edit mode).
/// - Returns Some(ServerNode) when OK, or None when cancelled.
pub fn details_dlg(parent: &dyn WxWidget, node_opt: Option<&ServerNode>) -> Option<ServerNode> {
    let (w, h) = (600, 400);
    let (x, y) = center_rect(parent, w, h);

    let title = if let Some(n) = node_opt {
        format!("Node details of \"{}\"", n.title())
    } else {
        "New Node Details".to_string()
    };

    let dialog = Dialog::builder(parent, &title)
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_position(x, y)
        .with_size(w, h)
        .build();

    let icon_bitmap = create_bitmap_from_memory(MAIN_ICON, Some((ICON_SIZE, ICON_SIZE))).unwrap();
    dialog.set_icon(&icon_bitmap);

    let left_width = 140;
    let right_width = w - left_width - 10;
    let panel: Panel = Panel::builder(&dialog).build();
    let label_size = Size::new(left_width, -1);
    let input_size = Size::new(right_width, -1);

    let type_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Type")
        .with_size(label_size)
        .build();
    let selected_type = node_opt.map(|node| node.node_type()).unwrap_or_default();
    let type_choice = Choice::builder(&panel)
        .with_choices(NodeType::choice_labels())
        .with_selection(Some(selected_type.index()))
        .with_size(input_size)
        .build();

    let remarks_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Remarks")
        .with_size(label_size)
        .build();
    let remarks_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let tunnel_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Tunnel Path")
        .with_size(label_size)
        .build();
    let tunnel_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let disable_tls_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("")
        .with_size(label_size)
        .build();
    let disable_tls_checkbox = CheckBox::builder(&panel).with_size(input_size).with_label("Disable TLS").build();

    let client_id_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Client ID")
        .with_size(label_size)
        .build();
    let client_id_input: TextCtrl = TextCtrl::builder(&panel).with_size(input_size).build();

    let server_host_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Server Host")
        .with_size(label_size)
        .build();
    let server_host_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let server_port_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Server Port")
        .with_size(label_size)
        .build();
    let server_port_input = SpinCtrl::builder(&panel)
        .with_size(input_size)
        .with_initial_value(443)
        .with_min_value(1)
        .with_max_value(u16::MAX as i32)
        .build();

    let server_domain_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Server Domain")
        .with_size(label_size)
        .build();
    let server_domain_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let ca_file_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("CA File/Content")
        .with_size(label_size)
        .build();
    let ca_file_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let dangerous_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("")
        .with_size(label_size)
        .build();
    let dangerous_checkbox = CheckBox::builder(&panel).with_size(input_size).with_label("Dangerous Mode").build();

    let anytls_password_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("Password")
        .with_size(label_size)
        .build();
    let anytls_password_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let anytls_sni_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("SNI")
        .with_size(label_size)
        .build();
    let anytls_sni_input = TextCtrl::builder(&panel).with_size(input_size).with_value("").build();

    let anytls_insecure_label = StaticText::builder(&panel)
        .with_style(StaticTextStyle::AlignRight)
        .with_label("")
        .with_size(label_size)
        .build();
    let anytls_insecure_chkbox = CheckBox::builder(&panel).with_size(input_size).with_label("Insecure TLS").build();

    let grid = FlexGridSizer::builder(13, 2).with_vgap(8).with_hgap(10).build();
    grid.add(&type_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&type_choice, 1, SizerFlag::Expand, 0);
    grid.add(&remarks_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&remarks_input, 1, SizerFlag::Expand, 0);
    grid.add(&server_host_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&server_host_input, 1, SizerFlag::Expand, 0);
    grid.add(&server_port_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&server_port_input, 1, SizerFlag::Expand, 0);
    grid.add(&client_id_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&client_id_input, 1, SizerFlag::Expand, 0);
    grid.add(&tunnel_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&tunnel_input, 1, SizerFlag::Expand, 0);
    grid.add(&disable_tls_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&disable_tls_checkbox, 1, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);
    grid.add(&server_domain_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&server_domain_input, 1, SizerFlag::Expand, 0);
    grid.add(&ca_file_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&ca_file_input, 1, SizerFlag::Expand, 0);
    grid.add(&dangerous_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&dangerous_checkbox, 1, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);
    grid.add(&anytls_password_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&anytls_password_input, 1, SizerFlag::Expand, 0);
    grid.add(&anytls_sni_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&anytls_sni_input, 1, SizerFlag::Expand, 0);
    grid.add(&anytls_insecure_label, 0, SizerFlag::AlignRight | SizerFlag::AlignCenterVertical, 0);
    grid.add(&anytls_insecure_chkbox, 1, SizerFlag::AlignLeft | SizerFlag::AlignCenterVertical, 0);

    let submit_btn = Button::builder(&panel).with_label("Submit").build();
    let cancel_btn = Button::builder(&panel).with_label("Cancel").with_id(ID_CANCEL).build();
    let dialog_clone = dialog;
    submit_btn.on_click(move |_data| {
        dialog_clone.end_modal(ID_OK);
    });
    let dialog_clone2 = dialog;
    cancel_btn.on_click(move |_data| {
        dialog_clone2.end_modal(ID_CANCEL);
    });

    let panel_sizer = BoxSizer::builder(Orientation::Vertical).build();
    panel_sizer.add_sizer(&grid, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let btn_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    btn_sizer.add(&cancel_btn, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    btn_sizer.add(&submit_btn, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    panel_sizer.add_sizer(&btn_sizer, 0, SizerFlag::AlignCentre | SizerFlag::All, 0);
    panel.set_sizer(panel_sizer, true);

    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    // Host, port, and client ID are shared; only type-specific controls are switched.
    let initial_type = node_opt.map(|node| node.node_type()).unwrap_or_default();
    let panel_for_type = panel;
    let update_type_visibility = move |node_type: NodeType| {
        let is_anytls = node_type == NodeType::AnyTls;
        tunnel_label.show(!is_anytls);
        tunnel_input.show(!is_anytls);
        disable_tls_label.show(!is_anytls);
        disable_tls_checkbox.show(!is_anytls);
        server_domain_label.show(!is_anytls);
        server_domain_input.show(!is_anytls);
        ca_file_label.show(!is_anytls);
        ca_file_input.show(!is_anytls);
        dangerous_label.show(!is_anytls);
        dangerous_checkbox.show(!is_anytls);
        anytls_password_label.show(is_anytls);
        anytls_password_input.show(is_anytls);
        anytls_sni_label.show(is_anytls);
        anytls_sni_input.show(is_anytls);
        anytls_insecure_label.show(is_anytls);
        anytls_insecure_chkbox.show(is_anytls);
        panel_for_type.layout();
    };
    update_type_visibility(initial_type);
    type_choice.on_selection_changed(move |_event| {
        let selected_type = type_choice.get_selection().and_then(NodeType::from_index).unwrap_or_default();
        update_type_visibility(selected_type);
    });

    // Initialize controls if editing an existing node
    if let Some(node) = node_opt {
        // Remarks
        remarks_input.set_value(&node.title());

        // Tunnel Path
        tunnel_input.set_value(&node.server_secret());

        if let Some(ov_node) = node.downcast_ref::<crate::settings::OverTlsNode>() {
            // Client fields (if present)
            if let Some(c) = ov_node.config.client.as_ref() {
                disable_tls_checkbox.set_value(c.disable_tls.unwrap_or(false));
                client_id_input.set_value(c.client_id.map(|s| s.to_string()).unwrap_or_default().as_str());
                server_host_input.set_value(&c.server_host);
                server_port_input.set_value(c.server_port as i32);
                server_domain_input.set_value(c.server_domain.as_deref().unwrap_or(""));
                ca_file_input.set_value(c.cafile.as_deref().unwrap_or(""));
                dangerous_checkbox.set_value(c.dangerous_mode.unwrap_or(false));
            } else {
                // Defaults when client is None
                disable_tls_checkbox.set_value(false);
                client_id_input.set_value("");
                server_host_input.set_value("");
                server_port_input.set_value(443);
                server_domain_input.set_value("");
                ca_file_input.set_value("");
                dangerous_checkbox.set_value(false);
            }
        } else if let Some(any_node) = node.downcast_ref::<AnyTlsNode>() {
            let config = &any_node.config;
            client_id_input.set_value(config.client_id.map(|id| id.to_string()).unwrap_or_default().as_str());
            server_host_input.set_value(&config.server.host());
            server_port_input.set_value(config.server.port() as i32);
            anytls_password_input.set_value(&config.password);
            anytls_sni_input.set_value(config.sni.as_deref().unwrap_or(""));
            anytls_insecure_chkbox.set_value(config.insecure);
        }
    }

    let result = dialog.show_modal();
    log::info!("Details dialog returned: {}", result);

    let result = if result == ID_OK {
        // Collect values into ServerNode (overtls::Config)
        let remarks = {
            let s = remarks_input.get_value();
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        };
        let selected_type = type_choice.get_selection().and_then(NodeType::from_index).unwrap_or_default();
        let tunnel_path = tunnel_input.get_value();
        let disable_tls = if disable_tls_checkbox.get_value() { Some(true) } else { None };
        let client_id = {
            let s = client_id_input.get_value();
            uuid::Uuid::parse_str(s.trim()).ok()
        };
        let server_host = server_host_input.get_value();
        let server_port = {
            let v = server_port_input.value();
            v.max(0).min(u16::MAX as i32) as u16
        };
        let server_domain = {
            let s = server_domain_input.get_value();
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        };
        let ca_file = {
            let s = ca_file_input.get_value();
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        };
        let dangerous_mode = if dangerous_checkbox.get_value() { Some(true) } else { None };
        match selected_type {
            NodeType::AnyTls => {
                let mut config = node_opt
                    .and_then(|node| node.downcast_ref::<AnyTlsNode>())
                    .map(|node| node.config.clone())
                    .unwrap_or_default();
                config.server = (server_host, server_port).into();
                config.sni = {
                    let value = anytls_sni_input.get_value().trim().to_string();
                    if value.is_empty() { None } else { Some(value) }
                };
                config.password = anytls_password_input.get_value();
                config.client_id = client_id;
                config.insecure = anytls_insecure_chkbox.get_value();
                config.display_name = remarks;
                Some(any_tls_node(config))
            }
            NodeType::OverTls => {
                let mut config = node_opt
                    .and_then(|node| node.downcast_ref::<OverTlsNode>())
                    .map(|node| node.config.clone())
                    .unwrap_or_default();
                config.remarks = remarks;
                config.tunnel_path = overtls::TunnelPath::Single(tunnel_path);
                let mut client = config.client.take().unwrap_or_default();
                client.client_id = client_id;
                client.server_host = server_host;
                client.server_port = server_port;
                client.server_domain = server_domain;
                client.cafile = ca_file;
                client.disable_tls = disable_tls;
                client.dangerous_mode = dangerous_mode;
                config.client = Some(client);
                Some(over_tls_node(config))
            }
        }
    } else {
        None
    };
    dialog.destroy();
    result
}
