use crate::ServerNode;
use wxdragon::prelude::*;

use crate::settings::{MAIN_ICON, center_rect, create_bitmap_from_memory};

pub fn show_qrcode_dlg(parent: &dyn WxWidget, node: &ServerNode) {
    let (w, h) = (320, 360);
    let (x, y) = center_rect(parent, w, h);

    let title = node
        .remarks
        .as_deref()
        .unwrap_or(node.client.as_ref().map(|c| c.server_host.as_str()).unwrap_or("Unnamed"));

    let dialog = Dialog::builder(parent, &format!("QR Code of node - \"{title}\""))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_position(x, y)
        .with_size(w, h)
        .build();

    let panel = Panel::builder(&dialog).build();

    let bmp = create_bitmap_from_memory(MAIN_ICON, Some((200, 200))).unwrap_or_else(|_| Bitmap::new(200, 200).unwrap());
    dialog.set_icon(&bmp);

    let bmp_ctrl = StaticBitmap::builder(&panel)
        .with_bitmap(Some(bmp))
        .with_size(Size::new(200, 200))
        .build();

    let info_label = StaticText::builder(&panel).with_label("Scan this QR code with your app").build();

    // OK button with id of ID_CANCEL to respond to Esc key
    let ok_btn = Button::builder(&panel).with_label("OK").with_id(ID_CANCEL).build();
    let dialog_clone = dialog.clone();
    ok_btn.on_click(move |_data| {
        dialog_clone.end_modal(ID_OK);
    });

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&info_label, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    sizer.add(&bmp_ctrl, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    sizer.add(&ok_btn, 0, SizerFlag::AlignCentre | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);

    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    let result = dialog.show_modal();
    log::info!("Show QRCode dialog returned: {}", result);
    dialog.destroy();
}
