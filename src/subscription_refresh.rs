use crate::{
    ServerNode, core,
    model::{ServerList, get_raw_pointer},
    settings::{self, ConfigRef},
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use wxdragon::prelude::*;

fn build_subscription_client(running_node: Option<&ServerNode>) -> reqwest::blocking::Client {
    let mut builder = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(30));
    if let Some(node) = running_node
        && let Some(client) = node.client.as_ref()
    {
        let host = core::normalize_connect_host(&client.listen_host);
        let proxy_addr = format!("socks5h://{}:{}", host, client.listen_port);
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_addr) {
            builder = builder.proxy(proxy);
        }
    }
    builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build subscription HTTP client: {e}");
        reqwest::blocking::Client::new()
    })
}

fn node_address_key(node: &ServerNode) -> Option<(String, u16)> {
    node.client
        .as_ref()
        .map(|client| (client.server_host.to_lowercase(), client.server_port))
}

fn append_nodes_to_model(model: &CustomDataViewTreeModel, cfg: &ConfigRef, nodes: Vec<ServerNode>) -> usize {
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

pub fn is_refresh_in_progress() -> bool {
    REFRESH_IN_PROGRESS.load(Ordering::SeqCst)
}

pub fn refresh_subscriptions(parent: &Frame, cfg: &ConfigRef, model: &CustomDataViewTreeModel) {
    if is_refresh_in_progress() {
        log::warn!("Subscription refresh already in progress; ignoring request.");
        return;
    }
    let subscriptions = cfg.lock().unwrap().get_subscriptions();
    if subscriptions.is_empty() {
        log::info!("No subscriptions to refresh.");
        return;
    }

    let parent_addr: usize = parent as *const Frame as usize;
    let model_addr: usize = model as *const CustomDataViewTreeModel as usize;
    let cfg_clone = cfg.clone();
    std::thread::spawn(move || {
        REFRESH_IN_PROGRESS.store(true, Ordering::SeqCst);
        let running_node = core::get_running_overtls_node();
        let client = build_subscription_client(running_node.as_ref());
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
                                        let is_overtls = item
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.eq_ignore_ascii_case("overtls"))
                                            .unwrap_or(false);
                                        let url = item.get("url").and_then(|v| v.as_str());
                                        if is_overtls && let Some(url_text) = url {
                                            valid_found = true;
                                            match ServerNode::from_ssr_url(url_text) {
                                                Ok(node) => fetched_nodes.push(node),
                                                Err(err) => log::warn!("Failed to parse server URL from subscription: {err}"),
                                            }
                                        }
                                    }
                                    if !valid_found {
                                        log::warn!("Subscription {} contained no valid overtls server entries", subscription_url);
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

        wxdragon::call_after(Box::new(move || {
            let parent_ref = unsafe { &*(parent_addr as *const Frame) };
            let model_ref = unsafe { &*(model_addr as *const CustomDataViewTreeModel) };
            let added = append_nodes_to_model(model_ref, &cfg_clone, fetched_nodes);
            let msg = if added > 0 {
                format!("Successfully added {added} new node(s) from subscriptions.")
            } else {
                "No new nodes were added from subscriptions.".into()
            };
            let dlg = MessageDialog::builder(parent_ref, &msg, "Refresh Complete")
                .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
                .build();
            let _ = dlg.show_modal();
            dlg.destroy();
            REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
        }));
    });
}
