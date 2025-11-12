// settings.rs
// Contains config structs for window settings

use crate::ServerNode;
use serde::{Deserialize, Serialize};
use std::path::Path;
use wxdragon::prelude::*;

/// Top-level application configuration.
/// - `window`: window position/size
/// - `servers`: a list of server nodes managed by the app
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
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
    pub tun2proxy: Option<Tun2proxySettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<HttpProxySettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<ServerNode>>,
}

pub(crate) const WIDGET_MARGIN: i32 = 2;
pub(crate) const APP_TITLE: &str = "OverTLS-GUI";
pub(crate) const MAIN_ICON: &[u8] = include_bytes!("../assets/main.png");
pub(crate) const ICON_SIZE: u32 = 72;

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        std::fs::read_to_string(path.as_ref())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Config {
                window: Some(WindowConfig::default()),
                servers: None,
                last_opened_dir: None,
                run_as_admin: None,
                over_tls: None,
                tun2proxy: None,
                http_proxy: None,
                logging: None,
            })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let _ = std::fs::write(path, serde_json::to_string_pretty(self).unwrap());
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
    }
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
pub struct Tun2proxySettings {
    pub exit_on_fatal_error: bool,
    pub max_sessions: usize,
    pub dns_address: String,
    pub dns_strategy: String,
}

impl Default for Tun2proxySettings {
    fn default() -> Self {
        Self {
            exit_on_fatal_error: true,
            max_sessions: 200,
            dns_address: "8.8.8.8".into(),
            dns_strategy: "over-tcp".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HttpProxySettings {
    pub listen_address_port: String,
    pub s5_server_address_port: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Default for HttpProxySettings {
    fn default() -> Self {
        Self {
            listen_address_port: "127.0.0.1:8080".into(),
            s5_server_address_port: "127.0.0.1:1080".into(),
            username: None,
            password: None,
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
    Config::load(&config_path)
}

pub fn save_settings(cfg: &Config) {
    let config_path: std::path::PathBuf = retrieve_config_path();
    log::info!("Saving settings to {}", config_path.display());
    cfg.save(&config_path);
}

fn retrieve_config_path() -> std::path::PathBuf {
    let app_name = env!("CARGO_PKG_NAME");
    let config_path: std::path::PathBuf = ::dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(app_name);
    let _ = std::fs::create_dir_all(&config_path);
    config_path.join("settings.json")
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
