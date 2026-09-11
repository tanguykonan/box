use box_cli::config::{
    delete_setting, get_box_home, get_config_file, get_runtimes_dir, get_setting, get_volumes_dir,
    load_config, save_config, set_setting, AppConfigData, AuthCredential,
};
use std::env;
use std::sync::Mutex;
use tempfile::tempdir;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_custom_box_home_and_directories() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let temp_home = tempdir().expect("Failed to create tempdir");
    let home_path_str = temp_home.path().to_string_lossy().to_string();

    env::set_var("BOX_HOME", &home_path_str);
    env::remove_var("BOX_RUNTIMES_DIR");
    env::remove_var("BOX_VOLUMES_DIR");

    let home = get_box_home();
    assert_eq!(home, temp_home.path());
    assert!(home.exists());

    let runtimes = get_runtimes_dir();
    assert_eq!(runtimes, temp_home.path().join("runtimes"));
    assert!(runtimes.exists());

    let volumes = get_volumes_dir();
    assert_eq!(volumes, temp_home.path().join("volumes"));
    assert!(volumes.exists());

    let config_file = get_config_file();
    assert_eq!(config_file, temp_home.path().join("config.json"));

    env::remove_var("BOX_HOME");
}

#[test]
fn test_config_save_load_and_settings() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let temp_home = tempdir().expect("Failed to create tempdir");
    let home_path_str = temp_home.path().to_string_lossy().to_string();
    env::set_var("BOX_HOME", &home_path_str);
    env::remove_var("BOX_HUB_URL");

    let mut auth_map = std::collections::HashMap::new();
    auth_map.insert(
        "registry.example.com".to_string(),
        AuthCredential {
            username: "tester".to_string(),
            token: "secret_token_123".to_string(),
        },
    );

    let config = AppConfigData {
        hub_url: Some("https://registry.example.com".to_string()),
        compression_level: Some(7),
        build_workers: Some(4),
        auth: auth_map,
        ..Default::default()
    };

    save_config(&config).expect("Failed to save config");

    let loaded = load_config();
    assert_eq!(
        loaded.hub_url.as_deref(),
        Some("https://registry.example.com")
    );
    assert_eq!(loaded.compression_level, Some(7));
    assert_eq!(loaded.build_workers, Some(4));
    assert!(loaded.auth.contains_key("registry.example.com"));

    // Test get_setting / set_setting / delete_setting
    set_setting("node_mirror", "https://nodejs.org/dist").expect("Failed to set setting");
    assert_eq!(
        get_setting("node_mirror").as_deref(),
        Some("https://nodejs.org/dist")
    );

    delete_setting("node_mirror").expect("Failed to delete setting");
    assert_eq!(get_setting("node_mirror"), None);

    env::remove_var("BOX_HOME");
}

#[test]
fn test_env_var_override_priority() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let temp_home = tempdir().expect("Failed to create tempdir");
    env::set_var("BOX_HOME", temp_home.path().to_string_lossy().to_string());

    set_setting("hub_url", "https://config-file-hub.com").expect("Failed to set config");

    // Without env var, it reads from config
    env::remove_var("BOX_HUB_URL");
    assert_eq!(
        get_setting("hub_url").as_deref(),
        Some("https://config-file-hub.com")
    );

    // With env var, env var takes priority
    env::set_var("BOX_HUB_URL", "https://env-override-hub.com");
    assert_eq!(
        get_setting("hub_url").as_deref(),
        Some("https://env-override-hub.com")
    );

    env::remove_var("BOX_HUB_URL");
    env::remove_var("BOX_HOME");
}
