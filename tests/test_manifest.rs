use box_cli::manifest::{BoxConfig, BoxManifest};

#[test]
fn test_boxfile_yaml_deserialization_standard() {
    let yaml = r#"
name: my-service
version: 2.1.0
category: WEB_APIS
protect: true
runtime:
  type: node
  version: "20.11"
  entrypoint: dist/server.js
env:
  PORT:
    default: "8080"
    required: false
  DATABASE_URL:
    required: true
volumes:
  data:
    mount: /app/data
"#;

    let config: BoxConfig = serde_yaml::from_str(yaml).expect("Failed to deserialize BoxConfig");
    assert_eq!(config.name, "my-service");
    assert_eq!(config.version, "2.1.0");
    assert_eq!(config.category, "WEB_APIS");
    assert!(config.protect);
    assert_eq!(config.runtime.runtime_type, "node");
    assert_eq!(config.runtime.version, "20.11");
    assert_eq!(config.runtime.entrypoint.as_deref(), Some("dist/server.js"));

    assert_eq!(config.env.len(), 2);
    let port_var = config.env.get("PORT").expect("PORT missing");
    assert_eq!(port_var.default.as_deref(), Some("8080"));
    assert!(!port_var.required);

    let db_var = config
        .env
        .get("DATABASE_URL")
        .expect("DATABASE_URL missing");
    assert!(db_var.required);

    assert_eq!(config.volumes.len(), 1);
    let vol = config.volumes.get("data").expect("data volume missing");
    assert_eq!(vol.mount, "/app/data");
}

#[test]
fn test_boxfile_yaml_array_env_and_volumes() {
    let yaml = r#"
name: short-format-app
runtime:
  type: python
  version: "3.11"
env:
  - DEBUG=true
  - API_KEY
volumes:
  - app_cache:/cache
"#;

    let config: BoxConfig = serde_yaml::from_str(yaml).expect("Failed to parse array format");
    assert_eq!(config.name, "short-format-app");
    assert_eq!(config.version, "1.0.0"); // Default version
    assert_eq!(config.category, "BOTS"); // Default category

    assert_eq!(config.env.len(), 2);
    let debug_var = config.env.get("DEBUG").unwrap();
    assert_eq!(debug_var.default.as_deref(), Some("true"));
    let api_key_var = config.env.get("API_KEY").unwrap();
    assert_eq!(api_key_var.default, None);

    assert_eq!(config.volumes.len(), 1);
    let cache_vol = config.volumes.get("app_cache").unwrap();
    assert_eq!(cache_vol.mount, "/cache");
}

#[test]
fn test_manifest_metadata_deserialization() {
    let json_data = r#"{
        "name": "packaged-bot",
        "version": "1.0.0",
        "tag": "v1.0",
        "description": "A packaged discord bot",
        "category": "BOTS",
        "isProtected": true,
        "isPrivate": true,
        "runtime": {
            "type": "python",
            "version": "3.12"
        },
        "entrypoint": "bot.py",
        "env_variables": {},
        "volumes": {},
        "files": [
            {
                "path": "bot.py",
                "size": 128,
                "sha256": "abc123"
            }
        ],
        "createdAt": "2026-09-10T12:00:00Z",
        "checksumSha256": "sha256_mock_hash"
    }"#;

    let manifest: BoxManifest =
        serde_json::from_str(json_data).expect("Failed to parse BoxManifest");
    assert_eq!(manifest.name, "packaged-bot");
    assert_eq!(manifest.tag, "v1.0");
    assert!(manifest.is_protected);
    assert!(manifest.is_private);
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].path, "bot.py");
}
