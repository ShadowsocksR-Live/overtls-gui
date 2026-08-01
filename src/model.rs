use crate::ServerNode;
use crate::settings::OverTlsNode;
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
    const Type = 1;
    const ServerSecret = 2;
    const ClientID = 3;
    const ServerHost = 4;
    const ServerPort = 5;
    const ServerDomain = 6;
    const CAFile = 7;
    const DisableTLS = 8;
    const DangerousMode = 9;
}
}

pub fn node_title(node: &ServerNode) -> String {
    node.title()
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
            NodeFields::Remarks => Variant::from_string(node.title()),
            NodeFields::Type => Variant::from_string(node.node_type().display_name()),
            NodeFields::ServerSecret => Variant::from_string(node.server_secret()),
            NodeFields::ClientID => Variant::from_string(node.client_id().map(|id| id.to_string()).unwrap_or_default()),
            NodeFields::ServerHost => Variant::from_string(node.server_address()),
            NodeFields::ServerPort => Variant::from_string(node.server_port().to_string()),
            NodeFields::ServerDomain => Variant::from_string(node.server_domain()),
            NodeFields::CAFile => Variant::from_string(
                node.downcast_ref::<OverTlsNode>()
                    .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                    .and_then(|c| c.cafile.clone())
                    .unwrap_or_default(),
            ),
            NodeFields::DisableTLS => Variant::from_bool(
                node.downcast_ref::<OverTlsNode>()
                    .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                    .and_then(|c| c.disable_tls)
                    .unwrap_or_default(),
            ),
            NodeFields::DangerousMode => Variant::from_bool(
                node.downcast_ref::<OverTlsNode>()
                    .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                    .and_then(|c| c.dangerous_mode)
                    .unwrap_or_default(),
            ),
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

    let modified = match NodeFields::from_bits_retain(col) {
        NodeFields::Remarks => {
            if let Some(s) = var.get_string() {
                node.set_title(if s.trim().is_empty() { None } else { Some(s) });
                true
            } else {
                false
            }
        }
        NodeFields::ServerSecret => {
            if let Some(s) = var.get_string() {
                node.set_server_secret(s);
                true
            } else {
                false
            }
        }
        NodeFields::ClientID => {
            if let Some(s) = var.get_string() {
                node.set_client_id(s.trim().parse::<uuid::Uuid>().ok());
                true
            } else {
                false
            }
        }
        NodeFields::ServerHost => {
            if let Some(ref s) = var.get_string() {
                node.set_server_address(s);
                true
            } else {
                false
            }
        }
        NodeFields::ServerPort => {
            if let Some(v) = var.get_i32() {
                if v >= 0 && v <= u16::MAX as i32 {
                    node.set_server_port(v as u16);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        NodeFields::ServerDomain => {
            if let Some(s) = &var.get_string() {
                node.set_server_domain(s);
                true
            } else {
                false
            }
        }
        NodeFields::CAFile => {
            if let Some(s) = var.get_string() {
                if let Some(over_tls_node) = node.downcast_mut::<OverTlsNode>()
                    && let Some(c) = over_tls_node.config.client.as_mut()
                {
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
                if let Some(over_tls_node) = node.downcast_mut::<OverTlsNode>()
                    && let Some(c) = over_tls_node.config.client.as_mut()
                {
                    c.disable_tls = if b { Some(true) } else { None };
                }
                true
            } else {
                false
            }
        }
        NodeFields::DangerousMode => {
            if let Some(b) = var.get_bool() {
                if let Some(over_tls_node) = node.downcast_mut::<OverTlsNode>()
                    && let Some(c) = over_tls_node.config.client.as_mut()
                {
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
    fn text(node: &ServerNode, col: NodeFields) -> String {
        let ot_node = node.downcast_ref::<OverTlsNode>();
        match col {
            NodeFields::Remarks => node.title().to_lowercase(),
            NodeFields::Type => node.node_type().display_name().to_lowercase(),
            NodeFields::ServerSecret => node.server_secret().to_lowercase(),
            NodeFields::ClientID => node.client_id().map(|id| id.to_string()).unwrap_or_default(),
            NodeFields::ServerHost => node.server_address().to_lowercase(),
            NodeFields::ServerPort => node.server_port().to_string(),
            NodeFields::ServerDomain => node.server_domain().to_lowercase(),
            NodeFields::CAFile => ot_node
                .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                .and_then(|c| c.cafile.clone())
                .unwrap_or_default()
                .to_lowercase(),
            NodeFields::DisableTLS => ot_node
                .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                .and_then(|c| c.disable_tls)
                .unwrap_or(false)
                .to_string(),
            NodeFields::DangerousMode => ot_node
                .and_then(|over_tls_node| over_tls_node.config.client.as_ref())
                .and_then(|c| c.dangerous_mode)
                .unwrap_or(false)
                .to_string(),
            _ => String::new(),
        }
    }
    let field = NodeFields::from_bits_retain(col);
    let ord = text(a, field).cmp(&text(b, field));
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
