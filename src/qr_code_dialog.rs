use fltk::{frame::Frame, image::PngImage, prelude::*, window::Window};
use image::Luma;
use qrcode::QrCode;

pub fn qr_code_dialog(parent: &Window, title: &str, ssr_url: &str) -> std::io::Result<()> {
    let dlg_width = 400;
    // Generate QR Code image
    let code = QrCode::new(ssr_url.as_bytes()).map_err(|e| std::io::Error::other(format!("QR code generation error: {e}")))?;
    let img = code.render::<Luma<u8>>().min_dimensions(256, 256).build();
    // Convert image::ImageBuffer to PNG bytes
    use image::DynamicImage;
    use std::io::Cursor;
    let mut png_bytes: Vec<u8> = Vec::new();
    let dyn_img = DynamicImage::ImageLuma8(img);
    use image::ImageFormat;
    let mut writer = Cursor::new(&mut png_bytes);
    dyn_img
        .write_to(&mut writer, ImageFormat::Png)
        .map_err(|e| std::io::Error::other(format!("Image encoding error: {e}")))?;

    let x = parent.x() + (parent.width() - dlg_width) / 2;
    let y = parent.y() + (parent.height() - dlg_width) / 2;
    let mut win = Window::new(x, y, dlg_width, dlg_width, title);
    let mut frame = Frame::new(72, 72, 256, 256, "");
    let png = PngImage::from_data(&png_bytes).map_err(|e| std::io::Error::other(format!("FLTK image error: {e}")))?;
    frame.set_image(Some(png));
    win.end();
    win.show();
    Ok(())
}
