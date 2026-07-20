// settings.rs
// Contains config structs for window settings

use crate::ServerNode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use wxdragon::prelude::*;

/// Top-level application configuration.
/// - `window`: window position/size
/// - `servers`: a list of server nodes managed by the app
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_as_admin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_dir: Option<std::path::PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over_tls: Option<OverTlsSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun2proxy: Option<tun2proxy::Args>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<ServerNode>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriptions: Option<Vec<url::Url>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_refresh_interval_minutes: Option<u64>,
}

pub(crate) type ConfigRef = std::sync::Arc<std::sync::Mutex<Config>>;

pub(crate) const WIDGET_MARGIN: i32 = 2;
pub(crate) const APP_TITLE: &str = "OverTLS-GUI";
pub(crate) const MAIN_ICON: &[u8] = include_bytes!("../assets/main.png");
pub(crate) const ICON_SIZE: u32 = 72;

static DIRTY_FLAG: AtomicBool = AtomicBool::new(false);

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let mut cfg = std::fs::read_to_string(path.as_ref())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Config::default());

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
pub struct OverTlsSettings {
    pub listen_host: String,
    pub listen_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_password: Option<String>,
    pub pool_max_size: usize,
    pub cache_dns: bool,
}

impl Default for OverTlsSettings {
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
        }
    }
}

pub fn load_settings() -> Config {
    let config_path: std::path::PathBuf = retrieve_config_path();
    let cfg = Config::load(&config_path);
    clear_dirty();
    cfg
}

pub fn save_settings(cfg: &Config) -> bool {
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
    /*
    let app_name = env!("CARGO_PKG_NAME");
    let config_path: std::path::PathBuf = ::dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(app_name);
    let _ = std::fs::create_dir_all(&config_path);
    config_path.join("settings.json")
    */
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
    }
}
