use crate::format::read_box_manifest;
use console::style;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn inspect_box(box_path: &str, json_output: bool) -> Result<(), String> {
    let p = Path::new(box_path);
    if !p.exists() {
        return Err(format!("File not found: {}", box_path));
    }

    let file_size_bytes = p.metadata().map(|m| m.len()).unwrap_or(0);
    let size_str = format_bytes(file_size_bytes);

    let manifest = read_box_manifest(p).map_err(|e| format!("Failed to read manifest: {e}"))?;

    // Decompress archive in memory to check files, .jsc, .pyc, env.example
    let mut has_jsc = false;
    let mut has_pyc = false;
    let mut env_schema_content = None;

    if let Ok(file) = File::open(p) {
        if let Ok(decompressed) = zstd::decode_all(file) {
            let mut archive = tar::Archive::new(Cursor::new(decompressed));
            if let Ok(entries) = archive.entries() {
                for entry_res in entries.flatten() {
                    if let Ok(path) = entry_res.path() {
                        let path_str = path.to_string_lossy();
                        if path_str.ends_with(".jsc") {
                            has_jsc = true;
                        } else if path_str.ends_with(".pyc") {
                            has_pyc = true;
                        } else if path_str == "env.example" {
                            let mut s = String::new();
                            let mut reader = entry_res;
                            if reader.read_to_string(&mut s).is_ok() {
                                env_schema_content = Some(s);
                            }
                        }
                    }
                }
            }
        }
    }

    let name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");
    let runtime_obj = manifest.get("runtime");
    let rt_type = runtime_obj
        .and_then(|r| r.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let rt_ver = runtime_obj
        .and_then(|r| r.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let entrypoint = manifest
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let workdir = manifest
        .get("workdir")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let payload_sha = manifest
        .get("payload_sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let tree_sha = manifest
        .get("tree_sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let files_arr = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total_files = files_arr.len();

    let is_protected = has_jsc
        || has_pyc
        || manifest
            .get("protect")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let protection_label = if has_jsc {
        "Protected (V8 Bytecode .jsc)"
    } else if has_pyc {
        "Protected (Python Bytecode .pyc)"
    } else if is_protected {
        "Protected (Bytecode)"
    } else {
        "Plain Source"
    };

    if json_output {
        let out = serde_json::json!({
            "name": name,
            "version": version,
            "file": p.to_string_lossy(),
            "size_bytes": file_size_bytes,
            "runtime": { "type": rt_type, "version": rt_ver },
            "entrypoint": entrypoint,
            "workdir": workdir,
            "protected": is_protected,
            "protection_type": protection_label,
            "files_count": total_files,
            "payload_sha256": payload_sha,
            "tree_sha256": tree_sha,
            "volumes": manifest.get("volumes"),
            "server": manifest.get("server"),
            "env_schema": env_schema_content
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }

    println!();
    println!(
        "{}: {} {} ({})",
        style("Package").dim().bold(),
        style(name).bold().cyan(),
        style(version).bold().yellow(),
        style(size_str).dim()
    );
    println!("{}", style("-".repeat(60)).dim());

    println!(
        "  {:<20} {} {}",
        style("Runtime:").bold(),
        style(rt_type).cyan(),
        style(format!("v{rt_ver}")).green().bold()
    );
    println!(
        "  {:<20} {} (workdir: {})",
        style("Entrypoint:").bold(),
        style(entrypoint).bold().white(),
        style(workdir).dim()
    );

    let prot_badge = if is_protected {
        format!(
            "{} {}",
            style("[PROTECTED]").green().bold(),
            protection_label
        )
    } else {
        format!("{} {}", style("[PLAIN SOURCE]").yellow(), protection_label)
    };
    println!("  {:<20} {}", style("Protection:").bold(), prot_badge);
    println!(
        "  {:<20} {} files packaged",
        style("Payload:").bold(),
        total_files
    );
    println!(
        "  {:<20} {}",
        style("Payload SHA-256:").bold(),
        style(payload_sha).dim()
    );
    if tree_sha != "N/A" {
        println!(
            "  {:<20} {}",
            style("Tree SHA-256:").bold(),
            style(tree_sha).dim()
        );
    }

    if let Some(srv) = manifest.get("server") {
        let host = srv
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1");
        let port = srv
            .get("port")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        println!(
            "  {:<20} Host: {} | Port: {}",
            style("Network:").bold(),
            style(host).bold(),
            style(port).cyan().bold()
        );
    }

    if let Some(serde_json::Value::Array(vols)) = manifest.get("volumes") {
        if !vols.is_empty() {
            println!(
                "\n{}",
                style("Declared Persistent Volumes:").bold().magenta()
            );
            for v in vols {
                if let Some(v_str) = v.as_str() {
                    if let Some((src, dst)) = v_str.split_once(':') {
                        println!("  • {:<20} -> {}", style(src).bold().cyan(), dst);
                    } else {
                        println!("  • {}", style(v_str).bold().cyan());
                    }
                }
            }
        }
    }

    if let Some(ref schema) = env_schema_content {
        println!(
            "\n{}",
            style("Environment Schema Template (.env.example):")
                .bold()
                .cyan()
        );
        for line in schema.lines().take(10) {
            println!("  {}", style(line).dim());
        }
        if schema.lines().count() > 10 {
            println!("  ... and {} more lines", schema.lines().count() - 10);
        }
    }

    println!();
    Ok(())
}
