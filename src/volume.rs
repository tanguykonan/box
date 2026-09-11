use console::style;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

use crate::config::get_volumes_dir;

pub fn get_volume_path(name: &str) -> PathBuf {
    get_volumes_dir().join(name)
}

fn calculate_dir_size<P: AsRef<Path>>(path: P) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut file_count = 0usize;

    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total_size += meta.len();
                file_count += 1;
            }
        }
    }
    (total_size, file_count)
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn handle_volume_create(name: &str) {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        eprintln!(
            "{}: Volume name cannot be empty.",
            style("Error").red().bold()
        );
        return;
    }

    if !clean_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        eprintln!(
            "{}: Volume name '{}' contains invalid characters. Use alphanumeric, '-' or '_'.",
            style("Error").red().bold(),
            clean_name
        );
        return;
    }

    let vol_path = get_volume_path(clean_name);
    if vol_path.exists() {
        println!(
            "{}: Volume '{}' already exists at {}",
            style("Notice").yellow().bold(),
            clean_name,
            vol_path.display()
        );
        return;
    }

    match fs::create_dir_all(&vol_path) {
        Ok(_) => {
            println!(
                "{} Created volume '{}' -> {}",
                style("[+]").green().bold(),
                style(clean_name).bold(),
                vol_path.display()
            );
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to create volume '{}': {}",
                style("Error").red().bold(),
                clean_name,
                e
            );
        }
    }
}

pub fn handle_volume_ls() {
    let volumes_dir = get_volumes_dir();
    let entries = match fs::read_dir(&volumes_dir) {
        Ok(e) => e,
        Err(_) => {
            println!("No volumes found.");
            return;
        }
    };

    let mut volume_list = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let (size, files) = calculate_dir_size(&path);
            volume_list.push((name, size, files, path));
        }
    }

    if volume_list.is_empty() {
        println!("No volumes found. Create one with `box volume create <name>`.");
        return;
    }

    volume_list.sort_by(|a, b| a.0.cmp(&b.0));

    println!(
        "{:<25} {:<15} {:<10} {}",
        style("VOLUME NAME").bold().cyan(),
        style("SIZE").bold().cyan(),
        style("FILES").bold().cyan(),
        style("LOCATION").bold().cyan()
    );
    println!("{}", style("-".repeat(80)).dim());

    for (name, size, files, path) in volume_list {
        println!(
            "{:<25} {:<15} {:<10} {}",
            style(name).bold().white(),
            format_bytes(size),
            files,
            style(path.display().to_string()).dim()
        );
    }
}

pub fn handle_volume_inspect(name: &str) {
    let vol_path = get_volume_path(name);
    if !vol_path.exists() {
        eprintln!(
            "{}: Volume '{}' not found.",
            style("Error").red().bold(),
            name
        );
        return;
    }

    let (size, file_count) = calculate_dir_size(&vol_path);
    let created = fs::metadata(&vol_path)
        .and_then(|m| m.created())
        .unwrap_or_else(|_| SystemTime::now());
    let datetime: chrono::DateTime<chrono::Local> = created.into();

    println!("{}", style(format!("Volume: {}", name)).bold().cyan());
    println!("  Mount Path: {}", vol_path.display());
    println!("  Total Size: {}", format_bytes(size));
    println!("  File Count: {}", file_count);
    println!("  Created At: {}", datetime.format("%Y-%m-%d %H:%M:%S"));
}

pub fn handle_volume_rm(names: &[String]) {
    for name in names {
        let vol_path = get_volume_path(name);
        if !vol_path.exists() {
            eprintln!(
                "{}: Volume '{}' does not exist.",
                style("Warning").yellow().bold(),
                name
            );
            continue;
        }

        match fs::remove_dir_all(&vol_path) {
            Ok(_) => {
                println!("{} Removed volume '{}'", style("[+]").green().bold(), name);
            }
            Err(e) => {
                eprintln!(
                    "{}: Could not remove volume '{}': {}",
                    style("Error").red().bold(),
                    name,
                    e
                );
            }
        }
    }
}

pub fn handle_volume_prune() {
    let volumes_dir = get_volumes_dir();
    let entries = match fs::read_dir(&volumes_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut removed = 0;
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let path = entry.path();
            let (size, files) = calculate_dir_size(&path);
            if size == 0 && files == 0 {
                let name = entry.file_name().to_string_lossy().to_string();
                if fs::remove_dir_all(&path).is_ok() {
                    println!(
                        "{} Pruned empty volume '{}'",
                        style("[+]").green().bold(),
                        name
                    );
                    removed += 1;
                }
            }
        }
    }

    if removed == 0 {
        println!("No empty volumes to prune.");
    } else {
        println!(
            "{} Pruned {} unused volumes.",
            style("[+]").green().bold(),
            removed
        );
    }
}

#[allow(dead_code)]
pub fn mount_volumes_into_sandbox(
    sandbox_dir: &Path,
    volume_args: &[String],
) -> std::io::Result<()> {
    for vol_spec in volume_args {
        let (source_raw, target_raw) = match vol_spec.rsplit_once(':') {
            Some((s, t)) if !s.is_empty() && !t.is_empty() => {
                (s.trim(), t.trim().trim_start_matches(['/', '\\']))
            }
            _ => {
                eprintln!(
                    "{}: Invalid volume format '{}'. Expected <name_or_host_path>:<target_dir>",
                    style("Warning").yellow().bold(),
                    vol_spec
                );
                continue;
            }
        };

        // Resolve source: named volume or host path
        let source_path = if Path::new(source_raw).is_dir() {
            PathBuf::from(source_raw)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(source_raw))
        } else {
            let p = get_volume_path(source_raw);
            fs::create_dir_all(&p)?;
            p
        };

        let target_path = sandbox_dir.join(target_raw);
        fs::create_dir_all(&target_path)?;

        // Copy / synchronize existing files from host/volume to sandbox target
        for entry in WalkDir::new(&source_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let rel = match entry.path().strip_prefix(&source_path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            let dest_entry = target_path.join(rel);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest_entry)?;
            } else if entry.file_type().is_file() {
                if let Some(p) = dest_entry.parent() {
                    fs::create_dir_all(p)?;
                }
                let _ = fs::copy(entry.path(), &dest_entry);
            }
        }
    }
    Ok(())
}
