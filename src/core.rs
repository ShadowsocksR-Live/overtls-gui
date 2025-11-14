use crate::{ServerNode, settings::OverTlsSettings};

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

pub fn main_task_block(
    config: ServerNode,
    tun2proxy_args: Option<tun2proxy::Args>,
    token: overtls::CancellationToken,
) -> std::io::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let token_tun2proxy = token.clone();
        let token_overtls = token.clone();

        let res = tokio::select! {
            res = tun2proxy_main_task(tun2proxy_args, token_tun2proxy) => {
                if let Err(err) = &res {
                    log::error!("tun2proxy task error: {err}");
                }
                res.map(|_| ())
            }
            res = overtls::async_main(config, false, token_overtls) => {
                if let Err(err) = &res {
                    log::error!("overtls task error: {err}");
                }
                res.map_err(std::io::Error::other)
            }
        };
        token.cancel();
        res
    })
}

pub fn cook_tun2proxy_config(settings: &crate::settings::Config, config: &ServerNode) -> Option<tun2proxy::Args> {
    let remote_server_ip = config.client.as_ref().and_then(|c| c.server_ip_addr())?;

    let mut result = tun2proxy::Args::default();
    result.bypass(remote_server_ip.ip().into());
    result.setup(true);

    {
        let mut proxy = tun2proxy::ArgProxy {
            proxy_type: tun2proxy::ProxyType::Socks5,
            ..Default::default()
        };

        let ip: std::net::IpAddr = settings.over_tls.as_ref().unwrap().listen_host.parse().ok()?;
        proxy.addr = (ip, settings.over_tls.as_ref().unwrap().listen_port).into();

        proxy.credentials = match (
            &settings.over_tls.as_ref().unwrap().listen_user.as_ref().map_or("", |v| v),
            &settings.over_tls.as_ref().unwrap().listen_password.as_ref().map_or("", |v| v),
        ) {
            (u, p) if u.is_empty() && p.is_empty() => None,
            _ => Some(tun2proxy::UserKey::new(
                settings.over_tls.as_ref().unwrap().listen_user.clone().unwrap_or_default(),
                settings.over_tls.as_ref().unwrap().listen_password.clone().unwrap_or_default(),
            )),
        };

        result.proxy(proxy);
    }

    Some(result)
}

async fn tun2proxy_main_task(args: Option<tun2proxy::Args>, shutdown_token: overtls::CancellationToken) -> std::io::Result<usize> {
    if let Some(tun2proxy_args) = args {
        _tun2proxy_main_task(tun2proxy_args, shutdown_token).await
    } else {
        std::future::pending::<std::io::Result<usize>>().await
    }
}

async fn _tun2proxy_main_task(args: tun2proxy::Args, shutdown_token: overtls::CancellationToken) -> std::io::Result<usize> {
    log::debug!("Starting tun2proxy...");
    unsafe extern "C" fn traffic_cb(status: *const tun2proxy::TrafficStatus, _: *mut std::ffi::c_void) {
        let status = unsafe { &*status };
        log::debug!("Traffic: ▲ {} : ▼ {}", status.tx, status.rx);
    }
    unsafe { tun2proxy::tun2proxy_set_traffic_status_callback(1, Some(traffic_cb), std::ptr::null_mut()) };

    let ret = tun2proxy::general_run_async(args, tun2proxy::DEFAULT_MTU, cfg!(target_os = "macos"), shutdown_token).await;
    if let Err(err) = &ret {
        log::error!("tun2proxy main loop error: {err}");
    }
    ret
}
