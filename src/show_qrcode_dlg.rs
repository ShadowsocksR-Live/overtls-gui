use crate::ServerNode;
use wxdragon::prelude::*;

use crate::settings::{MAIN_ICON, center_rect, create_bitmap_from_memory};

static IMG_WIDTH: u32 = 256;

pub fn show_qrcode_dlg(parent: &dyn WxWidget, node: &ServerNode) -> std::io::Result<()> {
    let (w, h) = (360, 400);
    let (x, y) = center_rect(parent, w, h);

    let title = node
        .remarks
        .as_deref()
        .unwrap_or(node.client.as_ref().map(|c| c.server_host.as_str()).unwrap_or("Unnamed"));

    // Generate the SSR URL for the node and display it as a QR code
    let bmp = if let Ok(ssr_url) = node.generate_ssr_url() {
        // Generate QR Code image
        let code = qrcode::QrCode::new(ssr_url.as_bytes()).map_err(|e| std::io::Error::other(format!("QR code generation error: {e}")))?;
        let img = code.render::<image::Luma<u8>>().min_dimensions(IMG_WIDTH, IMG_WIDTH).build();
        // Convert image::ImageBuffer to PNG bytes
        let mut png_bytes: Vec<u8> = Vec::new();
        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let mut writer = std::io::Cursor::new(&mut png_bytes);
        dyn_img
            .write_to(&mut writer, image::ImageFormat::Png)
            .map_err(|e| std::io::Error::other(format!("Image encoding error: {e}")))?;
        Some(create_bitmap_from_memory(&png_bytes, Some((IMG_WIDTH, IMG_WIDTH)))?)
    } else {
        None
    };

    let dialog = Dialog::builder(parent, &format!("QR Code of node - \"{title}\""))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_position(x, y)
        .with_size(w, h)
        .build();

    let panel = Panel::builder(&dialog).build();

    let icon = create_bitmap_from_memory(MAIN_ICON, Some((IMG_WIDTH, IMG_WIDTH)))
        .unwrap_or_else(|_| Bitmap::new(IMG_WIDTH as i32, IMG_WIDTH as i32).unwrap());
    dialog.set_icon(&icon);

    let bmp_ctrl = StaticBitmap::builder(&panel)
        .with_bitmap(if bmp.is_some() { bmp } else { Some(icon) })
        .with_size(Size::new(IMG_WIDTH as i32, IMG_WIDTH as i32))
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
    Ok(())
}
