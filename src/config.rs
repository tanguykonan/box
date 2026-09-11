use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Once;

static DOTENV_INIT: Once = Once::new();

pub const DEFAULT_HUB_URL: &str = "https://boxhub.paxiz.org";
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;
pub const DEFAULT_NETWORK_TIMEOUT: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthCredential {
    pub username: String,
    pub token: String,
}

fn default_hub_url() -> Option<String> {
    Some(DEFAULT_HUB_URL.to_string())
}

fn default_compression_level() -> Option<i32> {
    Some(DEFAULT_COMPRESSION_LEVEL)
}

fn default_network_timeout() -> Option<u64> {
    Some(DEFAULT_NETWORK_TIMEOUT)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfigData {
    #[serde(default = "default_hub_url")]
    pub hub_url: Option<String>,
    #[serde(default = "default_compression_level")]
    pub compression_level: Option<i32>,
    #[serde(default)]
    pub build_workers: Option<usize>,
    #[serde(default = "default_network_timeout")]
    pub network_timeout: Option<u64>,
    #[serde(default)]
    pub python_mirror: Option<String>,
    #[serde(default)]
    pub node_mirror: Option<String>,
    #[serde(default)]
    pub auth: HashMap<String, AuthCredential>,
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for AppConfigData {
    fn default() -> Self {
        Self {
            hub_url: Some(DEFAULT_HUB_URL.to_string()),
            compression_level: Some(DEFAULT_COMPRESSION_LEVEL),
            build_workers: None,
            network_timeout: Some(DEFAULT_NETWORK_TIMEOUT),
            python_mirror: None,
            node_mirror: None,
            auth: HashMap::new(),
            custom: HashMap::new(),
        }
    }
}

pub fn bootstrap_dotenv() {
    DOTENV_INIT.call_once(|| {
        let candidates = [".env.local", ".env", ".env.production", ".env.development"];

        if let Ok(curr) = std::env::current_dir() {
            let mut check_dir = Some(curr.as_path());
            let mut depth = 0;

            while let Some(dir) = check_dir {
                if depth > 6 {
                    break;
                }
                for candidate in &candidates {
                    let env_file = dir.join(candidate);
                    if env_file.is_file() {
                        let _ = dotenvy::from_path(&env_file);
                    }
                }
                check_dir = dir.parent();
                depth += 1;
            }
        }

        // Check ~/.box/.env
        let home_box_env = get_box_home().join(".env");
        if home_box_env.is_file() {
            let _ = dotenvy::from_path(&home_box_env);
        }
    });
}

pub fn get_box_home() -> PathBuf {
    if let Ok(custom) = std::env::var("BOX_HOME") {
        if !custom.trim().is_empty() {
            let p = PathBuf::from(custom.trim());
            let _ = fs::create_dir_all(&p);
            return p;
        }
    }

    let p = if let Some(base) = BaseDirs::new() {
        base.home_dir().join(".box")
    } else {
        PathBuf::from(".box")
    };
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_runtimes_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BOX_RUNTIMES_DIR") {
        if !custom.trim().is_empty() {
            let p = PathBuf::from(custom.trim());
            let _ = fs::create_dir_all(&p);
            return p;
        }
    }
    let p = get_box_home().join("runtimes");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_volumes_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BOX_VOLUMES_DIR") {
        if !custom.trim().is_empty() {
            let p = PathBuf::from(custom.trim());
            let _ = fs::create_dir_all(&p);
            return p;
        }
    }
    let p = get_box_home().join("volumes");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_config_file() -> PathBuf {
    get_box_home().join("config.json")
}

pub fn load_config() -> AppConfigData {
    bootstrap_dotenv();
    let cfg_file = get_config_file();
    if !cfg_file.exists() {
        return AppConfigData::default();
    }

    match fs::read_to_string(&cfg_file) {
        Ok(content) => serde_json::from_str::<AppConfigData>(&content).unwrap_or_default(),
        Err(_) => AppConfigData::default(),
    }
}

pub fn save_config(data: &AppConfigData) -> std::io::Result<()> {
    let cfg_file = get_config_file();
    if let Some(parent) = cfg_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_str = serde_json::to_string_pretty(data)?;
    fs::write(cfg_file, json_str)?;
    Ok(())
}

pub fn get_setting(key: &str) -> Option<String> {
    bootstrap_dotenv();

    // 1. Check environment variable first (Priority 1)
    let env_var = match key {
        "hub_url" => "BOX_HUB_URL",
        "compression_level" => "BOX_COMPRESSION_LEVEL",
        "build_workers" => "BOX_BUILD_WORKERS",
        "network_timeout" => "BOX_NETWORK_TIMEOUT",
        "python_mirror" => "BOX_PYTHON_MIRROR",
        "node_mirror" => "BOX_NODE_MIRROR",
        _ => "",
    };

    if !env_var.is_empty() {
        if let Ok(val) = std::env::var(env_var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 2. Check config.json (Priority 2) & Defaults (Priority 3)
    let cfg = load_config();
    match key {
        "hub_url" => cfg.hub_url.or_else(|| Some(DEFAULT_HUB_URL.to_string())),
        "compression_level" => cfg
            .compression_level
            .map(|v| v.to_string())
            .or_else(|| Some(DEFAULT_COMPRESSION_LEVEL.to_string())),
        "build_workers" => cfg.build_workers.map(|v| v.to_string()),
        "network_timeout" => cfg
            .network_timeout
            .map(|v| v.to_string())
            .or_else(|| Some(DEFAULT_NETWORK_TIMEOUT.to_string())),
        "python_mirror" => cfg.python_mirror,
        "node_mirror" => cfg.node_mirror,
        custom => cfg.custom.get(custom).and_then(|v| {
            if v.is_string() {
                v.as_str().map(|s| s.to_string())
            } else {
                Some(v.to_string())
            }
        }),
    }
}

pub fn set_setting(key: &str, value: &str) -> std::io::Result<()> {
    let mut cfg = load_config();
    match key {
        "hub_url" => cfg.hub_url = Some(value.to_string()),
        "compression_level" => cfg.compression_level = value.parse().ok(),
        "build_workers" => cfg.build_workers = value.parse().ok(),
        "network_timeout" => cfg.network_timeout = value.parse().ok(),
        "python_mirror" => cfg.python_mirror = Some(value.to_string()),
        "node_mirror" => cfg.node_mirror = Some(value.to_string()),
        custom => {
            cfg.custom.insert(
                custom.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    save_config(&cfg)
}

pub fn delete_setting(key: &str) -> std::io::Result<()> {
    let mut cfg = load_config();
    match key {
        "hub_url" => cfg.hub_url = None,
        "compression_level" => cfg.compression_level = None,
        "build_workers" => cfg.build_workers = None,
        "network_timeout" => cfg.network_timeout = None,
        "python_mirror" => cfg.python_mirror = None,
        "node_mirror" => cfg.node_mirror = None,
        custom => {
            cfg.custom.remove(custom);
        }
    }
    save_config(&cfg)
}
