use crate::selection_ctx;
use crate::settings::{self, AppSettingsRef};
use crate::{MenuId, ServerNode, about_dlg, details_dlg, model::ServerList, model::get_raw_pointer, settings_dlg, show_qrcode_dlg};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{cell::RefCell, rc::Rc};
use wxdragon::prelude::*;

/// Dispatch a menu command ID to the same logic used by Frame::on_menu.
/// This allows other UI elements (e.g., double-click on DataView) to reuse menu actions.
pub fn handle_menu_command(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, id: i32, cfg: &AppSettingsRef) {
    fn persist_model_servers(cfg: &AppSettingsRef, model: &CustomDataViewTreeModel) {
        if let Some(servers) = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Vec<ServerNode>>(|list_rc| {
            list_rc.borrow().nodes.iter().map(|rc| rc.borrow().clone()).collect()
        }) {
            let mut cfg_lock = cfg.lock().unwrap();
            cfg_lock.servers = Some(servers);
            settings::save_settings(&cfg_lock);
        }
    }

    let Ok(menu_id) = MenuId::try_from(id) else {
        log::warn!("Received unknown Menu ID: {id}");
        return;
    };
    match menu_id {
        MenuId::Quit => {
            log::info!("Menu/Toolbar: Quit clicked!");
            parent.close(true);
        }
        MenuId::About => {
            about_dlg::show_about_dialog(parent);
        }
        MenuId::Settings => {
            log::info!("Menu/Toolbar: Settings clicked!");
            settings_dlg::settings_dlg(parent, cfg);
        }
        MenuId::ScanQrCode => {
            log::info!("Menu/Toolbar: Scan QR Code clicked!");
            match screenshot_qr_import() {
                Ok(node) => {
                    let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, *const ServerNode>(|data| {
                        let rc = Rc::new(RefCell::new(node));
                        let ptr: *const ServerNode = get_raw_pointer(&rc);
                        data.borrow_mut().nodes.push(rc);
                        ptr
                    });
                    if let Some(ptr) = added {
                        model.item_added::<ServerNode>(None, ptr);
                    }
                }
                Err(e) => {
                    log::error!("Failed to import node from screenshot QR code: {e}");
                }
            }
        }
        MenuId::ViewDetails => {
            log::info!("Menu/Toolbar: View Details clicked!");
            // If a pending selection has been provided (e.g., by a double-click), use it to prefill
            if let Some(weak) = selection_ctx::get_pending_details() {
                if let Some(rc) = weak.upgrade() {
                    // Scope the immutable borrow just for prefill usage
                    let updated = {
                        let init_borrow = rc.borrow();
                        details_dlg::details_dlg(parent, Some(&*init_borrow))
                    };
                    if let Some(updated_node) = updated {
                        // Commit the changes back to the model
                        *rc.borrow_mut() = updated_node;
                        // Notify the view that this item changed
                        let ptr: *const ServerNode = get_raw_pointer(&rc);
                        model.item_changed::<ServerNode>(ptr);
                        persist_model_servers(cfg, model);
                    }
                } else {
                    // Node no longer exists; open dialog without prefill (no commit target)
                    let _ = details_dlg::details_dlg(parent, None);
                }
            } else {
                // No pending selection; treat as read-only view (or nothing to edit)
                let _ = details_dlg::details_dlg(parent, None);
            }
        }
        MenuId::New => {
            log::info!("Menu/Toolbar: New clicked!");
            if let Some(node) = details_dlg::details_dlg(parent, None) {
                let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Option<*const ServerNode>>(|list_rc| {
                    let rc = Rc::new(RefCell::new(node));
                    let ptr: *const ServerNode = get_raw_pointer(&rc);
                    list_rc.borrow_mut().nodes.push(rc);
                    Some(ptr)
                });
                if let Some(Some(ptr)) = added {
                    model.item_added::<ServerNode>(None, ptr);
                    persist_model_servers(cfg, model);
                }
            }
        }
        MenuId::Delete => {
            log::info!("Menu/Toolbar: Delete clicked!");
            if let Some(weak) = selection_ctx::get_pending_details() {
                if let Some(rc) = weak.upgrade() {
                    let title = crate::model::node_title(&rc.borrow());
                    let dlg = MessageDialog::builder(
                        parent,
                        &format!("Do you really want to delete the selected node: \"{title}\"?"),
                        "Confirm Deletion",
                    )
                    .with_style(MessageDialogStyle::OK | MessageDialogStyle::Cancel | MessageDialogStyle::IconWarning)
                    .build();
                    let res = dlg.show_modal();
                    dlg.destroy();
                    if res != wxdragon::id::ID_OK {
                        log::info!("Deletion cancelled by user.");
                        return;
                    }

                    // Capture raw pointer for model notification before removal
                    let child_ptr: *const ServerNode = get_raw_pointer(&rc);

                    // Remove from underlying data
                    let removed = model.with_userdata_mut::<Rc<RefCell<ServerList>>, bool>(|list_rc| {
                        let mut list = list_rc.borrow_mut();
                        if let Some(idx) = list.nodes.iter().position(|n| Rc::ptr_eq(n, &rc)) {
                            list.nodes.remove(idx);
                            true
                        } else {
                            false
                        }
                    });

                    if let Some(true) = removed {
                        // Notify view
                        model.item_deleted::<ServerNode>(None, child_ptr);
                        // Clear selection context as the item is gone
                        selection_ctx::set_pending_details(None);
                        persist_model_servers(cfg, model);
                    } else {
                        log::warn!("Delete requested, but selected node was not found in model.");
                    }
                } else {
                    log::warn!("Delete requested, but the selected item no longer exists.");
                }
            } else {
                log::info!("No selection to delete.");
            }
        }
        MenuId::ShowQrCode => {
            log::info!("Menu/Toolbar: Show QR Code clicked!");
            if let Some(weak) = selection_ctx::get_pending_details()
                && let Some(rc) = weak.upgrade()
            {
                let b = rc.borrow();
                if let Err(e) = show_qrcode_dlg::show_qrcode_dlg(parent, &b) {
                    log::error!("Failed to show QR code dialog: {e}");
                }
            }
        }
        MenuId::ExportNode => {
            log::info!("Menu/Toolbar: Export Node clicked!");
            if let Some(weak) = selection_ctx::get_pending_details()
                && let Some(rc) = weak.upgrade()
            {
                let node = rc.borrow();
                if let Ok(json_str) = serde_json::to_string_pretty(&node.to_json_value().unwrap_or_default()) {
                    let root = cfg.lock().unwrap().get_last_opened_dir().to_string_lossy().to_string();
                    let dialog = FileDialog::builder(parent)
                        .with_message("Save as")
                        .with_style(FileDialogStyle::Save | FileDialogStyle::OverwritePrompt)
                        .with_default_dir(&root)
                        .with_default_file("exported_node.json")
                        .with_wildcard("JSON files (*.json)|*.json|All files (*.*)|*.*")
                        .build();
                    if dialog.show_modal() == wxdragon::id::ID_OK {
                        if let Some(path_option) = dialog.get_path() {
                            cfg.lock()
                                .unwrap()
                                .set_last_opened_dir(PathBuf::from(&path_option).parent().unwrap());
                            if save_exported_node(&path_option, json_str).is_ok() {
                                log::debug!("Node exported to: {}", path_option);
                            }
                        }
                    } else {
                        log::info!("File Dialog: Save cancelled.");
                    }
                    dialog.destroy();
                }
            }
        }

        MenuId::ImportNodeFile => {
            log::info!("Menu/Toolbar: Import Node clicked!");
            let root = cfg.lock().unwrap().get_last_opened_dir().to_string_lossy().to_string();
            let dialog = FileDialog::builder(parent)
                .with_message("Select node JSON file to import")
                .with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist)
                .with_default_dir(&root)
                .with_wildcard("JSON files (*.json)|*.json|All files (*.*)|*.*")
                .build();
            if dialog.show_modal() == wxdragon::id::ID_OK {
                if let Some(path_option) = dialog.get_path() {
                    cfg.lock()
                        .unwrap()
                        .set_last_opened_dir(PathBuf::from(&path_option).parent().unwrap());
                    if let Ok(node) = settings::node_from_config_file(&path_option) {
                        let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Option<*const ServerNode>>(|list_rc| {
                            let rc = Rc::new(RefCell::new(node));
                            let ptr: *const ServerNode = get_raw_pointer(&rc);
                            list_rc.borrow_mut().nodes.push(rc);
                            Some(ptr)
                        });
                        if let Some(Some(ptr)) = added {
                            model.item_added::<ServerNode>(None, ptr);
                            persist_model_servers(cfg, model);
                        }
                    }
                }
            } else {
                log::info!("File Dialog: Open cancelled.");
            }
            dialog.destroy();
        }

        MenuId::Copy => {
            log::info!("Menu/Toolbar: Copy clicked!");
            if let Some(weak) = selection_ctx::get_pending_details()
                && let Some(rc) = weak.upgrade()
            {
                let node = rc.borrow();
                if let Ok(text) = &node.generate_node_url() {
                    if Clipboard::get().set_text(text) {
                        log::info!("Node copied to clipboard.");
                    } else {
                        log::error!("Failed to copy node to clipboard.");
                    }
                }
            }
        }

        MenuId::Paste => {
            log::info!("Menu/Toolbar: Paste clicked!");
            if let Ok(node) = paste() {
                let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, *const ServerNode>(|data| {
                    let rc = Rc::new(RefCell::new(node));
                    let ptr: *const ServerNode = get_raw_pointer(&rc);
                    data.borrow_mut().nodes.push(rc);
                    ptr
                });
                if let Some(ptr) = added {
                    model.item_added::<ServerNode>(None, ptr);
                    persist_model_servers(cfg, model);
                }
            } else {
                log::error!("Failed to paste node.");
            }
        }

        MenuId::RunNode => {
            log::info!("Menu/Toolbar: Run Node clicked!");
        }

        MenuId::Tun2proxy => {
            log::info!("Menu/Toolbar: Tun2proxy clicked!");
        }

        MenuId::SystemProxy => {
            log::info!("Menu/Toolbar: System Proxy clicked!");
            crate::core::toggle_system_proxy(parent, cfg);
        }

        _ => {
            log::warn!("Unhandled Menu ID: {menu_id:?}");
        }
    }
}

pub fn paste() -> std::io::Result<ServerNode> {
    use std::io::{Error, ErrorKind::InvalidData};

    let clipboard = Clipboard::get();

    // Try to get text from clipboard
    if let Some(text) = clipboard.get_text() {
        log::trace!("Pasted text: {text}");
        // Try to parse the text as a config
        return settings::node_from_json(&text)
            .or_else(|_| settings::node_from_anytls_url(&text))
            .or_else(|_| settings::node_from_ssr_url(&text))
            .map_err(|e| Error::new(InvalidData, format!("Some unknown error occurred: {e}")));
    }

    // Check if bitmap format is supported
    if !clipboard.is_format_supported(DataFormat::BITMAP) {
        println!("No bitmap on clipboard");
        return Err(Error::new(InvalidData, "No suitable data found in clipboard"));
    }

    // Create a bitmap data object to receive the data
    let bitmap_data = BitmapDataObject::new(&Bitmap::new(1, 1).unwrap());

    // Get the data from clipboard
    if let Some(_locker) = clipboard.locker() {
        if clipboard.get_data(&bitmap_data) {
            if let Some(bmp) = bitmap_data.get_bitmap() {
                let img = image::RgbaImage::from_raw(bmp.get_width() as u32, bmp.get_height() as u32, bmp.get_rgba_data().unwrap())
                    .ok_or_else(|| std::io::Error::other("Failed to convert clipboard image"))?;

                let dyn_img = image::DynamicImage::ImageRgba8(img);

                return server_node_from_image(&dyn_img);
            }
        } else {
            return Err(Error::new(InvalidData, "Failed to get bitmap from clipboard"));
        }
    }

    Err(Error::new(InvalidData, "No suitable data found in clipboard"))
}

fn server_node_from_image(dyn_img: &image::DynamicImage) -> std::io::Result<ServerNode> {
    use std::io::{Error, ErrorKind::InvalidData};

    // QR parsing
    let qr_str = qr_decode(dyn_img).map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code: {e}")))?;

    // convert to overtls config
    settings::node_from_anytls_url(&qr_str)
        .or_else(|_| settings::node_from_ssr_url(&qr_str))
        .map_err(|e| Error::new(InvalidData, format!("Failed parse '{qr_str}': {e}")))
}

fn qr_decode(img: &image::DynamicImage) -> std::io::Result<String> {
    use std::io::{Error, ErrorKind::InvalidData};

    let mut hints = rxing::DecodeHints {
        TryHarder: Some(true),
        ..Default::default()
    };

    let results = rxing::helpers::detect_multiple_in_image_with_hints(img.clone(), &mut hints)
        .map_err(|e| Error::new(InvalidData, format!("Failed to decode QR code: {e}")))?;

    if results.is_empty() {
        return Err(Error::new(InvalidData, "No QR code found"));
    }

    let text = results[0].getText();
    log::trace!("rxing decoded QR code: {}", text);
    Ok(text.to_string())
}

pub fn screenshot_qr_import() -> std::io::Result<ServerNode> {
    let img = screenshot_to_image()?;
    let scr_str = qr_decode(&img)?;
    settings::node_from_anytls_url(&scr_str).or_else(|_| settings::node_from_ssr_url(&scr_str))
}

fn screenshot_to_image() -> std::io::Result<image::DynamicImage> {
    // Take screenshot of the primary display
    let img = screen_shot::get_screenshot(0).map_err(|e| std::io::Error::other(format!("Screenshot failed: {e}")))?;

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

fn save_exported_node<P: AsRef<Path>>(path: P, json_str: String) -> std::io::Result<()> {
    let path = path.as_ref();
    std::fs::write(path, json_str)?;
    adjust_export_file_permissions(path);
    Ok(())
}

fn adjust_export_file_permissions(_path: &Path) {
    #[cfg(unix)]
    {
        if let Ok(metadata) = std::fs::metadata(_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o644);
            let _ = std::fs::set_permissions(_path, perms);
        }

        if run_as::is_elevated()
            && let Ok(sudo_user) = std::env::var("SUDO_USER")
        {
            let _ = std::process::Command::new("chown").arg("-R").arg(&sudo_user).arg(_path).status();
        }
    }
}
