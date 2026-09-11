use crate::config::get_setting;
use crate::format::{create_box_archive, sha256_file};
use crate::runtimes::get_runtime_adapter;
use crate::ui::StreamEngine;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use walkdir::WalkDir;

fn format_bytes(bytes: u64) -> String {
    if bytes > 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

pub fn build(path_arg: Option<&str>) -> Result<PathBuf, String> {
    let start_total = Instant::now();
    let project_dir = match path_arg {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };

    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);

    let config_file = if project_dir.join("boxconfig.yml").is_file() {
        project_dir.join("boxconfig.yml")
    } else if project_dir.join("boxconfig.yaml").is_file() {
        project_dir.join("boxconfig.yaml")
    } else {
        return Err(format!(
            "boxconfig.yml not found in {}",
            project_dir.display()
        ));
    };

    let config_raw = fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read {}: {e}", config_file.display()))?;

    let config: serde_json::Value = serde_yaml::from_str(&config_raw)
        .map_err(|e| format!("Failed to parse boxconfig.yml: {e}"))?;

    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required field in boxconfig.yml: 'name'".to_string())?
        .to_string();

    let version = config
        .get("version")
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v.to_string()
            }
        })
        .unwrap_or_else(|| "1.0.0".to_string());

    let entrypoint = config
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required field in boxconfig.yml: 'entrypoint'".to_string())?
        .to_string();

    let runtime = config.get("runtime").cloned().unwrap_or_else(|| {
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

    let deps_config = config.get("dependencies");
    let include_arr = config.get("include").and_then(|v| v.as_array()).cloned();
    let env_schema = config
        .get("env_schema")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let workdir = config
        .get("workdir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let volumes_conf = config.get("volumes").cloned();
    let server_conf = config
        .get("server")
        .cloned()
        .or_else(|| config.get("port").cloned());
    let protect = config
        .get("protect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let adapter = get_runtime_adapter(&rt_type)?;

    let total_steps = if protect { 5 } else { 4 };
    let engine = StreamEngine::new("Building", &format!("{}:{}", name, version), total_steps);

    let step_files = engine.add_step("Collecting application files");
    let step_deps = engine.add_step(&format!("Resolving {} dependencies", rt_type));
    let step_protect = if protect {
        Some(engine.add_step("Compiling and protecting source code"))
    } else {
        None
    };
    let step_manifest = engine.add_step("Indexing files and computing SHA-256 hashes");
    let step_pack = engine.add_step("Compressing and packaging archive");

    println!();
    engine.start();

    let temp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create tempdir: {e}"))?;
    let staging_dir = temp_dir.path();

    // Step 1: Application files
    engine.start_step(step_files, "transferring application files...");
    let mut file_count = 0usize;
    let (includes, strict_include) = if let Some(arr) = include_arr {
        (
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
            true,
        )
    } else {
        let mut def = Vec::new();
        if project_dir.join("src").exists() {
            def.push("src".to_string());
        }
        if project_dir.join("app").exists() {
            def.push("app".to_string());
        }
        if def.is_empty() && project_dir.join(&entrypoint).exists() {
            def.push(entrypoint.clone());
        }
        (def, false)
    };

    for pattern in &includes {
        let clean_pat = pattern.trim_matches(|c| c == '/' || c == '\\');
        let src = project_dir.join(clean_pat);
        if !src.exists() {
            if strict_include {
                engine.fail_step(step_files, &format!("Included path not found: {}", pattern));
                engine.stop(None);
                return Err(format!("Included path not found: {}", pattern));
            }
            continue;
        }

        if src.is_dir() {
            let dst = staging_dir.join(clean_pat);
            fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
            for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if let Ok(rel) = p.strip_prefix(&src) {
                    if rel.as_os_str().is_empty() {
                        continue;
                    }
                    let target = dst.join(rel);
                    if entry.file_type().is_dir() {
                        let _ = fs::create_dir_all(&target);
                    } else if entry.file_type().is_file() {
                        if let Some(parent) = target.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        if fs::copy(p, &target).is_ok() {
                            file_count += 1;
                        }
                    }
                }
            }
        } else {
            let dst = staging_dir.join(clean_pat);
            if let Some(parent) = dst.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::copy(&src, &dst).is_ok() {
                file_count += 1;
            }
        }
    }

    // Ensure entrypoint is included if present
    let entry_src = project_dir.join(&entrypoint);
    let entry_dst = staging_dir.join(&entrypoint);
    if entry_src.is_file() && !entry_dst.exists() {
        if let Some(parent) = entry_dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&entry_src, &entry_dst).is_ok() {
            file_count += 1;
        }
    }

    if let Some(ref schema_rel) = env_schema {
        let schema_path = project_dir.join(schema_rel);
        if !schema_path.is_file() {
            engine.fail_step(
                step_files,
                &format!("Environment schema template not found: {}", schema_rel),
            );
            engine.stop(None);
            return Err(format!(
                "Environment schema template not found: {}",
                schema_rel
            ));
        }
        let dst = staging_dir.join("env.example");
        let _ = fs::copy(&schema_path, dst);
        file_count += 1;
    }

    engine.finish_step(
        step_files,
        &format!(
            "transferring src/: {} item(s) from {} path(s)",
            file_count,
            includes.len()
        ),
    );

    // Step 2: Dependencies via Runtime Adapter
    engine.start_step(
        step_deps,
        &format!("installing {} dependencies...", rt_type),
    );
    let engine_deps_clone = engine.clone();
    let dep_res = adapter.install_dependencies(
        &project_dir,
        staging_dir,
        &runtime,
        deps_config,
        Some(&|msg: &str| {
            engine_deps_clone.update_step_detail(step_deps, msg);
        }),
    );

    match dep_res {
        Ok((true, summary)) => {
            engine.finish_step(step_deps, &format!("# {}", summary));
        }
        Ok((false, err)) | Err(err) => {
            engine.fail_step(step_deps, &err);
            engine.stop(None);
            return Err(err);
        }
    }

    // Step 3: Code Protection (if enabled)
    if let Some(step_prot_idx) = step_protect {
        engine.start_step(step_prot_idx, "compiling and protecting source code...");
        let engine_prot_clone = engine.clone();
        let prot_res = adapter.protect_code(
            staging_dir,
            &runtime,
            Some(&|msg: &str| {
                engine_prot_clone.update_step_detail(step_prot_idx, msg);
            }),
        );

        match prot_res {
            Ok((true, summary)) => {
                engine.finish_step(step_prot_idx, &format!("bytecode: {}", summary));
            }
            Ok((false, err)) | Err(err) => {
                engine.fail_step(step_prot_idx, &err);
                engine.stop(None);
                return Err(err);
            }
        }
    }

    // Step 4: Fast Parallel Indexing and SHA-256
    engine.start_step(
        step_manifest,
        "indexing files and computing sha256 digests...",
    );
    let mut discovered_files = Vec::new();

    for entry in WalkDir::new(staging_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fp = entry.path().to_path_buf();
            if let Ok(rel) = fp.strip_prefix(staging_dir) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                discovered_files.push((fp, rel_str, size));
            }
        }
    }

    let total_files = discovered_files.len();

    let mut files_manifest: Vec<serde_json::Value> = discovered_files
        .into_par_iter()
        .map(|(fp, rel, size)| {
            let sha = sha256_file(&fp).unwrap_or_default();
            serde_json::json!({
                "path": rel,
                "size": size,
                "sha256": sha
            })
        })
        .collect();

    // Deterministic canonical sort by path
    files_manifest.sort_by(|a, b| {
        let pa = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pb = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
        pa.cmp(pb)
    });

    let threads = rayon::current_num_threads();
    engine.finish_step(
        step_manifest,
        &format!(
            "sha256: indexed {} files (parallel {} threads)",
            total_files, threads
        ),
    );

    // Prepare boxfile_data
    let mut boxfile_data = serde_json::json!({
        "name": name,
        "version": version,
        "runtime": runtime,
        "entrypoint": entrypoint,
        "files": files_manifest
    });

    if env_schema.is_some() {
        boxfile_data["env_schema"] = serde_json::Value::String("env.example".to_string());
    }
    if let Some(ref w) = workdir {
        boxfile_data["workdir"] = serde_json::Value::String(w.clone());
    }
    if let Some(ref v) = volumes_conf {
        boxfile_data["volumes"] = v.clone();
    }
    if let Some(ref s) = server_conf {
        boxfile_data["server"] = s.clone();
    }
    if protect {
        boxfile_data["protect"] = serde_json::Value::Bool(true);
    }

    // Step 5: Pack Archive with Fast Integrity
    engine.start_step(step_pack, "compressing archive (TAR + Zstandard)...");
    let output_path = PathBuf::from(format!("{}.box", name));

    let comp_level: Option<i32> = get_setting("compression_level").and_then(|v| v.parse().ok());

    create_box_archive(&output_path, staging_dir, boxfile_data, comp_level)?;

    let size_bytes = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    let size_label = format_bytes(size_bytes);

    engine.finish_step(
        step_pack,
        &format!(
            "writing {} ({}, TAR + Zstandard)",
            output_path.display(),
            size_label
        ),
    );

    let elapsed = start_total.elapsed().as_secs_f64();
    let summary = format!(
        "Built {} ({}) in {:.2}s",
        output_path.display(),
        size_label,
        elapsed
    );
    engine.stop(Some(&summary));

    Ok(output_path)
}
