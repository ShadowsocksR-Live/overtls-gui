// settings.rs
// Contains config structs for window settings

use downcast_rs::{Downcast, impl_downcast};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumCount, EnumIter};
use wxdragon::prelude::*;

pub(crate) use overtls::Config as OverTlsConfig;

#[repr(u32)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, EnumIter, EnumCount, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    #[default]
    #[strum(serialize = "OverTLS")]
    OverTls,
    #[strum(serialize = "AnyTLS")]
    AnyTls,
}

impl NodeType {
    pub fn from_index(index: u32) -> Option<Self> {
        NodeType::iter().nth(index as usize)
    }

    pub fn index(self) -> u32 {
        self as u32
    }

    pub fn display_name(&self) -> &str {
        self.as_ref()
    }

    pub fn choice_labels() -> Vec<String> {
        NodeType::iter().map(|ty| ty.display_name().to_string()).collect()
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name().to_lowercase())
    }
}

pub trait ProxyNode: std::fmt::Debug + Send + Sync + Downcast {
    fn node_type(&self) -> NodeType;
    fn listen_address(&self) -> Option<anytls::ProxyParameters>;
    fn title(&self) -> String;
    fn set_title(&mut self, title: Option<String>);
    fn server_address(&self) -> String;
    fn set_server_address(&mut self, address: &str);
    fn server_port(&self) -> u16;
    fn set_server_port(&mut self, port: u16);
    fn server_domain(&self) -> String;
    fn set_server_domain(&mut self, domain: &str);
    fn server_secret(&self) -> String;
    fn set_server_secret(&mut self, secret: String);
    fn client_id(&self) -> Option<uuid::Uuid>;
    fn set_client_id(&mut self, client_id: Option<uuid::Uuid>);
    fn generate_node_url(&self) -> std::io::Result<String>;
    fn to_json_value(&self) -> serde_json::Result<serde_json::Value>;
}

impl_downcast!(ProxyNode);

pub type ServerNode = Box<dyn ProxyNode>;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct OverTlsNode {
    #[serde(flatten)]
    pub config: OverTlsConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AnyTlsNode {
    #[serde(flatten)]
    pub config: anytls::ClientRuntimeConfig,
    #[serde(skip)]
    pub listen: Option<anytls::ProxyParameters>,
}

impl ProxyNode for OverTlsNode {
    fn node_type(&self) -> NodeType {
        NodeType::OverTls
    }
    fn listen_address(&self) -> Option<anytls::ProxyParameters> {
        let client = self.config.client.as_ref()?;
        let addr = (client.listen_host.parse::<IpAddr>().ok()?, client.listen_port).into();
        let credentials = match (client.listen_user.as_ref(), client.listen_password.as_ref()) {
            (Some(user), Some(pass)) if !user.trim().is_empty() && !pass.trim().is_empty() => {
                Some(anytls::UserKey::new(user.clone(), pass.clone()))
            }
            _ => None,
        };

        Some(anytls::ProxyParameters::new(anytls::ProxyType::Socks5, addr, credentials))
    }
    fn title(&self) -> String {
        self.config
            .remarks
            .clone()
            .or_else(|| self.config.client.as_ref().map(|client| client.server_host.clone()))
            .unwrap_or_else(|| "Unnamed".to_string())
    }
    fn set_title(&mut self, title: Option<String>) {
        self.config.remarks = title;
    }
    fn server_address(&self) -> String {
        self.config
            .client
            .as_ref()
            .map(|client| client.server_host.clone())
            .unwrap_or_default()
    }
    fn set_server_address(&mut self, address: &str) {
        if let Some(client) = self.config.client.as_mut() {
            client.server_host = address.to_string();
        }
    }

    fn server_port(&self) -> u16 {
        self.config.client.as_ref().map(|client| client.server_port).unwrap_or_default()
    }
    fn set_server_port(&mut self, port: u16) {
        if let Some(client) = self.config.client.as_mut() {
            client.server_port = port;
        }
    }
    fn server_domain(&self) -> String {
        self.config
            .client
            .as_ref()
            .and_then(|client| client.server_domain.clone())
            .unwrap_or_default()
    }
    fn set_server_domain(&mut self, domain: &str) {
        if let Some(client) = self.config.client.as_mut() {
            client.server_domain = if domain.is_empty() { None } else { Some(domain.to_string()) };
        }
    }
    fn server_secret(&self) -> String {
        self.config.tunnel_path.to_string()
    }
    fn set_server_secret(&mut self, secret: String) {
        self.config.tunnel_path = overtls::TunnelPath::Single(secret);
    }

    fn client_id(&self) -> Option<uuid::Uuid> {
        self.config.client.as_ref().and_then(|client| client.client_id)
    }
    fn set_client_id(&mut self, client_id: Option<uuid::Uuid>) {
        if let Some(client) = self.config.client.as_mut() {
            client.client_id = client_id;
        }
    }

    fn generate_node_url(&self) -> std::io::Result<String> {
        self.config.generate_ssr_url().map_err(std::io::Error::other)
    }
    fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self.config.clone())
    }
}

impl ProxyNode for AnyTlsNode {
    fn node_type(&self) -> NodeType {
        NodeType::AnyTls
    }
    fn listen_address(&self) -> Option<anytls::ProxyParameters> {
        self.listen.clone()
    }
    fn title(&self) -> String {
        self.config
            .display_name
            .clone()
            .unwrap_or_else(|| self.config.authority().to_string())
    }
    fn set_title(&mut self, title: Option<String>) {
        self.config.display_name = title;
    }
    fn server_address(&self) -> String {
        self.config.server.host()
    }
    fn set_server_address(&mut self, address: &str) {
        self.config.server = (address, self.config.server.port()).into();
    }
    fn server_port(&self) -> u16 {
        self.config.server.port()
    }
    fn set_server_port(&mut self, port: u16) {
        self.config.server = (self.config.server.host(), port).into();
    }
    fn server_domain(&self) -> String {
        self.config.sni.clone().unwrap_or_default()
    }
    fn set_server_domain(&mut self, domain: &str) {
        self.config.sni = if domain.is_empty() { None } else { Some(domain.to_string()) };
    }
    fn server_secret(&self) -> String {
        self.config.password.clone()
    }
    fn set_server_secret(&mut self, secret: String) {
        self.config.password = secret;
    }

    fn client_id(&self) -> Option<uuid::Uuid> {
        self.config.client_id
    }
    fn set_client_id(&mut self, client_id: Option<uuid::Uuid>) {
        self.config.client_id = client_id;
    }

    fn generate_node_url(&self) -> std::io::Result<String> {
        Ok(String::from(&self.config))
    }
    fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self.config.clone())
    }
}

impl Clone for ServerNode {
    fn clone(&self) -> Self {
        match self.node_type() {
            NodeType::OverTls => {
                let config = self
                    .downcast_ref::<OverTlsNode>()
                    .expect("Failed to downcast to OverTlsNode")
                    .config
                    .clone();
                Box::new(OverTlsNode { config })
            }
            NodeType::AnyTls => {
                let node = self.downcast_ref::<AnyTlsNode>().expect("Failed to downcast to AnyTlsNode");
                Box::new(AnyTlsNode {
                    config: node.config.clone(),
                    listen: node.listen.clone(),
                })
            }
        }
    }
}

pub fn over_tls_node(config: OverTlsConfig) -> ServerNode {
    Box::new(OverTlsNode { config })
}

pub fn any_tls_node(config: anytls::ClientRuntimeConfig) -> ServerNode {
    Box::new(AnyTlsNode { config, listen: None })
}

pub fn node_from_json(text: &str) -> std::io::Result<ServerNode> {
    let value = serde_json::from_str::<serde_json::Value>(text).map_err(std::io::Error::other)?;
    node_from_value(value)
}

fn node_from_value(value: serde_json::Value) -> std::io::Result<ServerNode> {
    if let Some(node_type) = value.get("type").and_then(serde_json::Value::as_str) {
        let config = value
            .get("config")
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "node envelope has no config"))?;
        return match node_type {
            "overtls" => serde_json::from_value::<OverTlsConfig>(config)
                .map(over_tls_node)
                .map_err(std::io::Error::other),
            "anytls" => serde_json::from_value::<anytls::ClientRuntimeConfig>(config)
                .map(any_tls_node)
                .map_err(std::io::Error::other),
            other => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported node type: {other}"),
            )),
        };
    }

    // Legacy files had no type field. These fields distinguish the two
    // configuration formats without relying on serde's unknown-field behavior.
    if value.get("tunnel_path").is_some() || value.get("client_settings").is_some() {
        return serde_json::from_value::<OverTlsConfig>(value)
            .map(over_tls_node)
            .map_err(std::io::Error::other);
    }
    serde_json::from_value::<anytls::ClientRuntimeConfig>(value)
        .map(any_tls_node)
        .map_err(std::io::Error::other)
}

pub fn node_from_config_file<P: AsRef<Path>>(path: P) -> std::io::Result<ServerNode> {
    let text = std::fs::read_to_string(path).map_err(std::io::Error::other)?;
    node_from_json(&text)
}

pub fn node_from_ssr_url(text: &str) -> std::io::Result<ServerNode> {
    OverTlsConfig::from_ssr_url(text).map(over_tls_node).map_err(std::io::Error::other)
}

pub fn node_from_anytls_url(text: &str) -> std::io::Result<ServerNode> {
    text.parse::<url::Url>()
        .map_err(std::io::Error::other)
        .and_then(|url| anytls::ClientRuntimeConfig::try_from(&url).map_err(std::io::Error::other))
        .map(any_tls_node)
}

mod node_vec_serde {
    use super::{ServerNode, node_from_value};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(nodes: &Option<Vec<ServerNode>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        nodes
            .as_ref()
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|node| {
                        let type_name = node.node_type().to_string();
                        node.to_json_value().map(|config| {
                            serde_json::json!({
                                "type": type_name,
                                "config": config,
                            })
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<ServerNode>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?;
        values
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| node_from_value(value).map_err(serde::de::Error::custom))
                    .collect()
            })
            .transpose()
    }
}

/// Top-level application configuration.
/// - `window`: window position/size
/// - `servers`: a list of server nodes managed by the app
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AppSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_as_admin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_dir: Option<std::path::PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_settings: Option<LocalServerSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun2proxy: Option<tun2proxy::Args>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "node_vec_serde")]
    pub servers: Option<Vec<ServerNode>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriptions: Option<Vec<url::Url>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_refresh_interval_minutes: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_instance_port: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_auto_refresh: Option<bool>,
}

pub(crate) type AppSettingsRef = std::sync::Arc<std::sync::Mutex<AppSettings>>;

pub(crate) const WIDGET_MARGIN: i32 = 2;
pub(crate) const APP_TITLE: &str = "OverTLS-GUI";
pub(crate) const MAIN_ICON: &[u8] = include_bytes!("../assets/main.png");
pub(crate) const SETTINGS_ICON: &[u8] = include_bytes!("../assets/settings.png");
pub(crate) const OVERTLS_ICON: &[u8] = include_bytes!("../assets/overtls.png");
pub(crate) const PROXY_ICON: &[u8] = include_bytes!("../assets/proxy.png");
pub(crate) const TUN2PROXY_ICON: &[u8] = include_bytes!("../assets/tun2proxy.png");
pub(crate) const ICON_SIZE: u32 = 72;

static DIRTY_FLAG: AtomicBool = AtomicBool::new(false);

impl AppSettings {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let mut cfg = std::fs::read_to_string(path.as_ref())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(AppSettings::default());

        // sanitize window coordinates: negative values (e.g. -1) are invalid and
        // typically result from querying a hidden/uninitialized frame.  Replace
        // them with defaults so we don't persist bogus settings.
        if let Some(win) = &mut cfg.window
            && (win.position.0 < 0 || win.position.1 < 0 || win.size.0 <= 0 || win.size.1 <= 0)
        {
            log::warn!("Discarding invalid saved window geometry {:?}, resetting to default", win);
            *win = WindowConfig::default();
        }
        cfg
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }

    pub fn get_last_opened_dir(&self) -> std::path::PathBuf {
        if let Some(dir) = &self.last_opened_dir {
            std::path::PathBuf::from(dir)
        } else {
            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
        }
    }

    pub fn set_last_opened_dir<P: AsRef<Path>>(&mut self, path: P) {
        self.last_opened_dir = Some(path.as_ref().to_path_buf());
        mark_dirty();
    }

    pub fn add_subscription(&mut self, url: url::Url) {
        let subscriptions = self.subscriptions.get_or_insert_with(Vec::new);
        if subscriptions.iter().any(|existing| existing == &url) {
            log::info!("Subscription URL already exists: {}", url);
            return;
        }
        subscriptions.push(url);
        dedupe_subscriptions(subscriptions);

        mark_dirty();
    }

    pub fn get_subscriptions(&self) -> Vec<url::Url> {
        self.subscriptions.clone().unwrap_or_default()
    }

    pub fn remove_subscription(&mut self, url: &url::Url) -> bool {
        if let Some(subscriptions) = &mut self.subscriptions {
            subscriptions.retain(|existing| existing != url);
            mark_dirty();
            return true;
        }
        false
    }

    pub fn replace_subscription(&mut self, old_url: &url::Url, new_url: url::Url) -> bool {
        if let Some(subscriptions) = &mut self.subscriptions
            && let Some(pos) = subscriptions.iter().position(|existing| existing == old_url)
        {
            subscriptions[pos] = new_url;
            dedupe_subscriptions(subscriptions);
            mark_dirty();
            return true;
        }
        false
    }
}

fn dedupe_subscriptions(urls: &mut Vec<url::Url>) {
    let mut seen = std::collections::HashSet::new();
    urls.retain(|url| seen.insert(url.as_str().to_string()));
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub position: (i32, i32),
    pub size: (i32, i32),
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            position: (200, 250),
            size: (700, 400),
        }
    }
}

impl WindowConfig {
    pub fn new(position: Point, size: Size) -> Self {
        Self {
            position: (position.x, position.y),
            size: (size.width, size.height),
        }
    }

    pub fn get_point(&self) -> Point {
        Point::new(self.position.0, self.position.1)
    }

    pub fn get_size(&self) -> Size {
        Size::new(self.size.0, self.size.1)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalServerSettings {
    pub listen_host: String,
    pub listen_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_password: Option<String>,
    pub pool_max_size: usize,
    pub cache_dns: bool,
}

impl Default for LocalServerSettings {
    fn default() -> Self {
        Self {
            listen_host: "127.0.0.1".into(),
            listen_port: 1080,
            listen_user: None,
            listen_password: None,
            pool_max_size: 200,
            cache_dns: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LoggingSettings {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub global_log_level: Option<String>, // global log level: "Error", "Warn", "Info", "Debug", "Trace"

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_auto_scroll: Option<bool>, // log auto scroll

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_color_output: Option<bool>, // colored log output
}

impl Default for LoggingSettings {
    fn default() -> Self {
        LoggingSettings {
            global_log_level: Some("Debug".to_string()),
            rustls_log_level: Some("Debug".to_string()),
            tokio_tungstenite_log_level: Some("Debug".to_string()),
            tungstenite_log_level: Some("Debug".to_string()),
            ipstack_log_level: Some("Debug".to_string()),
            overtls_log_level: Some("Debug".to_string()),
            tun2proxy_log_level: Some("Debug".to_string()),
            log_auto_scroll: Some(true),
            log_color_output: None,
        }
    }
}

pub fn load_settings() -> AppSettings {
    let config_path: std::path::PathBuf = retrieve_config_path();
    let cfg = AppSettings::load(&config_path);
    clear_dirty();
    cfg
}

pub fn save_settings(cfg: &AppSettings) -> bool {
    let config_path: std::path::PathBuf = retrieve_config_path();
    log::info!("Saving settings to {}", config_path.display());
    if cfg.save(&config_path).is_ok() {
        clear_dirty();
        true
    } else {
        false
    }
}

pub fn mark_dirty() {
    DIRTY_FLAG.store(true, Ordering::Relaxed);
}

pub fn clear_dirty() {
    DIRTY_FLAG.store(false, Ordering::Relaxed);
}

pub fn is_dirty() -> bool {
    DIRTY_FLAG.load(Ordering::Relaxed)
}

fn retrieve_config_path() -> std::path::PathBuf {
    get_config_path("settings.json")
}

pub fn create_bitmap_from_memory(data: &[u8], target_size: Option<(u32, u32)>) -> std::io::Result<Bitmap> {
    let img = image::load_from_memory(data).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if let Some((w, h)) = target_size {
        use image::imageops::FilterType;
        let resized = img.resize_exact(w, h, FilterType::Lanczos3);
        convert_image_to_bitmap(&resized)
    } else {
        convert_image_to_bitmap(&img)
    }
}

pub fn convert_image_to_bitmap(image: &image::DynamicImage) -> std::io::Result<Bitmap> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let icon_bitmap = Bitmap::from_rgba(&rgba, width, height).ok_or(std::io::Error::other("Failed to create bitmap"))?;
    Ok(icon_bitmap)
}

/// Center a rectangle of size (w, h) within the parent window
pub fn center_rect(parent: &dyn WxWidget, w: i32, h: i32) -> (i32, i32) {
    let parent_pos = parent.get_position();
    let parent_size = parent.get_size();
    let x = parent_pos.x + (parent_size.width - w) / 2;
    let y = parent_pos.y + (parent_size.height - h) / 2;
    (x, y)
}

fn get_real_config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        let home_path = PathBuf::from("/home").join(&sudo_user).join(".config");
        return home_path;
    }
    dirs::config_dir().unwrap_or_else(|| std::env::current_dir().unwrap())
}

fn get_config_path(file_name: &str) -> PathBuf {
    let mut path = get_real_config_dir();
    path.push(env!("CARGO_PKG_NAME"));
    let _r = std::fs::create_dir_all(&path);
    #[cfg(target_os = "linux")]
    if _r.is_ok()
        && run_as::is_elevated()
        && let Ok(sudo_user) = std::env::var("SUDO_USER")
        && path.starts_with(format!("/home/{sudo_user}/.config"))
    {
        // chown -R <sudo_user> <path>
        let _ = std::process::Command::new("chown").arg("-R").arg(&sudo_user).arg(&path).status();
    }
    path.push(file_name);
    path
}

impl LoggingSettings {
    /// Creates a new Logger from LoggingSettings
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

        let default_level = if let Some(global_level) = &self.global_log_level {
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

    pub fn is_log_level_equal(&self, other: &LoggingSettings) -> bool {
        self.rustls_log_level == other.rustls_log_level
            && self.tokio_tungstenite_log_level == other.tokio_tungstenite_log_level
            && self.tungstenite_log_level == other.tungstenite_log_level
            && self.ipstack_log_level == other.ipstack_log_level
            && self.overtls_log_level == other.overtls_log_level
            && self.tun2proxy_log_level == other.tun2proxy_log_level
            && self.global_log_level == other.global_log_level
            && self.log_color_output == other.log_color_output
    }
}
