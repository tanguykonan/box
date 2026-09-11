#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(rename = "type", default = "default_runtime_type")]
    pub runtime_type: String,
    #[serde(default = "default_runtime_version")]
    pub version: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
}

fn default_runtime_type() -> String {
    "python".to_string()
}

fn default_runtime_version() -> String {
    "3.12".to_string()
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_type: default_runtime_type(),
            version: default_runtime_version(),
            entrypoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default = "default_app_root")]
    pub root: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_app_root() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvVarSpec {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VolumeSpec {
    #[serde(default)]
    pub mount: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoxConfig {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub protect: bool,
    #[serde(default)]
    pub private: Option<bool>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub dependencies: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub env_schema: Option<String>,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default, deserialize_with = "deserialize_env")]
    pub env: HashMap<String, EnvVarSpec>,
    #[serde(default, deserialize_with = "deserialize_volumes")]
    pub volumes: HashMap<String, VolumeSpec>,
    #[serde(default)]
    pub build: HashMap<String, serde_json::Value>,
}

fn deserialize_env<'de, D>(deserializer: D) -> Result<HashMap<String, EnvVarSpec>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    let mut map = HashMap::new();

    if let serde_json::Value::Object(obj) = val {
        for (k, v) in obj {
            if let Ok(spec) = serde_json::from_value::<EnvVarSpec>(v.clone()) {
                map.insert(k, spec);
            } else if let Some(s) = v.as_str() {
                map.insert(
                    k,
                    EnvVarSpec {
                        description: None,
                        required: false,
                        default: Some(s.to_string()),
                    },
                );
            }
        }
    } else if let serde_json::Value::Array(arr) = val {
        for item in arr {
            if let Some(s) = item.as_str() {
                if let Some((k, v)) = s.split_once('=') {
                    map.insert(
                        k.trim().to_string(),
                        EnvVarSpec {
                            description: None,
                            required: false,
                            default: Some(v.trim().to_string()),
                        },
                    );
                } else {
                    map.insert(
                        s.trim().to_string(),
                        EnvVarSpec {
                            description: None,
                            required: false,
                            default: None,
                        },
                    );
                }
            }
        }
    }

    Ok(map)
}

fn deserialize_volumes<'de, D>(deserializer: D) -> Result<HashMap<String, VolumeSpec>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    let mut map = HashMap::new();

    if let serde_json::Value::Object(obj) = val {
        for (k, v) in obj {
            if let Ok(spec) = serde_json::from_value::<VolumeSpec>(v) {
                map.insert(k, spec);
            }
        }
    } else if let serde_json::Value::Array(arr) = val {
        for item in arr {
            if let Some(s) = item.as_str() {
                if let Some((vol_name, mount_path)) = s.split_once(':') {
                    map.insert(
                        vol_name.trim().to_string(),
                        VolumeSpec {
                            mount: mount_path.trim().to_string(),
                            description: None,
                        },
                    );
                }
            }
        }
    }

    Ok(map)
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_category() -> String {
    "BOTS".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxManifest {
    pub name: String,
    pub version: String,
    #[serde(default = "default_tag")]
    pub tag: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(rename = "isProtected", default)]
    pub is_protected: bool,
    #[serde(rename = "isPrivate", default = "default_true")]
    pub is_private: bool,
    pub runtime: RuntimeConfig,
    pub entrypoint: String,
    #[serde(default)]
    pub env_variables: HashMap<String, EnvVarSpec>,
    #[serde(default)]
    pub volumes: HashMap<String, VolumeSpec>,
    #[serde(default)]
    pub files: Vec<ManifestFileEntry>,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "checksumSha256", default)]
    pub checksum_sha256: String,
    #[serde(default)]
    pub readme: Option<String>,
}

fn default_tag() -> String {
    "latest".to_string()
}

fn default_true() -> bool {
    true
}
