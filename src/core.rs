use crate::{
    ServerNode, selection_ctx,
    settings::{AnyTlsNode, AppSettings, AppSettingsRef, LocalServerSettings, NodeType, OverTlsConfig, OverTlsNode},
};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;
use wxdragon::prelude::*;

pub fn start_selected_node(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, cfg: &Arc<Mutex<AppSettings>>) {
    let Some(node_rc) = selection_ctx::get_pending_details().and_then(|weak| weak.upgrade()) else {
        show_proxy_error(parent, "Select a node before starting it.", "Info");
        return;
    };
    let node = node_rc.borrow().clone();
    match node.node_type() {
        NodeType::OverTls => start_overtls_only(parent, model, cfg, node),
        NodeType::AnyTls => start_anytls_only(parent, cfg, node),
    }
}

fn start_anytls_only(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>, node: ServerNode) {
    if is_global_node_running() {
        let _ = stop_running_node();
        return;
    }

    let Some(mut any_tls_node) = node.downcast_ref::<AnyTlsNode>().cloned() else {
        show_proxy_error(parent, "The selected node is not an AnyTLS node.", "Info");
        return;
    };
    let settings = cfg.lock().unwrap().clone();
    let local_settings = settings.local_settings.unwrap_or_default();

    let Some(listen_parameters) = build_listen_parameters_from_local_settings(&local_settings) else {
        show_proxy_error(parent, "Invalid local SOCKS5 listener settings.", "Error");
        return;
    };
    any_tls_node.listen = Some(listen_parameters.clone());

    let mut args = any_tls_node.config.clone();
    args.listen = listen_parameters;

    let token = overtls::CancellationToken::new();
    *GLOBAL_RUNNING_NODE_TOKEN.lock().unwrap() = Some(token.clone());
    let running_token = GLOBAL_RUNNING_NODE_TOKEN.clone();
    let running_handle = GLOBAL_RUNNING_NODE_HANDLE.clone();
    let global_running_node = GLOBAL_RUNNING_NODE.clone();
    let title = node.title();
    let handle = std::thread::spawn(move || {
        global_running_node.lock().unwrap().replace(Box::new(any_tls_node));
        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(std::io::Error::other)
            .and_then(|runtime| runtime.block_on(anytls::run_client(token.clone(), args, None)));
        if let Err(error) = &result {
            log::error!("AnyTLS unexpectedly exited with error: {error}");
        }
        if let Ok(mut token) = running_token.try_lock()
            && let Some(token) = token.take()
        {
            token.cancel();
        }
        if let Ok(mut handle) = running_handle.try_lock() {
            handle.take();
        }
        global_running_node.lock().unwrap().take();
        result
    });
    *GLOBAL_RUNNING_NODE_HANDLE.lock().unwrap() = Some(handle);
    log::info!("AnyTLS '{title}' is starting...");
}

pub fn build_listen_proxy_url(local_settings: &LocalServerSettings) -> Option<String> {
    let mut url = url::Url::parse("mixed://127.0.0.1:1080").ok()?;
    let host = if local_settings.listen_host.contains(':') && !local_settings.listen_host.starts_with('[') {
        format!("[{}]", local_settings.listen_host)
    } else {
        local_settings.listen_host.clone()
    };
    url.set_host(Some(&host)).ok()?;
    url.set_port(Some(local_settings.listen_port)).ok()?;
    let username = local_settings.listen_user.as_deref().unwrap_or_default();
    let password = local_settings.listen_password.as_deref().unwrap_or_default();
    if !username.is_empty() || !password.is_empty() {
        url.set_username(username).ok()?;
        url.set_password(Some(password)).ok()?;
    }
    Some(url.to_string())
}

pub fn build_listen_parameters_from_local_settings(local_settings: &LocalServerSettings) -> Option<anytls::ProxyParameters> {
    build_listen_proxy_url(local_settings)?.as_str().try_into().ok()
}

pub fn merge_local_settings_to_overtls_node_config(
    local_settings: &LocalServerSettings,
    ot_node_config: &mut OverTlsConfig,
) -> Result<(), String> {
    if let Some(client) = &mut ot_node_config.client {
        let listen_url = build_listen_proxy_url(local_settings).ok_or("Invalid local SOCKS5 listener settings")?;
        client.listen = listen_url
            .as_str()
            .try_into()
            .map_err(|_| "Invalid local SOCKS5 listener URL".to_string())?;

        client.pool_max_size = Some(local_settings.pool_max_size);
        client.cache_dns = local_settings.cache_dns;
    }
    Ok(())
}

pub fn cook_tun2proxy_config(settings: &crate::settings::AppSettings, running_node: Option<&ServerNode>) -> Option<tun2proxy::Args> {
    // start from user-configured arguments so bypass list and other options are preserved
    let mut result = settings.tun2proxy.clone().unwrap_or_default();

    // ensure routing setup is requested regardless of stored value
    result.setup(true);

    // if running node exists and has client config, add the remote server IP to bypass list and set up proxy config for it
    // otherwise, if no running node or client config, just use the user-configured tun2proxy args without modification
    if let Some(running_node) = running_node {
        if let Some(ot) = running_node.downcast_ref::<OverTlsNode>()
            && let Some(client) = ot.config.client.as_ref()
        {
            let remote_server_ip = client.server_ip_addr()?;

            // always bypass the remote server's own IP so traffic directed to it
            // does not go through the tunnel
            result.bypass(remote_server_ip.ip().into());

            result.proxy(client.listen.clone());
        } else if let Some(anytls_node) = running_node.downcast_ref::<AnyTlsNode>() {
            let server = anytls_node.config.server.as_ref()?;
            result.bypass(std::net::SocketAddr::try_from(server).ok()?.ip().into());

            let listen_addr = running_node.listen_address()?;
            result.proxy(listen_addr);
        } else {
            log::warn!("Running node is neither OverTLS nor AnyTLS, cannot configure Tun2Proxy.");
            return None;
        }
    }

    Some(result)
}

pub fn normalize_connect_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        _ => host,
    }
}

type CancelTokenPtr = Arc<Mutex<Option<overtls::CancellationToken>>>;
type ThreadHandlePtr = Arc<Mutex<Option<JoinHandle<std::io::Result<()>>>>>;

// Independent runners for toolbar actions
static GLOBAL_RUNNING_NODE_TOKEN: LazyLock<CancelTokenPtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static GLOBAL_RUNNING_NODE_HANDLE: LazyLock<ThreadHandlePtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static GLOBAL_RUNNING_NODE: LazyLock<Arc<Mutex<Option<ServerNode>>>> = LazyLock::new(|| Arc::new(Mutex::new(None)));

static TUN2PROXY_TOKEN: LazyLock<CancelTokenPtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));
static TUN2PROXY_HANDLE: LazyLock<ThreadHandlePtr> = LazyLock::new(|| Arc::new(Mutex::new(None)));

pub fn is_global_node_running() -> bool {
    // lock the handle and return true if it exists and the thread isn't finished
    GLOBAL_RUNNING_NODE_HANDLE
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

pub fn start_overtls_only(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, cfg: &Arc<Mutex<AppSettings>>, proxy_node: ServerNode) {
    if is_global_node_running() {
        show_proxy_error(parent, "Proxy node is already running.", "Info");
        return;
    }

    let Some(over_tls_node) = proxy_node.downcast_ref::<OverTlsNode>() else {
        show_proxy_error(parent, "The selected node is not an OverTLS node.", "Info");
        return;
    };
    let mut node: OverTlsConfig = over_tls_node.config.clone();
    let local_settings = cfg.lock().unwrap().clone().local_settings.clone().unwrap_or_default();
    if let Err(error) = merge_local_settings_to_overtls_node_config(&local_settings, &mut node) {
        show_proxy_error(parent, &error, "Invalid listener settings");
        return;
    }
    if let Err(e) = node.check_correctness(false) {
        show_proxy_error(parent, &format!("Node configuration is incorrect: {e}"), "Cannot start");
        return;
    }

    let title = node.remarks.clone().unwrap_or_else(|| "OverTLS".to_string());
    let token = overtls::CancellationToken::new();
    *GLOBAL_RUNNING_NODE_TOKEN.lock().unwrap() = Some(token.clone());
    let running_token = GLOBAL_RUNNING_NODE_TOKEN.clone();
    let running_handle = GLOBAL_RUNNING_NODE_HANDLE.clone();
    let global_running_node = GLOBAL_RUNNING_NODE.clone();
    let handle = std::thread::spawn(move || {
        let node_wrapper = Box::new(OverTlsNode { config: node.clone() });
        global_running_node.lock().unwrap().replace(node_wrapper);
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
        global_running_node.lock().unwrap().take();
        res
    });
    *GLOBAL_RUNNING_NODE_HANDLE.lock().unwrap() = Some(handle);
    log::info!("OverTLS '{title}' is starting...");
    let _ = model; // keep param for symmetry; not used here
}

pub fn get_global_running_node() -> Option<ServerNode> {
    GLOBAL_RUNNING_NODE.lock().unwrap().clone()
}

pub fn toggle_system_proxy(parent: &dyn WxWidget, cfg: &AppSettingsRef) {
    if systemproxy::SystemProxy::is_enabled() {
        if let Err(e) = systemproxy::SystemProxy::stop() {
            show_proxy_error(parent, &format!("Failed to disable the system proxy: {e}"), "Error");
        } else {
            log::info!("System proxy disabled.");
        }
        return;
    }

    let listen = if let Some(node) = get_global_running_node() {
        node.listen_address()
    } else {
        let cfg = cfg.lock().unwrap();
        let tmp = LocalServerSettings::default();
        let local_settings = cfg.local_settings.as_ref().unwrap_or(&tmp);
        build_listen_parameters_from_local_settings(local_settings)
    };

    let Some(listen) = listen else {
        let msg = "The proxy node has no client listen address and no local proxy settings are available.";
        show_proxy_error(parent, msg, "Error");
        return;
    };
    let Some(listen_addr) = listen
        .addr
        .as_ref()
        .and_then(|address| std::net::SocketAddr::try_from(address).ok())
    else {
        show_proxy_error(parent, "The proxy listen address is invalid.", "Error");
        return;
    };

    let mut proxy = systemproxy::SystemProxy {
        enable: true,
        host: normalize_connect_host(&listen_addr.ip().to_string()).to_string(),
        port: listen_addr.port(),
        ..Default::default()
    };
    proxy.deal_with_bypass_simplify(true);
    if let Err(e) = proxy.set_system_proxy() {
        show_proxy_error(parent, &format!("Failed to enable the system proxy: {e}"), "Error");
    } else {
        log::info!("System proxy enabled at {}:{}.", proxy.host, proxy.port);
    }
}

fn show_proxy_error(parent: &dyn WxWidget, message: &str, title: &str) {
    log::error!("{message}");
    let dlg = MessageDialog::builder(parent, message, title)
        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
        .build();
    let _ = dlg.show_modal();
    dlg.destroy();
}

#[inline]
pub fn stop_running_node() -> std::io::Result<()> {
    stop_thread_with_cancel_token(&GLOBAL_RUNNING_NODE_TOKEN, &GLOBAL_RUNNING_NODE_HANDLE)
}

pub fn start_tun2proxy_only(parent: &dyn WxWidget, cfg: &Arc<Mutex<AppSettings>>) {
    if !run_as::is_elevated() {
        let msg = "Tun2Proxy requires elevated privileges to run. Please restart the application as administrator.";
        show_proxy_error(parent, msg, "Insufficient Privileges");
        return;
    }

    if is_tun2proxy_running() {
        show_proxy_error(parent, "Tun2Proxy is already running.", "Info");
        return;
    }

    let node = get_global_running_node();

    let settings = cfg.lock().unwrap().clone();
    let Some(t2p_args) = crate::core::cook_tun2proxy_config(&settings, node.as_ref()) else {
        let msg = "Failed to prepare Tun2Proxy configuration. Please check your settings and make sure the running node has a valid client configuration.";
        show_proxy_error(parent, msg, "Error");
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

pub fn stop_all_services() -> std::io::Result<()> {
    if let Err(e) = stop_running_node() {
        log::debug!("Failed to stop OverTLS: {e}");
    }
    if let Err(e) = stop_tun2proxy_only() {
        log::debug!("Failed to stop Tun2Proxy: {e}");
    }
    Ok(())
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
