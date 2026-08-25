use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_download_dir")]
    pub download_dir: String,
    #[serde(default)]
    pub start_minimized: bool,
}

fn default_download_dir() -> String {
    dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("~/Downloads"))
        .to_str()
        .unwrap_or("~/Downloads")
        .to_string()
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            start_minimized: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub notifications: bool,
    #[serde(default)]
    pub auto_grant_media: bool,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

fn default_icon() -> String {
    "default".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default, rename = "services")]
    pub services: Vec<ServiceConfig>,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("ferdirust")
            .join("services.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("Failed to parse config: {e}"),
                },
                Err(e) => eprintln!("Failed to read config: {e}"),
            }
        }

        let config = Self::default_config();
        config.save();
        config
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    eprintln!("Failed to write config: {e}");
                }
            }
            Err(e) => eprintln!("Failed to serialize config: {e}"),
        }
    }

    pub fn enabled_services(&self) -> Vec<&ServiceConfig> {
        self.services.iter().filter(|s| s.enabled).collect()
    }

    fn default_config() -> Self {
        Self {
            global: GlobalConfig::default(),
            services: vec![
                ServiceConfig {
                    id: "messenger".to_string(),
                    name: "Messenger".to_string(),
                    url: "https://www.messenger.com".to_string(),
                    icon: "messenger".to_string(),
                    enabled: true,
                    notifications: true,
                    auto_grant_media: true,
                    allowed_origins: vec!["messenger.com".to_string(), "facebook.com".to_string()],
                },
                ServiceConfig {
                    id: "slack".to_string(),
                    name: "Slack".to_string(),
                    url: "https://app.slack.com".to_string(),
                    icon: "slack".to_string(),
                    enabled: true,
                    notifications: true,
                    auto_grant_media: true,
                    allowed_origins: vec!["slack.com".to_string()],
                },
                ServiceConfig {
                    id: "protonmail".to_string(),
                    name: "Proton Mail".to_string(),
                    url: "https://mail.proton.me".to_string(),
                    icon: "protonmail".to_string(),
                    enabled: true,
                    notifications: true,
                    auto_grant_media: false,
                    allowed_origins: vec!["proton.me".to_string()],
                },
                ServiceConfig {
                    id: "telegram".to_string(),
                    name: "Telegram".to_string(),
                    url: "https://web.telegram.org".to_string(),
                    icon: "telegram".to_string(),
                    enabled: true,
                    notifications: true,
                    auto_grant_media: true,
                    allowed_origins: vec!["telegram.org".to_string()],
                },
            ],
        }
    }
}
