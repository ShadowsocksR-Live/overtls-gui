use crate::{ServerNode, selection_ctx, settings::Config, settings::OverTlsSettings};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;
use wxdragon::prelude::*;

pub fn merge_system_settings_to_node_config(ot_settings: &OverTlsSettings, node_config: &mut ServerNode) {
    if let Some(client) = &mut node_config.client {
        client.listen_host = ot_settings.listen_host.clone();
        client.listen_port = ot_settings.listen_port;
        client.listen_user = ot_settings.listen_user.clone();
        client.listen_password = ot_settings.listen_password.clone();
        client.pool_max_size = Some(ot_settings.pool_max_size);
        client.cache_dns = ot_settings.cache_dns;
    }
}

pub fn cook_tun2proxy_config(settings: &crate::settings::Config, running_node: Option<&ServerNode>) -> Option<tun2proxy::Args> {
    // start from user-configured arguments so bypass list and other options are preserved
    let mut result = settings.tun2proxy.clone().unwrap_or_default();

    // ensure routing setup is requested regardless of stored value
    result.setup(true);

    // if running node exists and has client config, add the remote server IP to bypass list and set up proxy config for it
    // otherwise, if no running node or client config, just use the user-configured tun2proxy args without modification
    if let Some(running_node) = running_node
        && let Some(client) = running_node.client.as_ref()
    {
        let remote_server_ip = client.server_ip_addr()?;

        // always bypass the remote server's own IP so traffic directed to it
        // does not go through the tunnel
        result.bypass(remote_server_ip.ip().into());

        let client_host = normalize_connect_host(&client.listen_host);
        // convert host:port into a network SocketAddr using string parsing
        let listen_addr: std::net::SocketAddr = format!("{}:{}", client_host, client.listen_port).parse().ok()?;

        let mut proxy = tun2proxy::ArgProxy {
            proxy_type: tun2proxy::ProxyType::Socks5,
            addr: listen_addr,
            ..Default::default()
        };

        proxy.credentials = match (
            &client.listen_user.as_ref().map_or("", |v| v),
            &client.listen_password.as_ref().map_or("", |v| v),
        ) {
            (u, p) if u.is_empty() && p.is_empty() => None,
            _ => Some(tun2proxy::UserKey::new(
                client.listen_user.clone().unwrap_or_default(),
                client.listen_password.clone().unwrap_or_default(),
            )),
        };

        result.proxy(proxy);
    }

    Some(result)
}

fn normalize_connect_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        _ => host,
    }
}

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
        let host = normalize_connect_host(&client.listen_host);
        format!("socks5://{}:{}", host, client.listen_port)
    } else {
        format!("socks5://{}", http.s5_server_address_port)
    };

    let hub_cfg = socks_hub_core::Config::new(&listen, &remote);

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
                socks_hub_core::main_entry(&hub_cfg, token.clone(), Some(log_listen))
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
