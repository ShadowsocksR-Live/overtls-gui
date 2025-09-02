use crate::OverTlsNode;
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
    pub fn refresh_window(&mut self, win: &fltk::window::Window) {
        use fltk::prelude::WidgetExt;
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

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub log_level: Option<String>, // global log level: "Error", "Warn", "Info", "Debug", "Trace"

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rustls_log_level: Option<String>, // Rustls log level

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tokio_tungstenite_log_level: Option<String>, // tokio_tungstenite log level

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tungstenite_log_level: Option<String>, // tungstenite log level

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ipstack_log_level: Option<String>, // ipstack log level

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub overtls_log_level: Option<String>, // overtls log level

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tun2proxy_log_level: Option<String>, // tun2proxy log level
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
            log_level: Some("Debug".to_string()),
            rustls_log_level: Some("Debug".to_string()),
            tokio_tungstenite_log_level: Some("Debug".to_string()),
            tungstenite_log_level: Some("Debug".to_string()),
            ipstack_log_level: Some("Debug".to_string()),
            overtls_log_level: Some("Debug".to_string()),
            tun2proxy_log_level: Some("Debug".to_string()),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub window: WindowState,

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

impl SystemSettings {
    /// Creates a new Logger from SystemSettings
    pub fn create_logger(&self, sender: crate::logger::LogSender) -> crate::logger::Logger {
        /// Convert string to LevelFilter
        pub fn string_to_level_filter(s: &str) -> Result<log::LevelFilter, &'static str> {
            match s.to_lowercase().as_str() {
                "off" => Ok(log::LevelFilter::Off),
                "error" => Ok(log::LevelFilter::Error),
                "warn" => Ok(log::LevelFilter::Warn),
                "info" => Ok(log::LevelFilter::Info),
                "debug" => Ok(log::LevelFilter::Debug),
                "trace" => Ok(log::LevelFilter::Trace),
                _ => Err("Invalid log level"),
            }
        }

        let mut module_filters = std::collections::HashMap::new();

        if let Some(rustls_level) = &self.rustls_log_level
            && let Ok(level) = string_to_level_filter(rustls_level)
        {
            module_filters.insert("rustls".to_string(), level);
        }

        if let Some(tokio_tungstenite_level) = &self.tokio_tungstenite_log_level
            && let Ok(level) = string_to_level_filter(tokio_tungstenite_level)
        {
            module_filters.insert("tokio_tungstenite".to_string(), level);
        }

        if let Some(tungstenite_level) = &self.tungstenite_log_level
            && let Ok(level) = string_to_level_filter(tungstenite_level)
        {
            module_filters.insert("tungstenite".to_string(), level);
        }

        if let Some(ipstack_level) = &self.ipstack_log_level
            && let Ok(level) = string_to_level_filter(ipstack_level)
        {
            module_filters.insert("ipstack".to_string(), level);
        }

        if let Some(overtls_log_level) = &self.overtls_log_level
            && let Ok(level) = string_to_level_filter(overtls_log_level)
        {
            module_filters.insert("overtls".to_string(), level);
        }

        if let Some(tun2proxy_log_level) = &self.tun2proxy_log_level
            && let Ok(level) = string_to_level_filter(tun2proxy_log_level)
        {
            module_filters.insert("tun2proxy".to_string(), level);
        }

        let default_level = if let Some(global_level) = &self.log_level {
            string_to_level_filter(global_level).unwrap_or(log::LevelFilter::Debug)
        } else {
            log::LevelFilter::Debug
        };

        crate::logger::Logger {
            sender,
            module_filters,
            default_level,
        }
    }

    pub fn is_log_level_equal(&self, other: &SystemSettings) -> bool {
        self.rustls_log_level == other.rustls_log_level
            && self.tokio_tungstenite_log_level == other.tokio_tungstenite_log_level
            && self.tungstenite_log_level == other.tungstenite_log_level
            && self.ipstack_log_level == other.ipstack_log_level
            && self.overtls_log_level == other.overtls_log_level
            && self.tun2proxy_log_level == other.tun2proxy_log_level
            && self.log_level == other.log_level
    }
}
