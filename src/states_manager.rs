use crate::OverTlsNode;
use fltk::{prelude::WidgetExt, window::Window};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            x: 100,
            y: 100,
            w: 1024,
            h: 600,
        }
    }
}

impl WindowState {
    pub fn refresh_window(&mut self, win: &Window) {
        self.h = win.height();
        self.w = win.width();
        self.x = win.x();
        self.y = win.y();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    pub listen_host: String,
    pub listen_port: u16,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub listen_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub listen_password: Option<String>,
    pub pool_max_size: usize,
    pub cache_dns: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun2proxy_enable: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun2proxy: Option<tun2proxy::Args>,
}

impl Default for SystemSettings {
    fn default() -> Self {
        SystemSettings {
            listen_host: "127.0.0.1".into(),
            listen_port: 5080,
            listen_user: None,
            listen_password: None,
            pool_max_size: 100,
            cache_dns: false,
            tun2proxy_enable: Some(true),
            tun2proxy: None,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub window: WindowState,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub working_node: Option<OverTlsNode>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub current_node_index: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub current_selection_path: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub system_settings: Option<SystemSettings>,

    pub remote_nodes: Vec<OverTlsNode>,
}

impl AppState {
    pub fn set_current_path(&mut self, path: &std::path::Path) {
        self.current_selection_path = Some(path.to_path_buf());
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // Bug: `drop` method should not be called at all, we should call `save_app_state` explicitly
        if let Err(e) = save_app_state(self) {
            log::debug!("Failed to save app state: {e}");
        }
    }
}

fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| std::env::current_dir().unwrap());
    path.push(env!("CARGO_PKG_NAME"));
    let _ = std::fs::create_dir_all(&path);
    path.push("config.json");
    path
}

pub fn load_app_state() -> AppState {
    let config_path = get_config_path();
    let state: AppState = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    state
}

pub fn save_app_state(state: &AppState) -> std::io::Result<()> {
    let config_path = get_config_path();
    let contents = serde_json::to_string_pretty(state).map_err(|e| std::io::Error::other(format!("Failed to serialize state: {e}")))?;
    std::fs::write(config_path, contents)
}
