use crate::{
    ServerNode, core,
    model::{ServerList, get_raw_pointer},
    settings::{self, AppSettingsRef},
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashSet,
    net::SocketAddr,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use wxdragon::prelude::*;

fn build_subscription_client(running_node: Option<&ServerNode>) -> std::io::Result<reqwest::blocking::Client> {
    use std::io::Error;
    let mut builder = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(10));
    if let Some(node) = running_node {
        log::info!("Running node detected; using its listen address as a SOCKS5 proxy for subscription requests. {node:?}");
        let listen_address = SocketAddr::try_from(
            &node
                .listen_address()
                .ok_or_else(|| Error::other("Failed to get listen address from running node"))?
                .addr,
        )
        .map_err(|e| Error::other(format!("Failed to parse listen address: {e}")))?;
        let host = listen_address.ip().to_string();
        let host = core::normalize_connect_host(&host);
        let proxy_addr = format!("socks5h://{}:{}", host, listen_address.port());
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_addr) {
            builder = builder.proxy(proxy);
        }
    }
    builder
        .build()
        .map_err(|e| Error::other(format!("Failed to build subscription HTTP client: {e}")))
}

fn node_address_key(node: &ServerNode) -> Option<(String, u16)> {
    Some((node.server_address(), node.server_port()))
}

fn parse_subscription_node(item: &Value) -> std::io::Result<ServerNode> {
    use std::io::{Error, ErrorKind::InvalidData};
    let node_type = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(InvalidData, "subscription entry has no type"))?;
    let url = item
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(InvalidData, "subscription entry has no url"))?;

    match node_type.to_ascii_lowercase().as_str() {
        "anytls" => settings::node_from_anytls_url(url),
        "overtls" => settings::node_from_ssr_url(url),
        other => Err(Error::new(InvalidData, format!("unsupported subscription node type: {other}"))),
    }
}

fn append_nodes_to_model(model: &CustomDataViewTreeModel, cfg: &AppSettingsRef, nodes: Vec<ServerNode>) -> usize {
    let mut added = 0;
    let mut existing: HashSet<(String, u16)> = HashSet::new();
    let mut removed_ptrs: Vec<*const ServerNode> = Vec::new();

    model.with_userdata_mut::<Rc<RefCell<ServerList>>, ()>(|list_rc| {
        let mut list = list_rc.borrow_mut();
        let mut index = list.nodes.len();
        while index > 0 {
            index -= 1;
            let rc = &list.nodes[index];
            let node_key = node_address_key(&rc.borrow());
            if let Some(key) = node_key {
                if existing.contains(&key) {
                    removed_ptrs.push(get_raw_pointer(rc));
                    list.nodes.remove(index);
                } else {
                    existing.insert(key);
                }
            }
        }
    });

    for ptr in removed_ptrs.iter() {
        model.item_deleted::<ServerNode>(None, *ptr);
    }

    let mut waiting: Vec<*const ServerNode> = Vec::new();
    let mut to_add: Vec<Rc<RefCell<ServerNode>>> = Vec::new();
    for node in nodes {
        if let Some(key) = node_address_key(&node) {
            if existing.contains(&key) {
                continue;
            }
            existing.insert(key);
            let rc = Rc::new(RefCell::new(node));
            waiting.push(get_raw_pointer(&rc));
            to_add.push(rc);
        }
    }

    if !to_add.is_empty()
        && model
            .with_userdata_mut::<Rc<RefCell<ServerList>>, ()>(|list_rc| {
                let mut list = list_rc.borrow_mut();
                for rc in to_add.iter() {
                    list.nodes.push(rc.clone());
                }
            })
            .is_some()
    {
        for ptr in waiting {
            model.item_added::<ServerNode>(None, ptr);
            added += 1;
        }
    }

    if removed_ptrs.is_empty() && added == 0 {
        return 0;
    }

    if let Some(servers) = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Vec<ServerNode>>(|list_rc| {
        list_rc.borrow().nodes.iter().map(|rc| rc.borrow().clone()).collect()
    }) {
        let mut cfg_lock = cfg.lock().unwrap();
        cfg_lock.servers = Some(servers);
        settings::save_settings(&cfg_lock);
    }

    added
}

static REFRESH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub struct RefreshResult {
    pub nodes: Vec<ServerNode>,
    pub show_dialog: bool,
}

impl RefreshResult {
    pub fn new(nodes: Vec<ServerNode>, show_dialog: bool) -> Self {
        Self { nodes, show_dialog }
    }
}

pub fn is_refresh_in_progress() -> bool {
    REFRESH_IN_PROGRESS.load(Ordering::SeqCst)
}

pub fn refresh_subscriptions(cfg: &AppSettingsRef, sender: std::sync::mpsc::Sender<RefreshResult>, show_dialog: bool) {
    if REFRESH_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::warn!("Subscription refresh already in progress; ignoring request.");
        return;
    }
    let subscriptions = cfg.lock().unwrap().get_subscriptions();
    if subscriptions.is_empty() {
        log::info!("No subscriptions to refresh.");
        REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
        return;
    }
    let running_node = core::get_global_running_node();
    let Ok(client) = build_subscription_client(running_node.as_ref()) else {
        log::error!("Failed to build subscription HTTP client; aborting refresh.");
        REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
        return;
    };

    std::thread::spawn(move || {
        let mut fetched_nodes = Vec::new();

        for subscription_url in subscriptions {
            log::debug!("Fetching subscription URL: {}", subscription_url);
            match client.get(subscription_url.as_str()).send() {
                Ok(response) => match response.error_for_status() {
                    Ok(resp) => match resp.text() {
                        Ok(body) => match serde_json::from_str::<Value>(&body) {
                            Ok(value) => {
                                let mut valid_found = false;
                                if let Some(servers) = value.get("servers").and_then(|v| v.as_array()) {
                                    for item in servers {
                                        match parse_subscription_node(item) {
                                            Ok(node) => {
                                                valid_found = true;
                                                fetched_nodes.push(node);
                                            }
                                            Err(err) => {
                                                log::warn!("Failed to parse subscription server entry: {err}");
                                            }
                                        }
                                    }
                                    if !valid_found {
                                        log::warn!("Subscription {} contained no valid server entries", subscription_url);
                                    }
                                } else {
                                    log::warn!("Subscription {} response did not contain a servers array", subscription_url);
                                }
                            }
                            Err(err) => log::warn!("Failed to parse subscription JSON: {err}"),
                        },
                        Err(err) => log::warn!("Failed to read subscription body: {err}"),
                    },
                    Err(err) => log::warn!("Subscription HTTP error: {err}"),
                },
                Err(err) => log::warn!("Failed to fetch subscription {}: {err}", subscription_url),
            }
        }

        if sender.send(RefreshResult::new(fetched_nodes, show_dialog)).is_err() {
            REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
        }
    });
}

pub fn apply_refresh_result(parent: &Frame, cfg: &AppSettingsRef, model: &CustomDataViewTreeModel, result: RefreshResult) {
    let added = append_nodes_to_model(model, cfg, result.nodes);
    if result.show_dialog {
        let msg = if added > 0 {
            format!("Successfully added {added} new node(s) from subscriptions.")
        } else {
            "No new nodes were added from subscriptions.".into()
        };
        let dlg = MessageDialog::builder(parent, &msg, "Refresh Complete")
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build();
        let _ = dlg.show_modal();
        dlg.destroy();
    }
    REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
}
