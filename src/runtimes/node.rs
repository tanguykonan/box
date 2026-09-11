use crate::config::{get_runtimes_dir, get_setting};
use crate::runtimes::RuntimeAdapter;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub struct NodeRuntimeAdapter;

impl Default for NodeRuntimeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRuntimeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn detect_os_arch() -> (&'static str, &'static str) {
        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64"
        };

        (os, arch)
    }

    fn get_catalog_info(
        os: &str,
        arch: &str,
        version_req: &str,
    ) -> (String, String, String, String, String) {
        let full_ver = match version_req {
            "26" => "26.7.0",
            "24" => "24.0.0",
            "22" => "22.11.0",
            "20" => "20.18.0",
            "18" => "18.20.4",
            other => {
                if other.contains('.') {
                    other
                } else {
                    "20.18.0"
                }
            }
        };

        let custom_mirror = get_setting("node_mirror");
        let base_url = custom_mirror.unwrap_or_else(|| "https://nodejs.org/dist".to_string());
        let base_url = base_url.trim_end_matches('/');

        let (node_os, node_arch) = if os == "windows" {
            ("win", "x64")
        } else if os == "darwin" {
            ("darwin", if arch == "aarch64" { "arm64" } else { "x64" })
        } else {
            ("linux", if arch == "aarch64" { "arm64" } else { "x64" })
        };

        if os == "windows" {
            let filename = format!("node-v{}-{}-{}.zip", full_ver, node_os, node_arch);
            let url = format!("{}/v{}/{}", base_url, full_ver, filename);
            let inner_dir = format!("node-v{}-{}-{}", full_ver, node_os, node_arch);
            (
                full_ver.to_string(),
                url,
                "zip".to_string(),
                "node.exe".to_string(),
                inner_dir,
            )
        } else {
            let filename = format!("node-v{}-{}-{}.tar.gz", full_ver, node_os, node_arch);
            let url = format!("{}/v{}/{}", base_url, full_ver, filename);
            let inner_dir = format!("node-v{}-{}-{}", full_ver, node_os, node_arch);
            (
                full_ver.to_string(),
                url,
                "tar.gz".to_string(),
                "bin/node".to_string(),
                inner_dir,
            )
        }
    }

    fn find_node_bin(runtime_dir: &Path) -> Option<PathBuf> {
        let (os, _) = Self::detect_os_arch();
        let target = if os == "windows" { "node.exe" } else { "node" };

        let direct = runtime_dir.join(target);
        if direct.is_file() {
            return Some(direct);
        }
        let bin_direct = runtime_dir.join("bin").join(target);
        if bin_direct.is_file() {
            return Some(bin_direct);
        }

        for entry in WalkDir::new(runtime_dir)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.file_name().to_string_lossy() == target {
                return Some(entry.path().to_path_buf());
            }
        }
        None
    }

    fn find_npm_cli(runtime_dir: &Path) -> Option<PathBuf> {
        let npm_cli_direct = runtime_dir
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js");
        if npm_cli_direct.is_file() {
            return Some(npm_cli_direct);
        }

        for entry in WalkDir::new(runtime_dir)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.file_name().to_string_lossy() == "npm-cli.js" {
                return Some(entry.path().to_path_buf());
            }
        }
        None
    }
}

impl RuntimeAdapter for NodeRuntimeAdapter {
    fn get_runtime_type(&self) -> &'static str {
        "node"
    }

    fn ensure_runtime(
        &self,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(PathBuf, String, bool), String> {
        let version = runtime_config
            .get("version")
            .and_then(|v| if v.is_string() { v.as_str() } else { None })
            .unwrap_or("20");

        let (os, arch) = Self::detect_os_arch();
        let (full_ver, url, format, exec_rel, inner_dir_name) =
            Self::get_catalog_info(os, arch, version);
        let runtime_id = format!("node-v{}-{}-{}", full_ver, os, arch);
        let runtime_target_dir = get_runtimes_dir().join(&runtime_id);
        let executable_path = runtime_target_dir.join(&exec_rel);

        if executable_path.exists() {
            return Ok((executable_path, runtime_id, false));
        }
        if let Some(p) = Self::find_node_bin(&runtime_target_dir) {
            return Ok((p, runtime_id, false));
        }

        if let Some(cb) = on_progress {
            cb(&format!("Fetching Node.js v{} ...", full_ver));
        }

        let resp = ureq::get(&url)
            .call()
            .map_err(|e| format!("Failed to download Node.js runtime from {url}: {e}"))?;

        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read runtime stream: {e}"))?;

        let extract_tmp = get_runtimes_dir().join(format!("{}_extract", runtime_id));
        let _ = fs::remove_dir_all(&extract_tmp);
        fs::create_dir_all(&extract_tmp).map_err(|e| e.to_string())?;

        if format == "zip" {
            let cursor = std::io::Cursor::new(bytes);
            let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("Zip error: {e}"))?;
            zip.extract(&extract_tmp)
                .map_err(|e| format!("Zip extract error: {e}"))?;
        } else {
            let cursor = std::io::Cursor::new(bytes);
            let gz = flate2::read::GzDecoder::new(cursor);
            let mut archive = tar::Archive::new(gz);
            archive
                .unpack(&extract_tmp)
                .map_err(|e| format!("Tar unpack error: {e}"))?;
        }

        let inner = extract_tmp.join(&inner_dir_name);
        if inner.exists() {
            let _ = fs::remove_dir_all(&runtime_target_dir);
            if let Some(parent) = runtime_target_dir.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&inner, &runtime_target_dir)
                .or_else(|_| copy_dir_recursive(&inner, &runtime_target_dir))
                .map_err(|e| {
                    format!(
                        "Failed to move runtime to {}: {e}",
                        runtime_target_dir.display()
                    )
                })?;
        } else {
            let _ = fs::remove_dir_all(&runtime_target_dir);
            fs::rename(&extract_tmp, &runtime_target_dir)
                .or_else(|_| copy_dir_recursive(&extract_tmp, &runtime_target_dir))
                .map_err(|e| e.to_string())?;
        }

        let _ = fs::remove_dir_all(&extract_tmp);

        let final_exec = Self::find_node_bin(&runtime_target_dir)
            .unwrap_or_else(|| runtime_target_dir.join(&exec_rel));

        Ok((final_exec, runtime_id, true))
    }

    fn install_dependencies(
        &self,
        project_dir: &Path,
        staging_dir: &Path,
        runtime_config: &serde_json::Value,
        deps_config: Option<&serde_json::Value>,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(bool, String), String> {
        let (runtime_exe, _, _) = self.ensure_runtime(runtime_config, on_progress)?;
        let runtime_dir = runtime_exe.parent().unwrap_or(&runtime_exe);
        let npm_cli = Self::find_npm_cli(runtime_dir);

        let node_modules_src = project_dir.join("node_modules");
        let node_modules_dst = staging_dir.join("node_modules");

        // 1. Explicit package list declared in dependencies: [express, cors]
        if let Some(serde_json::Value::Array(arr)) = deps_config {
            let pkgs: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.ends_with(".json"))
                .map(|s| s.to_string())
                .collect();

            if !pkgs.is_empty() {
                if let Some(ref npm_path) = npm_cli {
                    if let Some(cb) = on_progress {
                        cb(&format!(
                            "Installing {} npm package(s) via embedded npm...",
                            pkgs.len()
                        ));
                    }
                    let mut cmd = Command::new(&runtime_exe);
                    cmd.arg(npm_path)
                        .arg("install")
                        .arg("--prefix")
                        .arg(staging_dir)
                        .arg("--no-audit")
                        .arg("--no-fund")
                        .arg("--omit=dev")
                        .arg("--loglevel=error");
                    for pkg in &pkgs {
                        cmd.arg(pkg);
                    }

                    let res = cmd
                        .output()
                        .map_err(|e| format!("Failed to run npm install: {e}"))?;
                    if !res.status.success() {
                        let err_str = String::from_utf8_lossy(&res.stderr);
                        return Err(format!("npm install failed: {err_str}"));
                    }
                    return Ok((
                        true,
                        format!("Installed {} npm package(s) into node_modules/", pkgs.len()),
                    ));
                }
            }
        }

        // 2. package.json declared or present
        let pkg_json = project_dir.join("package.json");
        let is_pkg_json_spec = deps_config.and_then(|v| v.as_str()) == Some("package.json");

        if is_pkg_json_spec || pkg_json.is_file() {
            if let Some(ref npm_path) = npm_cli {
                if let Some(cb) = on_progress {
                    cb("Resolving dependencies from package.json via embedded npm...");
                }
                if pkg_json.is_file() {
                    let _ = fs::copy(&pkg_json, staging_dir.join("package.json"));
                }
                let pkg_lock = project_dir.join("package-lock.json");
                if pkg_lock.is_file() {
                    let _ = fs::copy(&pkg_lock, staging_dir.join("package-lock.json"));
                }

                let mut cmd = Command::new(&runtime_exe);
                cmd.arg(npm_path)
                    .arg("install")
                    .arg("--prefix")
                    .arg(staging_dir)
                    .arg("--no-audit")
                    .arg("--no-fund")
                    .arg("--omit=dev")
                    .arg("--loglevel=error");

                let res = cmd
                    .output()
                    .map_err(|e| format!("Failed to run npm install: {e}"))?;
                if !res.status.success() {
                    let err_str = String::from_utf8_lossy(&res.stderr);
                    return Err(format!("npm install failed: {err_str}"));
                }
                return Ok((
                    true,
                    "Resolved and installed dependencies from package.json in node_modules/"
                        .to_string(),
                ));
            }
        }

        // 3. Fallback: bundle local node_modules
        if node_modules_src.is_dir() {
            if let Some(cb) = on_progress {
                cb("Bundling local node_modules...");
            }
            copy_dir_recursive(&node_modules_src, &node_modules_dst).map_err(|e| e.to_string())?;
            return Ok((true, "Bundled local node_modules into archive".to_string()));
        }

        Ok((true, "No external node_modules declared".to_string()))
    }

    fn protect_code(
        &self,
        staging_dir: &Path,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(bool, String), String> {
        if let Some(cb) = on_progress {
            cb("Compiling JS/TS sources to binary V8 bytecode (.jsc)...");
        }

        let (runtime_exe, _, _) = self.ensure_runtime(runtime_config, on_progress)?;

        let mut code_files = Vec::new();
        for entry in WalkDir::new(staging_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                if let Ok(rel) = path.strip_prefix(staging_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.starts_with("node_modules/")
                        && (rel_str.ends_with(".js") || rel_str.ends_with(".ts"))
                    {
                        code_files.push(path);
                    }
                }
            }
        }

        if code_files.is_empty() {
            return Ok((true, "No JS/TS source files to protect".to_string()));
        }

        // 1. Write universal _box_node_loader.js helper in staging root
        let loader_path = staging_dir.join("_box_node_loader.js");
        let loader_code = "const fs = require('fs');\n\
const vm = require('vm');\n\
const v8 = require('v8');\n\
v8.setFlagsFromString('--no-lazy');\n\
const Module = require('module');\n\
function runBytecode(jscPath, len, mod, exp, req, filename, dirname) {\n\
  const bytecode = fs.readFileSync(jscPath);\n\
  const dummy = Module.wrap(' '.repeat(len));\n\
  const script = new vm.Script(dummy, { cachedData: bytecode, filename: filename });\n\
  const fn = script.runInThisContext();\n\
  return fn.call(mod.exports, mod.exports, req, mod, filename, dirname);\n\
}\n\
module.exports = { runBytecode };\n";
        fs::write(&loader_path, loader_code).map_err(|e| e.to_string())?;

        // 2. Prepare files metadata
        let mut files_meta = Vec::new();
        for code_file in &code_files {
            let jsc_file = code_file.with_extension("jsc");
            files_meta.push(serde_json::json!({
                "src": code_file.to_string_lossy().replace('\\', "/"),
                "dst": jsc_file.to_string_lossy().replace('\\', "/"),
            }));
        }

        let compiler_path = staging_dir.join("_box_compiler.js");
        let lengths_path = staging_dir.join("_box_lengths.json");

        let files_json = serde_json::to_string(&files_meta).unwrap_or_default();
        let lengths_json_escaped = lengths_path.to_string_lossy().replace('\\', "/");

        let compiler_script = format!(
            "const fs = require('fs');\n\
const vm = require('vm');\n\
const Module = require('module');\n\
const v8 = require('v8');\n\
v8.setFlagsFromString('--no-lazy');\n\
const files = {};\n\
const lengths = {{}};\n\
for (const item of files) {{\n\
  let code = fs.readFileSync(item.src, 'utf8');\n\
  if (item.src.toLowerCase().endsWith('.ts') && Module.stripTypeScriptTypes) {{\n\
    code = Module.stripTypeScriptTypes(code);\n\
  }}\n\
  const wrapped = Module.wrap(code);\n\
  const script = new vm.Script(wrapped, {{ produceCachedData: true }});\n\
  const bytecode = script.createCachedData();\n\
  fs.writeFileSync(item.dst, bytecode);\n\
  lengths[item.src] = code.length;\n\
}}\n\
fs.writeFileSync('{}', JSON.stringify(lengths));\n",
            files_json, lengths_json_escaped
        );

        fs::write(&compiler_path, compiler_script).map_err(|e| e.to_string())?;

        let res = Command::new(&runtime_exe)
            .arg(&compiler_path)
            .output()
            .map_err(|e| format!("Failed to run bytecode compiler: {e}"))?;

        let _ = fs::remove_file(&compiler_path);

        if !res.status.success() {
            let _ = fs::remove_file(&lengths_path);
            let err_msg = String::from_utf8_lossy(&res.stderr);
            return Err(format!("V8 Bytecode compilation failed: {err_msg}"));
        }

        let lengths_raw = fs::read_to_string(&lengths_path).unwrap_or_else(|_| "{}".to_string());
        let _ = fs::remove_file(&lengths_path);
        let lengths: HashMap<String, usize> =
            serde_json::from_str(&lengths_raw).unwrap_or_default();

        // 3. Replace each source file with a 1-line bytecode loader
        for meta in &files_meta {
            let src_str = meta["src"].as_str().unwrap_or("");
            let src_path = PathBuf::from(src_str);
            let code_len = lengths.get(src_str).copied().unwrap_or(0);

            let rel_to_loader = if let Some(rel) =
                pathdiff::diff_paths(&loader_path, src_path.parent().unwrap_or(staging_dir))
            {
                rel.to_string_lossy().replace('\\', "/")
            } else {
                "./_box_node_loader.js".to_string()
            };

            let stub = format!(
                "const _bldr = require('{}');\n_bldr.runBytecode(__filename.replace(/\\.[^/\\\\]+$/, '.jsc'), {}, module, exports, require, __filename, __dirname);\n",
                rel_to_loader, code_len
            );
            fs::write(&src_path, stub).map_err(|e| e.to_string())?;
        }

        Ok((
            true,
            format!(
                "Protected {} JS/TS file(s) (V8 bytecode generated, sources stripped)",
                code_files.len()
            ),
        ))
    }

    fn setup_environment(
        &self,
        env: &mut HashMap<String, String>,
        sandbox_dir: &Path,
        _workdir_rel: &str,
    ) {
        let node_modules = sandbox_dir.join("node_modules");
        if node_modules.exists() {
            let sep = if cfg!(windows) { ";" } else { ":" };
            let existing = env.get("NODE_PATH").cloned().unwrap_or_default();
            let new_path = if existing.is_empty() {
                node_modules.to_string_lossy().to_string()
            } else {
                format!("{}{}{}", node_modules.display(), sep, existing)
            };
            env.insert("NODE_PATH".to_string(), new_path);
        }
    }

    fn build_launch_command(
        &self,
        runtime_exe: &Path,
        entrypoint: &str,
        sandbox_dir: &Path,
    ) -> Vec<String> {
        let mut tokens: Vec<String> = shlex::split(entrypoint).unwrap_or_else(|| {
            entrypoint
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        });

        if !tokens.is_empty() && tokens[0].to_lowercase() == "node" {
            tokens.remove(0);
        }

        if !tokens.is_empty() {
            let candidate_file = sandbox_dir.join(&tokens[0]);
            if candidate_file.exists() {
                tokens[0] = candidate_file.to_string_lossy().to_string();
            }
        }

        let mut cmd = vec![runtime_exe.to_string_lossy().to_string()];
        cmd.extend(tokens);
        cmd
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if let Ok(rel) = path.strip_prefix(src) {
            if rel.as_os_str().is_empty() {
                continue;
            }
            let target = dst.join(rel);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)?;
            } else if entry.file_type().is_file() {
                if let Some(p) = target.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::copy(path, &target)?;
            }
        }
    }
    Ok(())
}
