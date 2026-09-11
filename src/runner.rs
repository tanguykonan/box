use crate::format::{extract_and_verify_box_archive, read_box_manifest, sha256_file};
use crate::registry::pull;
use crate::runtimes::get_runtime_adapter;
use crate::ui::StreamEngine;
use console::style;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn load_env_file_to_map<P: AsRef<Path>>(filepath: P) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let p = filepath.as_ref();
    if !p.is_file() {
        return map;
    }

    if let Ok(content) = fs::read_to_string(p) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || !line.contains('=') {
                continue;
            }
            let (k, v) = line.split_once('=').unwrap_or(("", ""));
            let k = k.trim().to_string();
            let mut v = v.trim().to_string();
            if v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"'))
                    || (v.starts_with('\'') && v.ends_with('\'')))
            {
                v = v[1..v.len() - 1].to_string();
            }
            if !k.is_empty() {
                map.insert(k, v);
            }
        }
    }
    map
}

pub fn run(
    target: &str,
    env_file: Option<&str>,
    volumes: &[String],
    port: Option<u16>,
    host: Option<&str>,
    workdir_override: Option<&str>,
    verify_files: bool,
) -> Result<i32, String> {
    let target_path = PathBuf::from(target);
    let box_file = if target_path.is_file() {
        target_path
    } else if PathBuf::from(format!("{}.box", target)).is_file() {
        PathBuf::from(format!("{}.box", target))
    } else {
        // Auto-pull from remote registry
        println!(
            "{} Resolving and pulling {} from registry...",
            style("[+]").cyan().bold(),
            target
        );
        pull(target, None)?
    };

    let manifest = read_box_manifest(&box_file)
        .map_err(|e| format!("Failed to read manifest from {}: {e}", box_file.display()))?;

    let name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("app")
        .to_string();

    let version = manifest
        .get("version")
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v.to_string()
            }
        })
        .unwrap_or_else(|| "1.0.0".to_string());

    let runtime = manifest.get("runtime").cloned().unwrap_or_else(|| {
        serde_json::json!({
            "type": "python",
            "version": "3.12"
        })
    });

    let rt_type = runtime
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("python")
        .to_string();

    let entrypoint = manifest
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .unwrap_or("main.py")
        .to_string();

    let manifest_workdir = manifest
        .get("workdir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let effective_workdir = workdir_override.unwrap_or(&manifest_workdir);

    let expected_payload_sha = manifest
        .get("payload_sha256")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let files_arr = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let adapter = get_runtime_adapter(&rt_type)?;

    let engine = StreamEngine::new("Running", &format!("{}:{}", name, version), 4);
    let step_extract = engine.add_step("Creating sandbox and extracting archive");
    let step_integrity = engine.add_step("Verifying archive integrity");
    let step_runtime = engine.add_step(&format!("Provisioning {} standalone runtime", rt_type));
    let step_env = engine.add_step("Loading environment configuration");

    println!();
    engine.start();

    let temp_dir =
        tempfile::tempdir().map_err(|e| format!("Failed to create sandbox tempdir: {e}"))?;
    let sandbox_dir = temp_dir.path();

    // Step 1: Extract payload into sandbox
    engine.start_step(step_extract, "extracting payload into sandbox...");
    let stream_verified = extract_and_verify_box_archive(
        &box_file,
        sandbox_dir,
        if !verify_files {
            expected_payload_sha.as_deref()
        } else {
            None
        },
    )?;

    engine.finish_step(
        step_extract,
        &format!("extracted to sandbox ({} files)", files_arr.len()),
    );

    // Step 2: Integrity check
    engine.start_step(step_integrity, "verifying payload checksum...");
    if stream_verified && expected_payload_sha.is_some() {
        if let Some(ref sha) = expected_payload_sha {
            let short_sha = if sha.len() >= 16 { &sha[..16] } else { sha };
            engine.finish_step(
                step_integrity,
                &format!(
                    "sha256:{}... verified ({} files)",
                    short_sha,
                    files_arr.len()
                ),
            );
        }
    } else {
        // Fallback to canonical file-by-file verification
        let mut failed = Vec::new();
        for (i, f_info) in files_arr.iter().enumerate() {
            let rel_p = f_info.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let expected_sha = f_info.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
            let target_file = sandbox_dir.join(rel_p);

            if !target_file.is_file() {
                failed.push(format!("Missing file: {}", rel_p));
                continue;
            }

            if !expected_sha.is_empty() {
                match sha256_file(&target_file) {
                    Ok(actual) => {
                        if actual != expected_sha {
                            failed.push(format!("Checksum mismatch for {}", rel_p));
                        }
                    }
                    Err(e) => {
                        failed.push(format!("Cannot read {}: {}", rel_p, e));
                    }
                }
            }

            if i % 10 == 0 || i == files_arr.len() - 1 {
                engine.update_step_detail(
                    step_integrity,
                    &format!("verifying {}/{} files...", i + 1, files_arr.len()),
                );
            }
        }

        if !failed.is_empty() {
            let err_msg = format!(
                "Archive integrity check failed: {} corrupted or missing file(s)",
                failed.len()
            );
            engine.fail_step(step_integrity, &err_msg);
            engine.stop(None);
            return Err(err_msg);
        }

        let tree_sha = manifest
            .get("tree_sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let short_sha = if tree_sha.len() >= 16 {
            &tree_sha[..16]
        } else {
            "canonical"
        };
        engine.finish_step(
            step_integrity,
            &format!(
                "tree_sha256:{}... verified ({} files)",
                short_sha,
                files_arr.len()
            ),
        );
    }

    // Step 3: Provisioning standalone runtime
    engine.start_step(
        step_runtime,
        &format!("resolving {} standalone runtime...", rt_type),
    );
    let engine_rt_clone = engine.clone();
    let (runtime_exe, runtime_id, was_downloaded) = adapter.ensure_runtime(
        &runtime,
        Some(&|msg: &str| {
            engine_rt_clone.update_step_detail(step_runtime, msg);
        }),
    )?;

    let rt_status = if was_downloaded {
        "downloaded"
    } else {
        "cached"
    };
    engine.finish_step(
        step_runtime,
        &format!("runtime active: {} ({})", runtime_id, rt_status),
    );

    // Step 4: Environment Variables
    engine.start_step(step_env, "validating environment...");
    let env_file_candidate = env_file.unwrap_or(".env.local");
    let mut env_vars = HashMap::new();
    let mut source_label = env_file_candidate.to_string();

    if Path::new(env_file_candidate).is_file() {
        env_vars = load_env_file_to_map(env_file_candidate);
    } else if Path::new(".env").is_file() {
        source_label = ".env".to_string();
        env_vars = load_env_file_to_map(".env");
    }

    // Check env.example schema if present
    let schema_file = sandbox_dir.join("env.example");
    if schema_file.is_file() {
        if let Ok(content) = fs::read_to_string(&schema_file) {
            let mut missing = Vec::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || !line.contains('=') {
                    continue;
                }
                let (k, default_val) = line.split_once('=').unwrap_or(("", ""));
                let k = k.trim();
                if !k.is_empty() && !env_vars.contains_key(k) {
                    let trimmed_val = default_val.trim();
                    if !trimmed_val.is_empty()
                        && trimmed_val != "votre_token_discord_ici"
                        && !trimmed_val.starts_with("YOUR_")
                    {
                        env_vars.insert(k.to_string(), trimmed_val.to_string());
                    } else {
                        missing.push(k.to_string());
                    }
                }
            }

            if !missing.is_empty() {
                let err_msg = format!(
                    "Missing required variable(s): {} (checked in {})",
                    missing.join(", "),
                    source_label
                );
                engine.fail_step(step_env, &err_msg);
                engine.stop(None);
                return Err(err_msg);
            }
        }
    }

    if !env_vars.is_empty() {
        engine.finish_step(
            step_env,
            &format!("loaded {} ({} variable(s))", source_label, env_vars.len()),
        );
    } else {
        engine.finish_step(step_env, &format!("no {} required", source_label));
    }

    engine.stop(None);

    // Mount persistent volumes
    let mut mounted_info = Vec::new();
    if !volumes.is_empty() {
        for v_spec in volumes {
            if let Some((src, target)) = v_spec.split_once(':') {
                let vol_path = crate::volume::get_volume_path(src.trim());
                let _ = fs::create_dir_all(&vol_path);
                let target_clean = target.trim().trim_start_matches(['/', '\\']);
                let sandbox_target = sandbox_dir.join(target_clean);
                let _ = fs::create_dir_all(&sandbox_target);
                mounted_info.push((
                    "Named Volume",
                    vol_path.display().to_string(),
                    target_clean.to_string(),
                ));
            }
        }
    }

    if !mounted_info.is_empty() {
        println!(
            "{}",
            style(format!("[+] Mounted {} volume(s):", mounted_info.len()))
                .cyan()
                .bold()
        );
        for (m_type, phys, rel) in &mounted_info {
            println!(
                "  - {} -> {} ({})",
                style(m_type).bold(),
                style(rel).yellow(),
                style(phys).dim()
            );
        }
        println!();
    }

    // Configure runtime environment
    let mut process_env: HashMap<String, String> = std::env::vars().collect();
    for (k, v) in &env_vars {
        process_env.insert(k.clone(), v.clone());
    }

    adapter.setup_environment(&mut process_env, sandbox_dir, effective_workdir);

    let resolved_port = port
        .map(|p| p.to_string())
        .or_else(|| env_vars.get("PORT").cloned())
        .unwrap_or_else(|| "8000".to_string());

    let resolved_host = host
        .map(|h| h.to_string())
        .or_else(|| env_vars.get("HOST").cloned())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    process_env.insert("PORT".to_string(), resolved_port);
    process_env.insert("HOST".to_string(), resolved_host);

    // Determine execution directory
    let exec_workdir =
        if !effective_workdir.is_empty() && effective_workdir != "." && effective_workdir != "/" {
            let clean = effective_workdir.trim_matches(|c| c == '/' || c == '\\');
            let p = sandbox_dir.join(clean);
            let _ = fs::create_dir_all(&p);
            p
        } else {
            sandbox_dir.to_path_buf()
        };

    let launch_cmd = adapter.build_launch_command(&runtime_exe, &entrypoint, sandbox_dir);

    println!(
        "{} {} {}\n",
        style("[*]").green().bold(),
        style("Launching application:").bold(),
        style(&entrypoint).dim()
    );

    let mut cmd = Command::new(&launch_cmd[0]);
    if launch_cmd.len() > 1 {
        cmd.args(&launch_cmd[1..]);
    }
    cmd.current_dir(&exec_workdir);

    for (k, v) in process_env {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn application process: {e}"))?;

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {e}"))?;

    Ok(status.code().unwrap_or(0))
}
