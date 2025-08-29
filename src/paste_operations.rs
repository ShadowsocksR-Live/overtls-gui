use crate::OverTlsNode;
use fltk::{
    input::Input,
    prelude::{ImageExt, InputExt, WidgetBase},
};

pub fn paste() -> std::io::Result<OverTlsNode> {
    if fltk::app::clipboard_contains(fltk::app::ClipboardContent::Text) {
        let text_holder = Input::new(0, 0, 0, 0, None);
        fltk_paste_fix(&text_holder, fltk::app::ClipboardContent::Text);
        let text = text_holder.value();

        log::trace!("Pasted text: {text}");
        // Try to parse the text as a config
        return OverTlsNode::from_json_str(&text)
            .or_else(|_| OverTlsNode::from_ssr_url(&text))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Some unknown error occurred: {e}")));
    }
    if fltk::app::clipboard_contains(fltk::app::ClipboardContent::Image) {
        log::trace!("Pasted image");

        // Bug workaround: even if we need only image, we must call fltk_paste_fix on a dummy widget first
        let dummy = Input::new(0, 0, 0, 0, None);
        fltk_paste_fix(&dummy, fltk::app::ClipboardContent::Image);

        if let Some(img) = fltk::app::event_clipboard_image() {
            return config_from_rgb_image(&img);
        }
    }

    Err(std::io::Error::other("Another paste operations not implemented"))
}

pub fn files_drag_n_drop() -> Vec<OverTlsNode> {
    let mut configs = Vec::new();
    for line in fltk::app::event_text().lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        match process_inputed_file(path) {
            Ok(config) => configs.push(config),
            Err(e) => log::warn!("Failed to process dropped file: {e}"),
        }
    }
    configs
}

fn process_inputed_file(path: &str) -> std::io::Result<OverTlsNode> {
    use std::io::{Error, ErrorKind::InvalidData};
    // 1. try to parse as config file
    if let Ok(config) = OverTlsNode::from_config_file(path) {
        return Ok(config);
    }
    // 2. try to parse as image
    let img = image::open(path).map_err(|e| Error::new(InvalidData, format!("Failed to load file '{path}' as image: {e}")))?;

    // 3. try to parse QR code
    let qr_str = qr_decode(&img).map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code from image '{path}': {e}")))?;

    log::trace!("QR code detected: {qr_str}");
    // 4. try to convert QR code string to config
    let config = OverTlsNode::from_ssr_url(&qr_str).map_err(|e| Error::new(InvalidData, format!("Failed parse '{qr_str}': {e}")))?;

    Ok(config)
}

pub fn screenshot_qr_import() -> std::io::Result<OverTlsNode> {
    let img = screenshot_to_image()?;
    let scr_str = qr_decode(&img)?;
    Ok(OverTlsNode::from_ssr_url(&scr_str)?)
}

fn screenshot_to_image() -> std::io::Result<image::DynamicImage> {
    // Take screenshot of the primary display
    let img = screenshot::get_screenshot(0).map_err(|e| std::io::Error::other(format!("Screenshot failed: {e}")))?;

    // Screenshot struct: data: Vec<u8>, height, width, row_len, pixel_width
    // ARGB format, need to convert to RGBA for image crate
    let width = img.width() as u32;
    let height = img.height() as u32;
    let pixel_width = img.pixel_width();
    let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);
    let data = img.as_ref();
    // BGRA -> RGBA
    for chunk in data.chunks(pixel_width) {
        if chunk.len() >= 4 {
            // BGRA: [b, g, r, a] -> RGBA: [r, g, b, a]
            rgba_buf.push(chunk[2]); // r
            rgba_buf.push(chunk[1]); // g
            rgba_buf.push(chunk[0]); // b
            rgba_buf.push(chunk[3]); // a
        }
    }
    let rgba_img = image::RgbaImage::from_raw(width, height, rgba_buf)
        .ok_or_else(|| std::io::Error::other("Failed to create RGBA image from screenshot"))?;
    let dyn_img = image::DynamicImage::ImageRgba8(rgba_img);
    Ok(dyn_img)
}

fn qr_decode(img: &image::DynamicImage) -> std::io::Result<String> {
    use std::io::{Error, ErrorKind::InvalidData};
    let img = img.to_luma8();
    // Prepare for detection
    let mut img = rqrr::PreparedImage::prepare(img);
    // Search for grids, without decoding
    let grids = img.detect_grids();
    // Decode the grid
    let (meta, content) = grids
        .first()
        .ok_or_else(|| Error::new(InvalidData, "Failed to get QR code grid"))?
        .decode()
        .map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code: {e}")))?;
    log::trace!("QR code meta: {:?}", meta);
    Ok(content)
}

fn fltk_rgb_image_to_dynamic_image(rgb_img: &fltk::image::RgbImage) -> image::DynamicImage {
    let (w, h, d) = (rgb_img.width(), rgb_img.height(), rgb_img.depth());
    let data = rgb_img.to_rgb_data();

    // create RgbaImage
    let mut img_buf = image::RgbaImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * d as i32) as usize;
            let (r, g, b, a) = match d {
                fltk::enums::ColorDepth::L8 => {
                    // grayscale
                    let v = data[idx];
                    (v, v, v, 255)
                }
                fltk::enums::ColorDepth::La8 => {
                    // grayscale + Alpha
                    let v = data[idx];
                    let a = data[idx + 1];
                    (v, v, v, a)
                }
                fltk::enums::ColorDepth::Rgb8 => {
                    // RGB
                    let r = data[idx];
                    let g = data[idx + 1];
                    let b = data[idx + 2];
                    (r, g, b, 255)
                }
                fltk::enums::ColorDepth::Rgba8 => {
                    // RGBA
                    let r = data[idx];
                    let g = data[idx + 1];
                    let b = data[idx + 2];
                    let a = data[idx + 3];
                    (r, g, b, a)
                }
            };
            img_buf.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
        }
    }
    // convert to DynamicImage
    image::DynamicImage::ImageRgba8(img_buf)
}

fn config_from_rgb_image(rgb_img: &fltk::image::RgbImage) -> std::io::Result<OverTlsNode> {
    use std::io::{Error, ErrorKind::InvalidData};
    let dyn_img = fltk_rgb_image_to_dynamic_image(rgb_img);

    // QR parsing
    let qr_str = qr_decode(&dyn_img).map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code: {e}")))?;

    // convert to overtls config
    OverTlsNode::from_ssr_url(&qr_str).map_err(|e| Error::new(InvalidData, format!("Failed parse '{qr_str}': {e}")))
}

/// Fix for FLTK paste operations.
/// This function checks if the clipboard contains the specified content type
/// and performs the paste operation accordingly.
/// It is a workaround for the issue where the `fltk::app::paste(widget)` method
pub fn fltk_paste_fix<T: fltk::prelude::WidgetExt>(widget: &T, k: fltk::app::ClipboardContent) {
    if fltk::app::clipboard_contains(k) {
        match k {
            fltk::app::ClipboardContent::Text => fltk::app::paste_text(widget),
            fltk::app::ClipboardContent::Image => fltk::app::paste_image(widget),
        }
    }
}
