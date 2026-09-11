use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn sha256_file<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 131072];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

pub fn create_box_archive(
    output_path: &Path,
    staging_dir: &Path,
    mut boxfile_data: serde_json::Value,
    compression_level: Option<i32>,
) -> Result<(String, String), String> {
    let comp_level = compression_level.unwrap_or(3);

    // 1. Compute canonical tree_sha256 from the files manifest
    let mut tree_hasher = Sha256::new();
    let files_arr = boxfile_data
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for item in &files_arr {
        let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let size = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let sha = item.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        let line = format!("{}:{}:{}\n", path, size, sha);
        tree_hasher.update(line.as_bytes());
    }
    let tree_sha256 = hex::encode(tree_hasher.finalize());

    // 2. Build payload TAR in memory to compute its exact stream SHA-256
    let mut payload_buf = Vec::new();
    {
        let mut payload_tar = tar::Builder::new(Cursor::new(&mut payload_buf));
        for item in &files_arr {
            let rel_path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let file_path = staging_dir.join(rel_path);
            if file_path.is_file() {
                let mut f = File::open(&file_path)
                    .map_err(|e| format!("Failed to read file {}: {e}", file_path.display()))?;
                let mut header = tar::Header::new_gnu();
                let meta = f.metadata().map_err(|e| e.to_string())?;
                header.set_size(meta.len());
                header.set_mode(0o644);
                header.set_cksum();
                let clean_rel = rel_path.replace('\\', "/");
                payload_tar
                    .append_data(&mut header, clean_rel, &mut f)
                    .map_err(|e| format!("Failed to add {rel_path} to tar: {e}"))?;
            }
        }
        payload_tar.finish().map_err(|e| e.to_string())?;
    }

    let payload_sha256 = sha256_bytes(&payload_buf);

    // 3. Embed checksums in manifest
    if let Some(obj) = boxfile_data.as_object_mut() {
        obj.insert(
            "payload_sha256".to_string(),
            serde_json::Value::String(payload_sha256.clone()),
        );
        obj.insert(
            "tree_sha256".to_string(),
            serde_json::Value::String(tree_sha256.clone()),
        );
    }

    let manifest_yaml = serde_yaml::to_string(&boxfile_data)
        .map_err(|e| format!("Failed to serialize boxfile.yml: {e}"))?;
    let manifest_bytes = manifest_yaml.as_bytes();

    // 4. Assemble complete TAR: boxfile.yml first, then reuse payload stream
    let mut final_tar_buf = Vec::new();
    {
        let mut final_tar = tar::Builder::new(Cursor::new(&mut final_tar_buf));

        // Add boxfile.yml
        let mut mf_header = tar::Header::new_gnu();
        mf_header.set_size(manifest_bytes.len() as u64);
        mf_header.set_mode(0o644);
        mf_header.set_cksum();
        final_tar
            .append_data(&mut mf_header, "boxfile.yml", Cursor::new(manifest_bytes))
            .map_err(|e| format!("Failed to add boxfile.yml to tar: {e}"))?;

        // Re-read members from payload TAR
        let mut src_archive = tar::Archive::new(Cursor::new(&payload_buf));
        for entry_res in src_archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry_res.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.to_path_buf();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|e| e.to_string())?;

            let mut header = entry.header().clone();
            final_tar
                .append_data(&mut header, path, Cursor::new(data))
                .map_err(|e| format!("Failed to assemble final tar: {e}"))?;
        }

        final_tar.finish().map_err(|e| e.to_string())?;
    }

    // 5. Compress with Zstandard
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let compressed = zstd::encode_all(Cursor::new(&final_tar_buf), comp_level)
        .map_err(|e| format!("Zstandard compression failed: {e}"))?;

    let mut out_file = File::create(output_path)
        .map_err(|e| format!("Failed to create {}: {e}", output_path.display()))?;
    out_file
        .write_all(&compressed)
        .map_err(|e| format!("Failed to write {}: {e}", output_path.display()))?;

    Ok((payload_sha256, tree_sha256))
}

pub fn read_box_manifest(box_path: &Path) -> Result<serde_json::Value, String> {
    let file =
        File::open(box_path).map_err(|e| format!("Failed to open {}: {e}", box_path.display()))?;
    let decompressed = zstd::decode_all(file)
        .map_err(|e| format!("Failed to decompress {}: {e}", box_path.display()))?;

    let mut archive = tar::Archive::new(Cursor::new(decompressed));
    for entry_res in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_res.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;

        if path == Path::new("boxfile.yml") || path == Path::new("boxfile.yaml") {
            let mut content = String::new();
            entry
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            let val: serde_json::Value = serde_yaml::from_str(&content)
                .map_err(|e| format!("Failed to parse boxfile.yml in archive: {e}"))?;
            return Ok(val);
        } else if path == Path::new("manifest.json") {
            let mut content = String::new();
            entry
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            let val: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse manifest.json in archive: {e}"))?;
            return Ok(val);
        }
    }

    Err(format!(
        "Neither boxfile.yml nor manifest.json found in {}",
        box_path.display()
    ))
}

pub fn extract_and_verify_box_archive(
    box_path: &Path,
    dest_dir: &Path,
    expected_payload_sha256: Option<&str>,
) -> Result<bool, String> {
    let file =
        File::open(box_path).map_err(|e| format!("Failed to open {}: {e}", box_path.display()))?;
    let decompressed = zstd::decode_all(file)
        .map_err(|e| format!("Failed to decompress {}: {e}", box_path.display()))?;

    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;

    // If expected_payload_sha256 is provided, verify the payload stream
    if let Some(expected_sha) = expected_payload_sha256 {
        let mut payload_buf = Vec::new();
        {
            let mut payload_tar = tar::Builder::new(Cursor::new(&mut payload_buf));
            let mut archive = tar::Archive::new(Cursor::new(&decompressed));
            for entry_res in archive.entries().map_err(|e| e.to_string())? {
                let mut entry = entry_res.map_err(|e| e.to_string())?;
                let path = entry.path().map_err(|e| e.to_string())?.to_path_buf();
                let path_str = path.to_string_lossy();
                if path_str != "boxfile.yml"
                    && path_str != "boxfile.yaml"
                    && path_str != "manifest.json"
                {
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                    let mut header = entry.header().clone();
                    payload_tar
                        .append_data(&mut header, path, Cursor::new(data))
                        .map_err(|e| e.to_string())?;
                }
            }
            payload_tar.finish().map_err(|e| e.to_string())?;
        }

        let actual_sha = sha256_bytes(&payload_buf);
        if actual_sha != expected_sha {
            return Ok(false);
        }
    }

    // Extract all non-boxfile.yml members
    let mut archive = tar::Archive::new(Cursor::new(&decompressed));
    for entry_res in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_res.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.to_path_buf();
        let path_str = path.to_string_lossy();
        if path_str != "boxfile.yml" && path_str != "boxfile.yaml" && path_str != "manifest.json" {
            let target_path = dest_dir.join(&path);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            entry
                .unpack(&target_path)
                .map_err(|e| format!("Failed to extract {}: {e}", path.display()))?;
        }
    }

    Ok(true)
}
