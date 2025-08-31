use crate::OverTlsNode;

pub fn paste() -> std::io::Result<OverTlsNode> {
    // Use arboard::Clipboard for cross-platform clipboard access
    let mut clipboard = arboard::Clipboard::new().map_err(|e| std::io::Error::other(format!("Clipboard error: {e}")))?;

    // Try to get text from clipboard
    if let Ok(text) = clipboard.get_text() {
        log::trace!("Pasted text: {text}");
        // Try to parse the text as a config
        return OverTlsNode::from_json_str(&text)
            .or_else(|_| OverTlsNode::from_ssr_url(&text))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Some unknown error occurred: {e}")));
    }

    // Try to get image from clipboard (requires arboard image-data feature)
    let Ok(img) = clipboard.get_image() else {
        return Err(std::io::Error::other("Another paste operations not implemented"));
    };
    // Convert arboard::ImageData to image::DynamicImage
    let dyn_img = image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
            .ok_or_else(|| std::io::Error::other("Failed to convert clipboard image"))?,
    );
    config_from_image(&dyn_img)
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

pub fn process_inputed_file<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<OverTlsNode> {
    use std::io::{Error, ErrorKind::InvalidData};
    let path = path.as_ref();
    // 1. try to parse as config file
    if let Ok(config) = OverTlsNode::from_config_file(path) {
        return Ok(config);
    }
    // 2. try to parse as image
    let img = image::open(path).map_err(|e| Error::new(InvalidData, format!("Failed to load file '{path:?}' as image: {e}")))?;

    // 3. try to parse QR code
    let qr_str = qr_decode(&img).map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code from image '{path:?}': {e}")))?;

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

fn config_from_image(dyn_img: &image::DynamicImage) -> std::io::Result<OverTlsNode> {
    use std::io::{Error, ErrorKind::InvalidData};

    // QR parsing
    let qr_str = qr_decode(dyn_img).map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code: {e}")))?;

    // convert to overtls config
    OverTlsNode::from_ssr_url(&qr_str).map_err(|e| Error::new(InvalidData, format!("Failed parse '{qr_str}': {e}")))
}
