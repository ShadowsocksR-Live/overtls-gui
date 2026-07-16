use crate::ServerNode;
use std::{cell::RefCell, rc::Rc};
use wxdragon::prelude::*;

pub type ServerNodeRc = Rc<RefCell<ServerNode>>;

#[derive(Default, Debug, Clone)]
pub struct ServerList {
    pub nodes: Vec<ServerNodeRc>,
}

bitflags::bitflags! {
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeFields : u32 {
    const Remarks = 0;
    const TunnelPath = 1;
    const ClientID = 2;
    const ServerHost = 3;
    const ServerPort = 4;
    const ServerDomain = 5;
    const CAFile = 6;
    const DisableTLS = 7;
    const DangerousMode = 8;
}
}

pub fn node_title(node: &ServerNode) -> String {
    node.remarks
        .as_deref()
        .unwrap_or(node.client.as_ref().map(|c| c.server_host.as_str()).unwrap_or("Unnamed"))
        .to_string()
}

/// Build a CustomDataViewTreeModel that exposes ServerList as a flat list under a virtual root.
/// - Root (None) is a container
/// - Each ServerNode is a leaf item
pub fn create_server_tree_model(data: Rc<RefCell<ServerList>>) -> CustomDataViewTreeModel {
    CustomDataViewTreeModel::new(
        data,
        // parent: top-level leaves have None (root) as parent
        move |_data: &Rc<RefCell<ServerList>>, _item: Option<&ServerNode>| None,
        // is_container: the virtual root (None) is a container; leaves are not
        move |_data: &Rc<RefCell<ServerList>>, item: Option<&ServerNode>| item.is_none(),
        // get_children: root returns all server nodes; leaves have no children
        move |data: &Rc<RefCell<ServerList>>, item: Option<&ServerNode>| match item {
            None => data
                .borrow()
                .nodes
                .iter()
                .map(|rc| &*rc.borrow() as *const ServerNode as *mut ServerNode)
                .collect(),
            Some(_leaf) => Vec::new(),
        },
        get_value_cb,
        Some(set_value_cb),
        Some(move |_: &Rc<RefCell<ServerList>>, _item: Option<&ServerNode>, _col: u32| true),
        Some(compare_cb),
    )
}

fn get_value_cb(data: &Rc<RefCell<ServerList>>, item: Option<&ServerNode>, col: u32) -> Variant {
    fn render(node: &ServerNode, col: NodeFields) -> Variant {
        match col {
            NodeFields::Remarks => Variant::from_string(node.remarks.clone().unwrap_or_default()),
            NodeFields::TunnelPath => Variant::from_string(match &node.tunnel_path {
                overtls::TunnelPath::Single(s) => s.clone(),
                overtls::TunnelPath::Multiple(v) => v.first().cloned().unwrap_or_default(),
            }),
            NodeFields::ClientID => Variant::from_string(
                node.client
                    .as_ref()
                    .and_then(|c| c.client_id.map(|id| id.to_string()))
                    .unwrap_or_default(),
            ),
            NodeFields::ServerHost => Variant::from_string(node.client.as_ref().map(|c| c.server_host.clone()).unwrap_or_default()),
            // DataViewTextRenderer expects a string; display port as string
            NodeFields::ServerPort => Variant::from_string(node.client.as_ref().map(|c| c.server_port.to_string()).unwrap_or_default()),
            NodeFields::ServerDomain => {
                Variant::from_string(node.client.as_ref().and_then(|c| c.server_domain.clone()).unwrap_or_default())
            }
            NodeFields::CAFile => Variant::from_string(node.client.as_ref().and_then(|c| c.cafile.clone()).unwrap_or_default()),
            NodeFields::DisableTLS => Variant::from_bool(node.client.as_ref().and_then(|c| c.disable_tls).unwrap_or(false)),
            NodeFields::DangerousMode => Variant::from_bool(node.client.as_ref().and_then(|c| c.dangerous_mode).unwrap_or(false)),
            _ => Variant::from_string(String::new()),
        }
    }

    let col = NodeFields::from_bits_retain(col);

    match item {
        None => {
            // Virtual root: show a summary in col 0, blanks elsewhere
            let count = data.borrow().nodes.len();
            if col == NodeFields::Remarks {
                Variant::from_string(format!("{count} node(s)"))
            } else {
                Variant::from_string(String::new())
            }
        }
        Some(n) => render(n, col),
    }
}

fn set_value_cb(data: &Rc<RefCell<ServerList>>, item: Option<&ServerNode>, col: u32, var: &Variant) -> bool {
    let needle_ptr: *const ServerNode = match item {
        None => return false, // root is not editable
        Some(n) => n as *const _,
    };

    let target_rc = match find_node_via_raw_ptr(data, needle_ptr) {
        Some(rc) => rc,
        None => return false,
    };

    let mut node = target_rc.borrow_mut();
    if node.client.is_none() {
        node.client = Some(Default::default());
    }

    let modified = match NodeFields::from_bits_retain(col) {
        NodeFields::Remarks => {
            if let Some(s) = var.get_string() {
                node.remarks = if s.trim().is_empty() { None } else { Some(s) };
                true
            } else {
                false
            }
        }
        NodeFields::TunnelPath => {
            if let Some(s) = var.get_string() {
                node.tunnel_path = overtls::TunnelPath::Single(s);
                true
            } else {
                false
            }
        }
        NodeFields::ClientID => {
            if let Some(s) = var.get_string() {
                if let Some(c) = node.client.as_mut() {
                    c.client_id = s.trim().parse::<uuid::Uuid>().ok();
                }
                true
            } else {
                false
            }
        }
        NodeFields::ServerHost => {
            if let Some(s) = var.get_string() {
                if let Some(c) = node.client.as_mut() {
                    c.server_host = s;
                }
                true
            } else {
                false
            }
        }
        NodeFields::ServerPort => {
            if let Some(v) = var.get_i32() {
                if v >= 0 && v <= u16::MAX as i32 {
                    if let Some(c) = node.client.as_mut() {
                        c.server_port = v as u16;
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        NodeFields::ServerDomain => {
            if let Some(s) = var.get_string() {
                if let Some(c) = node.client.as_mut() {
                    let t = s.trim().to_string();
                    c.server_domain = if t.is_empty() { None } else { Some(t) };
                }
                true
            } else {
                false
            }
        }
        NodeFields::CAFile => {
            if let Some(s) = var.get_string() {
                if let Some(c) = node.client.as_mut() {
                    let t = s.trim().to_string();
                    c.cafile = if t.is_empty() { None } else { Some(t) };
                }
                true
            } else {
                false
            }
        }
        NodeFields::DisableTLS => {
            if let Some(b) = var.get_bool() {
                if let Some(c) = node.client.as_mut() {
                    c.disable_tls = if b { Some(true) } else { None };
                }
                true
            } else {
                false
            }
        }
        NodeFields::DangerousMode => {
            if let Some(b) = var.get_bool() {
                if let Some(c) = node.client.as_mut() {
                    c.dangerous_mode = if b { Some(true) } else { None };
                }
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if modified {
        crate::settings::mark_dirty();
    }
    modified
}

fn compare_cb(_data: &Rc<RefCell<ServerList>>, a: &ServerNode, b: &ServerNode, col: u32, asc: bool) -> i32 {
    let ord = match NodeFields::from_bits_retain(col) {
        NodeFields::Remarks => {
            let fa = a.client.as_ref().map(|c| c.server_host.as_str()).unwrap_or("");
            let fb = b.client.as_ref().map(|c| c.server_host.as_str()).unwrap_or("");
            let la = a.remarks.as_deref().unwrap_or(fa).to_lowercase();
            let lb = b.remarks.as_deref().unwrap_or(fb).to_lowercase();
            la.cmp(&lb)
        }
        NodeFields::TunnelPath => {
            let sa: &str = match &a.tunnel_path {
                overtls::TunnelPath::Single(s) => s.as_str(),
                overtls::TunnelPath::Multiple(v) => v.first().map(|s| s.as_str()).unwrap_or(""),
            };
            let sb: &str = match &b.tunnel_path {
                overtls::TunnelPath::Single(s) => s.as_str(),
                overtls::TunnelPath::Multiple(v) => v.first().map(|s| s.as_str()).unwrap_or(""),
            };
            sa.to_lowercase().cmp(&sb.to_lowercase())
        }
        NodeFields::ClientID => a
            .client
            .as_ref()
            .and_then(|c| c.client_id.as_ref())
            .cmp(&b.client.as_ref().and_then(|c| c.client_id.as_ref())),
        NodeFields::ServerHost => a
            .client
            .as_ref()
            .map(|c| c.server_host.to_lowercase())
            .unwrap_or_default()
            .cmp(&b.client.as_ref().map(|c| c.server_host.to_lowercase()).unwrap_or_default()),
        NodeFields::ServerPort => a
            .client
            .as_ref()
            .map(|c| c.server_port)
            .unwrap_or(0)
            .cmp(&b.client.as_ref().map(|c| c.server_port).unwrap_or(0)),
        NodeFields::ServerDomain => a
            .client
            .as_ref()
            .and_then(|c| c.server_domain.as_deref())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.client
                    .as_ref()
                    .and_then(|c| c.server_domain.as_deref())
                    .unwrap_or("")
                    .to_lowercase(),
            ),
        NodeFields::CAFile => a
            .client
            .as_ref()
            .and_then(|c| c.cafile.as_deref())
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b.client.as_ref().and_then(|c| c.cafile.as_deref()).unwrap_or("").to_lowercase()),
        NodeFields::DisableTLS => a
            .client
            .as_ref()
            .and_then(|c| c.disable_tls)
            .cmp(&b.client.as_ref().and_then(|c| c.disable_tls)),
        NodeFields::DangerousMode => a
            .client
            .as_ref()
            .and_then(|c| c.dangerous_mode)
            .cmp(&b.client.as_ref().and_then(|c| c.dangerous_mode)),
        _ => std::cmp::Ordering::Equal,
    };
    let v = match ord {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    if asc { v } else { -v }
}

/// Resolve target Rc<RefCell<ServerNode>> from raw pointer.
pub fn find_node_via_raw_ptr(list: &Rc<RefCell<ServerList>>, needle: *const ServerNode) -> Option<ServerNodeRc> {
    let list_ref = list.borrow();
    for rc in list_ref.nodes.iter() {
        let ptr: *const ServerNode = get_raw_pointer(rc);
        if ptr == needle {
            return Some(rc.clone());
        }
    }
    None
}

pub(crate) fn get_raw_pointer<T>(rc: &Rc<RefCell<T>>) -> *const T {
    let b = rc.borrow();
    &*b as *const _
}
