use crate::selection_ctx;
use crate::settings::Config;
use crate::{MenuId, ServerNode, about_dlg, details_dlg, model::ServerList, model::get_raw_pointer, settings_dlg, show_qrcode_dlg};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::{cell::RefCell, rc::Rc};
use wxdragon::prelude::*;

type CancelTokenPtr = Arc<Mutex<Option<overtls::CancellationToken>>>;
type ThreadHandlePtr = Arc<Mutex<Option<JoinHandle<std::io::Result<()>>>>>;

// Independent runners for toolbar actions
static OVERTLS_TOKEN: LazyLock<CancelTokenPtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static OVERTLS_HANDLE: LazyLock<ThreadHandlePtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static OVERTLS_RUNNING_NODE: LazyLock<Arc<Mutex<Option<ServerNode>>>> = LazyLock::new(|| Arc::new(Mutex::new(None)));

static TUN2PROXY_TOKEN: LazyLock<CancelTokenPtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static TUN2PROXY_HANDLE: LazyLock<ThreadHandlePtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));

static HTTPPROXY_TOKEN: LazyLock<CancelTokenPtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static HTTPPROXY_HANDLE: LazyLock<ThreadHandlePtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Dispatch a menu command ID to the same logic used by Frame::on_menu.
/// This allows other UI elements (e.g., double-click on DataView) to reuse menu actions.
pub fn handle_menu_command(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, id: i32, cfg: &Arc<Mutex<Config>>) {
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
                }
            }
        }
        MenuId::Delete => {
            log::info!("Menu/Toolbar: Delete clicked!");
            if let Some(weak) = selection_ctx::get_pending_details() {
                if let Some(rc) = weak.upgrade() {
                    let title = crate::model::node_title(&rc.borrow());
                    let res = MessageDialog::builder(
                        parent,
                        &format!("Do you really want to delete the selected node: \"{title}\"?"),
                        "Confirm Deletion",
                    )
                    .with_style(MessageDialogStyle::OK | MessageDialogStyle::Cancel | MessageDialogStyle::IconWarning)
                    .build()
                    .show_modal();
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
                if let Ok(json_str) = serde_json::to_string_pretty(&*node) {
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
                            if std::fs::write(&path_option, json_str).is_ok() {
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
                    if let Ok(node) = ServerNode::from_config_file(&path_option) {
                        let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Option<*const ServerNode>>(|list_rc| {
                            let rc = Rc::new(RefCell::new(node));
                            let ptr: *const ServerNode = get_raw_pointer(&rc);
                            list_rc.borrow_mut().nodes.push(rc);
                            Some(ptr)
                        });
                        if let Some(Some(ptr)) = added {
                            model.item_added::<ServerNode>(None, ptr);
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
                if let Ok(text) = &node.generate_ssr_url() {
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
                }
            } else {
                log::error!("Failed to paste node.");
            }
        }

        MenuId::OverTls => {
            log::info!("Menu/Toolbar: OverTLS clicked!");
        }

        MenuId::Tun2proxy => {
            log::info!("Menu/Toolbar: Tun2proxy clicked!");
        }

        MenuId::HttpProxy => {
            log::info!("Menu/Toolbar: HttpProxy clicked!");
        }

        _ => {
            log::warn!("Unhandled Menu ID: {menu_id:?}");
        }
    }
}

// ----- Toolbar public APIs -----

pub fn is_overtls_running() -> bool {
    // lock the handle and return true if it exists and the thread isn't finished
    OVERTLS_HANDLE
        .lock()
        .map(|opt| opt.as_ref().map(|h| !h.is_finished()).unwrap_or(false))
        .unwrap_or(false)
}

pub fn is_tun2proxy_running() -> bool {
    TUN2PROXY_HANDLE
        .lock()
        .map(|opt| opt.as_ref().map(|h| !h.is_finished()).unwrap_or(false))
        .unwrap_or(false)
}

pub fn is_http_proxy_running() -> bool {
    HTTPPROXY_HANDLE
        .lock()
        .map(|opt| opt.as_ref().map(|h| !h.is_finished()).unwrap_or(false))
        .unwrap_or(false)
}

pub fn start_overtls_only(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, cfg: &Arc<Mutex<Config>>) {
    if is_overtls_running() {
        MessageDialog::builder(parent, "OverTLS is already running.", "Info")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
        return;
    }

    let Some(weak) = selection_ctx::get_pending_details() else {
        MessageDialog::builder(parent, "Please select a node first.", "Info")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
        return;
    };
    let Some(rc) = weak.upgrade() else {
        MessageDialog::builder(parent, "The selected node does not exist or is invalid.", "Info")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
        return;
    };
    let mut node = rc.borrow().clone();
    let settings = cfg.lock().unwrap().clone();
    crate::core::merge_system_settings_to_node_config(&settings.over_tls.clone().unwrap_or_default(), &mut node);
    if let Err(e) = node.check_correctness(false) {
        MessageDialog::builder(parent, &format!("Node configuration is incorrect: {e}"), "Cannot start")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
            .build()
            .show_modal();
        return;
    }

    let title = node.remarks.clone().unwrap_or_else(|| "OverTLS".to_string());
    let token = overtls::CancellationToken::new();
    *OVERTLS_TOKEN.lock().unwrap() = Some(token.clone());
    let running_token = OVERTLS_TOKEN.clone();
    let running_handle = OVERTLS_HANDLE.clone();
    let overtls_running_node = OVERTLS_RUNNING_NODE.clone();
    let handle = std::thread::spawn(move || {
        overtls_running_node.lock().unwrap().replace(node.clone());
        let res = {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build();
            match rt {
                Ok(rt) => rt.block_on(async move { overtls::async_main(node, false, token).await.map_err(std::io::Error::other) }),
                Err(e) => Err(std::io::Error::other(e)),
            }
        };
        if let Err(e) = &res {
            log::error!("OverTLS unexpectedly exited with error: {e}");
        }
        if let Ok(mut token) = running_token.try_lock()
            && let Some(t) = token.take()
        {
            t.cancel();
        }
        if let Ok(mut handle) = running_handle.try_lock() {
            handle.take();
        }
        overtls_running_node.lock().unwrap().take();
        res
    });
    *OVERTLS_HANDLE.lock().unwrap() = Some(handle);
    log::info!("OverTLS '{title}' is starting...");
    let _ = model; // keep param for symmetry; not used here
}

pub fn get_running_overtls_node() -> Option<overtls::Config> {
    OVERTLS_RUNNING_NODE.lock().unwrap().clone()
}

#[inline]
pub fn stop_overtls_only() -> std::io::Result<()> {
    stop_thread_with_cancel_token(&OVERTLS_TOKEN, &OVERTLS_HANDLE)
}

pub fn start_tun2proxy_only(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>) {
    if !run_as::is_elevated() {
        let msg = "Tun2Proxy requires elevated privileges to run. Please restart the application as administrator.";
        MessageDialog::builder(parent, msg, "Insufficient Privileges")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
            .build()
            .show_modal();
        return;
    }

    if is_tun2proxy_running() {
        MessageDialog::builder(parent, "Tun2Proxy is already running.", "Info")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
        return;
    }

    let node = get_running_overtls_node();

    let settings = cfg.lock().unwrap().clone();
    let Some(t2p_args) = crate::core::cook_tun2proxy_config(&settings, node.as_ref()) else {
        MessageDialog::builder(parent, "Failed to prepare Tun2Proxy configuration. Please check your settings and make sure the running node has a valid client configuration.", "Error")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
            .build()
            .show_modal();
        return;
    };

    let token = overtls::CancellationToken::new();
    *TUN2PROXY_TOKEN.lock().unwrap() = Some(token.clone());
    let running_token = TUN2PROXY_TOKEN.clone();
    let running_handle = TUN2PROXY_HANDLE.clone();
    let handle = std::thread::spawn(move || {
        let res = {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build();
            match rt {
                Ok(rt) => rt.block_on(async move {
                    log::debug!("Starting tun2proxy...");
                    unsafe extern "C" fn traffic_cb(status: *const tun2proxy::TrafficStatus, _: *mut std::ffi::c_void) {
                        let status = unsafe { &*status };
                        log::debug!("Traffic: ▲ {} : ▼ {}", status.tx, status.rx);
                    }
                    unsafe { tun2proxy::tun2proxy_set_traffic_status_callback(1, Some(traffic_cb), std::ptr::null_mut()) };
                    tun2proxy::general_run_async(t2p_args, tun2proxy::DEFAULT_MTU, cfg!(target_os = "macos"), token)
                        .await
                        .map(|_| ())
                }),
                Err(e) => Err(std::io::Error::other(e)),
            }
        };
        if let Err(e) = &res {
            log::error!("Tun2Proxy unexpectedly exited with error: {e}");
        }
        if let Ok(mut token) = running_token.try_lock()
            && let Some(t) = token.take()
        {
            t.cancel()
        }
        if let Ok(mut handle) = running_handle.try_lock() {
            handle.take();
        }
        res
    });
    *TUN2PROXY_HANDLE.lock().unwrap() = Some(handle);
    log::info!("Tun2Proxy is starting...");
}

#[inline]
pub fn stop_tun2proxy_only() -> std::io::Result<()> {
    stop_thread_with_cancel_token(&TUN2PROXY_TOKEN, &TUN2PROXY_HANDLE)
}

pub fn start_http_proxy_only(parent: &dyn WxWidget, cfg: &Arc<Mutex<Config>>) {
    if is_http_proxy_running() {
        MessageDialog::builder(parent, "HTTP proxy is already running.", "Info")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
        return;
    }

    // prepare configuration from saved settings
    let http = cfg.lock().unwrap().http_proxy.clone().unwrap_or_default();
    // construct socks-hub config strings
    let listen = format!("http://{}", http.listen_address_port);

    // if OverTLS node is running, point the hub's remote server at its listen address
    let remote = if let Some(overtls_cfg) = get_running_overtls_node()
        && let Some(client) = overtls_cfg.client.as_ref()
    {
        format!("socks5://{}:{}", client.listen_host, client.listen_port)
    } else {
        format!("socks5://{}", http.s5_server_address_port)
    };

    let hub_cfg = socks_hub::Config::new(&listen, &remote);

    let token = overtls::CancellationToken::new();
    *HTTPPROXY_TOKEN.lock().unwrap() = Some(token.clone());
    let running_token = HTTPPROXY_TOKEN.clone();
    let running_handle = HTTPPROXY_HANDLE.clone();

    // prepare a simple callback that logs the actual listen address
    fn log_listen(addr: std::net::SocketAddr) {
        log::info!("HTTP proxy listening on {}", addr);
    }

    let handle = std::thread::spawn(move || {
        // socks-hub provides async entry point
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build();
        let res = match rt {
            Ok(rt) => rt.block_on(async move {
                log::debug!("Starting http proxy (socks-hub)...");
                socks_hub::main_entry(&hub_cfg, token.clone(), Some(log_listen))
                    .await
                    .map_err(std::io::Error::other)
            }),
            Err(e) => Err(std::io::Error::other(e)),
        };

        if let Err(e) = &res {
            log::error!("HTTP proxy task exited with error: {e}");
        }

        if let Ok(mut token) = running_token.try_lock()
            && let Some(t) = token.take()
        {
            t.cancel();
        }
        if let Ok(mut handle) = running_handle.try_lock() {
            handle.take();
        }

        res
    });

    *HTTPPROXY_HANDLE.lock().unwrap() = Some(handle);
    log::info!("HTTP proxy is starting...");
}

#[inline]
pub fn stop_http_proxy_only() -> std::io::Result<()> {
    stop_thread_with_cancel_token(&HTTPPROXY_TOKEN, &HTTPPROXY_HANDLE)
}

pub fn stop_all_services() -> std::io::Result<()> {
    if let Err(e) = stop_overtls_only() {
        log::debug!("Failed to stop OverTLS: {e}");
    }
    if let Err(e) = stop_tun2proxy_only() {
        log::debug!("Failed to stop Tun2Proxy: {e}");
    }
    if let Err(e) = stop_http_proxy_only() {
        log::debug!("Failed to stop HTTP proxy: {e}");
    }
    Ok(())
}

pub fn paste() -> std::io::Result<ServerNode> {
    use std::io::{Error, ErrorKind::InvalidData};

    let clipboard = Clipboard::get();

    // Try to get text from clipboard
    if let Some(text) = clipboard.get_text() {
        log::trace!("Pasted text: {text}");
        // Try to parse the text as a config
        return ServerNode::from_json_str(&text)
            .or_else(|_| ServerNode::from_ssr_url(&text))
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
    ServerNode::from_ssr_url(&qr_str).map_err(|e| Error::new(InvalidData, format!("Failed parse '{qr_str}': {e}")))
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

pub fn screenshot_qr_import() -> std::io::Result<ServerNode> {
    let img = screenshot_to_image()?;
    let scr_str = qr_decode(&img)?;
    Ok(ServerNode::from_ssr_url(&scr_str)?)
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

fn stop_thread_with_cancel_token(running_token: &CancelTokenPtr, running_handle: &ThreadHandlePtr) -> std::io::Result<()> {
    let f1 = |e| std::io::Error::other(format!("running_token lock error: {e}"));
    if let Some(token) = running_token.lock().map_err(f1)?.take() {
        token.cancel();
    } else {
        return Err(std::io::Error::other("No running node."));
    }
    let f2 = |e| std::io::Error::other(format!("running_handle lock error: {e}"));
    if let Some(handle) = running_handle.lock().map_err(f2)?.take()
        && let Err(e) = crate::util::thread_handle_join_with_timeout(handle, 3000)
    {
        return Err(std::io::Error::other(format!("Failed to join running thread: {e}")));
    }
    Ok(())
}
