use box_cli::format::{
    create_box_archive, extract_and_verify_box_archive, read_box_manifest, sha256_bytes,
    sha256_file,
};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_sha256_bytes_and_file() {
    let data = b"Hello, Box Engine!";
    let hash = sha256_bytes(data);

    let dir = tempdir().expect("Failed to create tempdir");
    let file_path = dir.path().join("test_file.txt");
    fs::write(&file_path, data).expect("Failed to write test file");

    let file_hash = sha256_file(&file_path).expect("Failed to hash file");
    assert_eq!(hash, file_hash);
    assert_eq!(
        hash,
        "07922b03650bd75d4d7483be465ba6ff3813ccf5e3cce5c6e5ffd178de0530b1"
    );
}

#[test]
fn test_create_and_read_box_archive() {
    let dir = tempdir().expect("Failed to create tempdir");
    let staging_dir = dir.path().join("staging");
    fs::create_dir_all(&staging_dir).expect("Failed to create staging dir");

    let main_py = staging_dir.join("main.py");
    fs::write(&main_py, "print('hello world')\n").expect("Failed to write main.py");
    let main_sha = sha256_file(&main_py).expect("Failed to hash main.py");

    let boxfile_data = json!({
        "name": "test-app",
        "version": "1.0.0",
        "description": "Integration test app",
        "runtime": {
            "type": "python",
            "version": "3.12"
        },
        "entrypoint": "main.py",
        "files": [
            {
                "path": "main.py",
                "size": 21,
                "sha256": main_sha
            }
        ]
    });

    let box_archive_path = dir.path().join("test-app-1.0.0.box");

    let (payload_sha, tree_sha) =
        create_box_archive(&box_archive_path, &staging_dir, boxfile_data, Some(3))
            .expect("Failed to create box archive");

    assert!(!payload_sha.is_empty());
    assert!(!tree_sha.is_empty());
    assert!(box_archive_path.exists());

    // Verify reading manifest from .box
    let manifest_val = read_box_manifest(&box_archive_path).expect("Failed to read box manifest");
    assert_eq!(
        manifest_val.get("name").and_then(|v| v.as_str()),
        Some("test-app")
    );
    assert_eq!(
        manifest_val.get("version").and_then(|v| v.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        manifest_val.get("payload_sha256").and_then(|v| v.as_str()),
        Some(payload_sha.as_str())
    );
    assert_eq!(
        manifest_val.get("tree_sha256").and_then(|v| v.as_str()),
        Some(tree_sha.as_str())
    );

    // Test extraction and verification
    let extract_dir = dir.path().join("extracted");
    let is_valid =
        extract_and_verify_box_archive(&box_archive_path, &extract_dir, Some(&payload_sha))
            .expect("Extraction failed");

    assert!(is_valid);
    let extracted_main = extract_dir.join("main.py");
    assert!(extracted_main.exists());
    let extracted_content =
        fs::read_to_string(&extracted_main).expect("Failed to read extracted file");
    assert_eq!(extracted_content, "print('hello world')\n");
}

#[test]
fn test_archive_corrupted_payload_checksum() {
    let dir = tempdir().expect("Failed to create tempdir");
    let staging_dir = dir.path().join("staging");
    fs::create_dir_all(&staging_dir).expect("Failed to create staging dir");

    let file_path = staging_dir.join("data.txt");
    fs::write(&file_path, "sample content").unwrap();
    let file_sha = sha256_file(&file_path).unwrap();

    let boxfile_data = json!({
        "name": "corrupt-test",
        "version": "0.1.0",
        "files": [
            {
                "path": "data.txt",
                "size": 14,
                "sha256": file_sha
            }
        ]
    });

    let box_archive_path = dir.path().join("corrupt-test.box");
    let (payload_sha, _) =
        create_box_archive(&box_archive_path, &staging_dir, boxfile_data, Some(1)).unwrap();

    let extract_dir = dir.path().join("extracted");
    // Verify with mismatched checksum
    let fake_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let is_valid =
        extract_and_verify_box_archive(&box_archive_path, &extract_dir, Some(fake_sha)).unwrap();

    assert!(
        !is_valid,
        "Integrity check must fail on mismatched checksum"
    );

    // Verify with actual checksum
    let is_valid_real =
        extract_and_verify_box_archive(&box_archive_path, &extract_dir, Some(&payload_sha))
            .unwrap();

    assert!(
        is_valid_real,
        "Integrity check must succeed on matching checksum"
    );
}
