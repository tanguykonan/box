use crate::config::{get_setting, load_config, save_config};
use crate::format::{read_box_manifest, sha256_file};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const USER_AGENT: &str = concat!("box-cli/", env!("CARGO_PKG_VERSION"));

fn get_target_hub(hub_override: Option<&str>) -> Result<String, String> {
    let hub = if let Some(h) = hub_override {
        h.to_string()
    } else if let Some(h) = get_setting("hub_url") {
        h
    } else {
        std::env::var("BOX_HUB_URL").unwrap_or_default()
    };

    let trimmed = hub.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err(
            "Registry Hub URL is not configured. Set BOX_HUB_URL in your .env (e.g. BOX_HUB_URL=https://boxhub.paxiz.org) or run `box config set hub_url <url>` or use `box login --hub <url>`.".to_string()
        );
    }
    Ok(trimmed)
}

fn format_http_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            if let Ok(body) = resp.into_string() {
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
                        return format!("HTTP {}: {}", code, err);
                    }
                    if let Some(msg) = json.get("message").and_then(|m| m.as_str()) {
                        return format!("HTTP {}: {}", code, msg);
                    }
                }
                if body.to_lowercase().contains("<html") {
                    let desc = match code {
                        401 => "Authentication required or token expired.",
                        403 => "Permission denied.",
                        404 => "Route or package not found on registry.",
                        413 => "Payload too large for registry.",
                        500 => "Internal server error on registry.",
                        _ => "Registry server returned HTML response.",
                    };
                    return format!("HTTP {}: {}", code, desc);
                }
                return format!(
                    "HTTP {}: {}",
                    code,
                    body.chars().take(200).collect::<String>()
                );
            }
            format!("HTTP {}", code)
        }
        ureq::Error::Transport(t) => format!("Network error: {}", t),
    }
}

pub fn login(token_opt: Option<&str>, hub_opt: Option<&str>) -> Result<(), String> {
    let target_hub = get_target_hub(hub_opt)?;
    let mut cfg = load_config();

    println!(
        "\n{}",
        style("Box Cloud Registry Authentication").bold().cyan()
    );

    let token = match token_opt {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => {
            println!(
                "Generate your Personal Access Token on {}",
                style(format!("{}/settings/tokens", target_hub))
                    .underlined()
                    .bold()
            );
            print!("Enter your Box Access Token: ");
            std::io::stdout().flush().map_err(|e| e.to_string())?;

            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .map_err(|e| e.to_string())?;
            input.trim().to_string()
        }
    };

    if token.is_empty() {
        return Err("Token cannot be empty.".to_string());
    }

    println!("{}", style("Verifying credentials...").dim());
    let whoami_url = format!("{}/api/v1/auth/whoami", target_hub);

    let resp = ureq::get(&whoami_url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(format_http_error)?;

    let body: Value = resp
        .into_json()
        .map_err(|e| format!("Invalid JSON response: {e}"))?;
    let username = body
        .get("username")
        .and_then(|u| u.as_str())
        .or_else(|| {
            body.get("user")
                .and_then(|u| u.get("username"))
                .and_then(|u| u.as_str())
        })
        .ok_or_else(|| "Could not determine user identity from registry response.".to_string())?
        .to_string();

    cfg.hub_url = Some(target_hub.clone());
    cfg.auth.insert(
        target_hub,
        crate::config::AuthCredential {
            username: username.clone(),
            token,
        },
    );
    save_config(&cfg).map_err(|e| e.to_string())?;

    println!(
        "{} Successfully logged in as {}\n",
        style("[+]").green().bold(),
        style(&username).white().bold()
    );
    Ok(())
}

pub fn logout(hub_opt: Option<&str>) -> Result<(), String> {
    let target_hub = get_target_hub(hub_opt)?;
    let mut cfg = load_config();

    if cfg.auth.remove(&target_hub).is_some() {
        save_config(&cfg).map_err(|e| e.to_string())?;
        println!(
            "{} Successfully logged out from {}.",
            style("[+]").green().bold(),
            target_hub
        );
    } else {
        println!("Not currently logged in to {}.", target_hub);
    }
    Ok(())
}

pub fn whoami() -> Result<(), String> {
    let target_hub = get_target_hub(None)?;
    let cfg = load_config();

    if let Some(cred) = cfg.auth.get(&target_hub) {
        println!(
            "Logged in as {} ({})",
            style(&cred.username).green().bold(),
            style(&target_hub).dim()
        );
    } else {
        println!(
            "Not logged in. Run `box login` to authenticate with {}.",
            target_hub
        );
    }
    Ok(())
}

fn resolve_push_tag(
    tag: &str,
    current_username: Option<&str>,
) -> Result<(String, String, String), String> {
    let (main_part, version) = if let Some((m, v)) = tag.split_once(':') {
        (m, v.to_string())
    } else {
        (tag, "latest".to_string())
    };

    if let Some((ns, pkg)) = main_part.split_once('/') {
        Ok((ns.to_string(), pkg.to_string(), version))
    } else if let Some(user) = current_username {
        Ok((user.to_string(), main_part.to_string(), version))
    } else {
        Err(format!(
            "Not logged in. Specify full tag as <username>/{}:{} or run `box login` first.",
            main_part, version
        ))
    }
}

pub fn push(
    tag: &str,
    box_path_opt: Option<&str>,
    is_public: bool,
    is_private: Option<bool>,
) -> Result<(), String> {
    let target_hub = get_target_hub(None)?;
    let cfg = load_config();
    let auth = cfg
        .auth
        .get(&target_hub)
        .ok_or_else(|| "Authentication required. Please run `box login` first.".to_string())?;

    let (namespace, pkg_name, version) = resolve_push_tag(tag, Some(&auth.username))?;
    let full_tag = format!("{}/{}:{}", namespace, pkg_name, version);

    // Locate .box file
    let file_to_push = if let Some(p) = box_path_opt {
        PathBuf::from(p)
    } else {
        let candidates = [
            PathBuf::from(format!("{}.box", pkg_name)),
            PathBuf::from(format!("{}-{}.box", pkg_name, version)),
            PathBuf::from(format!("{}_{}.box", pkg_name, version)),
        ];
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| format!("Could not find .box file for '{}'. Specify path explicitly: `box push {} path/to/file.box`", pkg_name, tag))?
    };

    let file_bytes = fs::read(&file_to_push)
        .map_err(|e| format!("Failed to read file {}: {e}", file_to_push.display()))?;
    let file_size_mb = file_bytes.len() as f64 / (1024.0 * 1024.0);
    let checksum = format!(
        "sha256:{}",
        sha256_file(&file_to_push).map_err(|e| e.to_string())?
    );

    // Read manifest
    let manifest_val = read_box_manifest(&file_to_push).unwrap_or_else(|_| serde_json::json!({}));

    let runtime_type = manifest_val
        .get("runtime")
        .and_then(|r| r.get("type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "PYTHON".to_string());

    let runtime_ver = manifest_val
        .get("runtime")
        .and_then(|r| r.get("version"))
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v.to_string()
            }
        })
        .unwrap_or_else(|| "3.12".to_string());

    let desc = manifest_val
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Box package {}", pkg_name));

    let category = manifest_val
        .get("category")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "BOTS".to_string());

    let is_protected = manifest_val
        .get("protect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let is_private_final = if is_public {
        false
    } else if let Some(p) = is_private {
        p
    } else {
        manifest_val
            .get("private")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    };

    let readme_content = manifest_val
        .get("readme")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| fs::read_to_string("README.md").ok())
        .or_else(|| fs::read_to_string("readme.md").ok())
        .unwrap_or_default();

    println!(
        "\n{} {} ({:.2} MB)...",
        style("Pushing").bold(),
        style(&full_tag).cyan().bold(),
        file_size_mb
    );

    // Build multipart/form-data boundary
    let boundary = format!("----BoxCliBoundary{}", hex::encode(rand_bytes(16)));
    let mut body = Vec::new();

    let fields = [
        ("name", pkg_name.as_str()),
        (
            "version",
            if version != "latest" {
                version.as_str()
            } else {
                "1.0.0"
            },
        ),
        ("tag", version.as_str()),
        ("description", desc.as_str()),
        ("runtime", runtime_type.as_str()),
        ("runtimeVersion", runtime_ver.as_str()),
        ("category", category.as_str()),
        ("isPrivate", if is_private_final { "true" } else { "false" }),
        ("isProtected", if is_protected { "true" } else { "false" }),
        ("readme", readme_content.as_str()),
        ("checksum", checksum.as_str()),
        (
            "manifest",
            &serde_json::to_string(&manifest_val).unwrap_or_default(),
        ),
    ];

    for (name, val) in fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(val.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    // Append file
    let file_name = file_to_push
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "package.box".to_string());
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            file_name
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let push_url = format!("{}/api/v1/registry/push", target_hub);
    let resp = ureq::post(&push_url)
        .set("Authorization", &format!("Bearer {}", auth.token))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={}", boundary),
        )
        .set("X-Box-Tag", &full_tag)
        .set("User-Agent", USER_AGENT)
        .send_bytes(&body)
        .map_err(format_http_error)?;

    let res_json: Value = resp
        .into_json()
        .map_err(|e| format!("Invalid JSON response: {e}"))?;
    let digest = res_json
        .get("version")
        .and_then(|v| v.get("checksum").or_else(|| v.get("checksumSha256")))
        .and_then(|c| c.as_str())
        .or_else(|| res_json.get("digest").and_then(|d| d.as_str()))
        .unwrap_or("sha256:...");

    let visibility = if is_private_final {
        "PRIVATE"
    } else {
        "PUBLIC"
    };
    println!(
        "{} Successfully pushed {} (Visibility: {}, Digest: {})\n",
        style("[+]").green().bold(),
        style(&full_tag).bold(),
        style(visibility).yellow(),
        style(&digest.chars().take(19).collect::<String>()).dim()
    );
    Ok(())
}

pub fn pull(tag: &str, output_path: Option<&str>) -> Result<PathBuf, String> {
    let target_hub = get_target_hub(None)?;
    let cfg = load_config();
    let token = cfg.auth.get(&target_hub).map(|a| a.token.clone());

    let (author, pkg_name, version) =
        resolve_push_tag(tag, cfg.auth.get(&target_hub).map(|a| a.username.as_str()))?;
    let full_tag = format!("{}/{}:{}", author, pkg_name, version);
    let dest_file = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}.box", pkg_name)));

    println!(
        "\n{} {}...",
        style("Pulling").bold(),
        style(&full_tag).cyan().bold()
    );

    let pull_url = format!(
        "{}/api/v1/registry/pull/{}/{}/{}",
        target_hub, author, pkg_name, version
    );
    let mut req = ureq::get(&pull_url).set("User-Agent", USER_AGENT);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {}", t));
    }

    let resp = req.call().map_err(format_http_error)?;
    let content_type = resp.content_type().to_string();

    if content_type.contains("application/json") {
        let json_val: Value = resp.into_json().map_err(|e| e.to_string())?;
        if let Some(download_url) = json_val.get("downloadUrl").and_then(|u| u.as_str()) {
            let size = json_val.get("sizeBytes").and_then(|s| s.as_u64());
            download_file_with_progress(download_url, &dest_file, size)?;
            println!(
                "{} Downloaded {} -> {}\n",
                style("[+]").green().bold(),
                full_tag,
                dest_file.display()
            );
            return Ok(dest_file);
        }
        Err("Registry response did not contain downloadUrl".to_string())
    } else {
        // Direct binary stream
        let total_size = resp.header("Content-Length").and_then(|l| l.parse().ok());
        let mut reader = resp.into_reader();
        let mut out = File::create(&dest_file).map_err(|e| e.to_string())?;

        let pb = if let Some(len) = total_size {
            let pb = ProgressBar::new(len);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap(),
            );
            pb.set_message("Downloading .box package...");
            Some(pb)
        } else {
            None
        };

        let mut buf = [0u8; 65536];
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            if let Some(ref pb) = pb {
                pb.inc(n as u64);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message("Download complete.");
        }

        println!(
            "{} Downloaded {} -> {}\n",
            style("[+]").green().bold(),
            full_tag,
            dest_file.display()
        );
        Ok(dest_file)
    }
}

fn download_file_with_progress(
    url: &str,
    dest_file: &Path,
    expected_size: Option<u64>,
) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("Download stream error: {e}"))?;

    let total_size =
        expected_size.or_else(|| resp.header("Content-Length").and_then(|l| l.parse().ok()));
    let pb = ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap(),
    );
    pb.set_message("Downloading .box package...");

    let mut reader = resp.into_reader();
    let mut out = File::create(dest_file).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];

    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        pb.inc(n as u64);
    }

    pb.finish_with_message("Download complete.");
    Ok(())
}

fn rand_bytes(len: usize) -> Vec<u8> {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(42);
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(((seed.wrapping_add((i * 31) as u128)) & 0xFF) as u8);
    }
    out
}
