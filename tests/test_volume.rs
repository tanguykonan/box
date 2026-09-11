use box_cli::volume::{get_volume_path, mount_volumes_into_sandbox};
use std::env;
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;

static VOL_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_volume_path_and_mounting() {
    let _lock = VOL_MUTEX.lock().unwrap();

    let temp_home = tempdir().expect("Failed to create tempdir");
    env::set_var("BOX_HOME", temp_home.path().to_string_lossy().to_string());

    let vol_name = "test_data_vol";
    let vol_path = get_volume_path(vol_name);
    assert_eq!(vol_path, temp_home.path().join("volumes").join(vol_name));

    // Pre-populate volume with files
    fs::create_dir_all(&vol_path).unwrap();
    fs::write(vol_path.join("database.sqlite"), b"SQLITE_TEST_DATA").unwrap();
    let nested_dir = vol_path.join("uploads");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(nested_dir.join("avatar.png"), b"PNG_TEST_DATA").unwrap();

    // Create a temporary sandbox dir
    let sandbox = tempdir().expect("Failed to create sandbox tempdir");
    let sandbox_path = sandbox.path();

    let mount_spec = vec![format!("{}:/app/storage", vol_name)];
    mount_volumes_into_sandbox(sandbox_path, &mount_spec).expect("Mounting failed");

    // Verify sandbox received files
    let mounted_db = sandbox_path.join("app/storage/database.sqlite");
    assert!(mounted_db.exists());
    assert_eq!(fs::read(&mounted_db).unwrap(), b"SQLITE_TEST_DATA");

    let mounted_avatar = sandbox_path.join("app/storage/uploads/avatar.png");
    assert!(mounted_avatar.exists());
    assert_eq!(fs::read(&mounted_avatar).unwrap(), b"PNG_TEST_DATA");

    env::remove_var("BOX_HOME");
}

#[test]
fn test_host_path_volume_mounting() {
    let _lock = VOL_MUTEX.lock().unwrap();

    let host_temp = tempdir().expect("Failed to create host dir");
    let host_file = host_temp.path().join("config.ini");
    fs::write(&host_file, "key=value").unwrap();

    let sandbox = tempdir().expect("Failed to create sandbox tempdir");
    let sandbox_path = sandbox.path();

    let mount_spec = vec![format!("{}:/mounted_host_dir", host_temp.path().display())];
    mount_volumes_into_sandbox(sandbox_path, &mount_spec).expect("Mounting host path failed");

    let dest_file = sandbox_path.join("mounted_host_dir/config.ini");
    assert!(dest_file.exists());
    assert_eq!(fs::read_to_string(&dest_file).unwrap(), "key=value");
}
