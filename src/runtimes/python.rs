use crate::config::{get_box_home, get_runtimes_dir, get_setting};
use crate::runtimes::RuntimeAdapter;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub struct PythonRuntimeAdapter;

impl Default for PythonRuntimeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonRuntimeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn detect_os_arch() -> (&'static str, &'static str) {
        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
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

    fn get_download_url(
        version: &str,
        os: &str,
        arch: &str,
    ) -> Result<(String, String, String, String), String> {
        let custom_mirror = get_setting("python_mirror");

        if os == "windows" && arch == "x86_64" {
            let (full_ver, file_name) = match version {
                "3.13" => ("3.13.2", "python-3.13.2-embed-amd64.zip"),
                "3.12" => ("3.12.5", "python-3.12.5-embed-amd64.zip"),
                "3.11" => ("3.11.9", "python-3.11.9-embed-amd64.zip"),
                "3.10" => ("3.10.11", "python-3.10.11-embed-amd64.zip"),
                _ => ("3.12.5", "python-3.12.5-embed-amd64.zip"),
            };

            let base_url =
                custom_mirror.unwrap_or_else(|| "https://www.python.org/ftp/python".to_string());
            let url = format!(
                "{}/{}/{}",
                base_url.trim_end_matches('/'),
                full_ver,
                file_name
            );
            return Ok((
                full_ver.to_string(),
                url,
                "zip".to_string(),
                "python.exe".to_string(),
            ));
        }

        // Linux and macOS standalone builds
        let triple = match (os, arch) {
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            _ => "x86_64-unknown-linux-gnu",
        };

        let (full_ver, rel_tag) = match version {
            "3.13" => ("3.13.2", "20250212"),
            "3.12" => ("3.12.5", "20240814"),
            "3.11" => ("3.11.9", "20240814"),
            "3.10" => ("3.10.14", "20240814"),
            _ => ("3.12.5", "20240814"),
        };

        let base_mirror = custom_mirror.unwrap_or_else(|| {
            "https://github.com/astral-sh/python-build-standalone/releases/download".to_string()
        });

        let url = format!(
            "{}/{}/cpython-{}+{}-{}-install_only.tar.gz",
            base_mirror.trim_end_matches('/'),
            rel_tag,
            full_ver,
            rel_tag,
            triple
        );
        Ok((
            full_ver.to_string(),
            url,
            "tar.gz".to_string(),
            "bin/python3".to_string(),
        ))
    }

    fn ensure_pip(on_progress: Option<&dyn Fn(&str)>) -> std::io::Result<PathBuf> {
        let pip_path = get_box_home().join("pip.pyz");
        if pip_path.exists() {
            return Ok(pip_path);
        }

        if let Some(cb) = on_progress {
            cb("Downloading standalone pip.pyz installer...");
        }

        let pip_url = "https://bootstrap.pypa.io/pip/pip.pyz";
        let resp = ureq::get(pip_url).call().map_err(std::io::Error::other)?;

        let mut body = Vec::new();
        resp.into_reader().read_to_end(&mut body)?;

        let tmp = pip_path.with_extension("tmp");
        fs::write(&tmp, body)?;
        fs::rename(tmp, &pip_path)?;
        Ok(pip_path)
    }

    fn find_python_bin(runtime_dir: &Path) -> Option<PathBuf> {
        let (os, _) = Self::detect_os_arch();
        let target = if os == "windows" {
            "python.exe"
        } else {
            "python3"
        };

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
            if entry.file_type().is_file()
                && (entry.file_name().to_string_lossy() == target
                    || entry.file_name().to_string_lossy() == "python")
            {
                return Some(entry.path().to_path_buf());
            }
        }
        None
    }
}

impl RuntimeAdapter for PythonRuntimeAdapter {
    fn get_runtime_type(&self) -> &'static str {
        "python"
    }

    fn ensure_runtime(
        &self,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(PathBuf, String, bool), String> {
        let version = runtime_config
            .get("version")
            .and_then(|v| if v.is_string() { v.as_str() } else { None })
            .unwrap_or("3.12");

        let (os, arch) = Self::detect_os_arch();
        let (full_ver, url, format, exec_rel) = Self::get_download_url(version, os, arch)?;
        let runtime_id = format!("python-{}-{}-{}", full_ver, os, arch);
        let runtime_target_dir = get_runtimes_dir().join(&runtime_id);
        let executable_path = runtime_target_dir.join(&exec_rel);

        if executable_path.exists() {
            for entry in fs::read_dir(&runtime_target_dir)
                .map_err(|e| e.to_string())?
                .flatten()
            {
                if entry.file_name().to_string_lossy().ends_with("._pth") {
                    let _ = fs::remove_file(entry.path());
                }
            }
            return Ok((executable_path, runtime_id, false));
        }

        if let Some(p) = Self::find_python_bin(&runtime_target_dir) {
            for entry in fs::read_dir(&runtime_target_dir)
                .map_err(|e| e.to_string())?
                .flatten()
            {
                if entry.file_name().to_string_lossy().ends_with("._pth") {
                    let _ = fs::remove_file(entry.path());
                }
            }
            return Ok((p, runtime_id, false));
        }

        if let Some(cb) = on_progress {
            cb(&format!("Fetching Python {} ...", full_ver));
        }

        let resp = ureq::get(&url)
            .call()
            .map_err(|e| format!("Failed to download Python runtime from {url}: {e}"))?;

        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read runtime download stream: {e}"))?;

        fs::create_dir_all(&runtime_target_dir).map_err(|e| e.to_string())?;

        if format == "zip" {
            let cursor = std::io::Cursor::new(bytes);
            let mut zip =
                zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to unpack zip: {e}"))?;
            zip.extract(&runtime_target_dir)
                .map_err(|e| format!("Zip extract failed: {e}"))?;

            for entry in fs::read_dir(&runtime_target_dir)
                .map_err(|e| e.to_string())?
                .flatten()
            {
                if entry.file_name().to_string_lossy().ends_with("._pth") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        } else {
            let cursor = std::io::Cursor::new(bytes);
            let gz = flate2::read::GzDecoder::new(cursor);
            let mut archive = tar::Archive::new(gz);
            archive
                .unpack(&runtime_target_dir)
                .map_err(|e| format!("Tar unpack failed: {e}"))?;
        }

        let final_exec = Self::find_python_bin(&runtime_target_dir)
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
        let pip_pyz = Self::ensure_pip(on_progress).map_err(|e| e.to_string())?;

        let lib_dir = staging_dir.join("_box_lib");
        fs::create_dir_all(&lib_dir).map_err(|e| e.to_string())?;

        let mut pip_args: Vec<String> = Vec::new();
        let summary: String;

        // Check dependencies spec
        if let Some(serde_json::Value::String(dep_str)) = deps_config {
            let target_file = project_dir.join(dep_str);
            if target_file.is_file() || dep_str.ends_with(".txt") {
                pip_args.push("-r".to_string());
                pip_args.push(target_file.to_string_lossy().to_string());
                summary = format!("requirements from {}", dep_str);
            } else {
                pip_args.push(dep_str.clone());
                summary = format!("1 package ({})", dep_str);
            }
        } else if let Some(serde_json::Value::Array(arr)) = deps_config {
            let mut req_files = 0;
            let mut pkgs = 0;
            for item in arr {
                if let Some(s) = item.as_str() {
                    let target_file = project_dir.join(s);
                    if target_file.is_file() || s.ends_with(".txt") {
                        pip_args.push("-r".to_string());
                        pip_args.push(target_file.to_string_lossy().to_string());
                        req_files += 1;
                    } else {
                        pip_args.push(s.to_string());
                        pkgs += 1;
                    }
                }
            }
            if req_files > 0 && pkgs > 0 {
                summary = format!("{req_files} file(s) and {pkgs} package(s)");
            } else if req_files > 0 {
                summary = format!("{req_files} requirements file(s)");
            } else {
                summary = format!("{pkgs} package(s)");
            }
        } else {
            let auto_req = project_dir.join("requirements.txt");
            if auto_req.is_file() {
                pip_args.push("-r".to_string());
                pip_args.push(auto_req.to_string_lossy().to_string());
                summary = "auto-detected requirements.txt".to_string();
            } else {
                return Ok((true, "No external dependencies declared".to_string()));
            }
        }

        let mut cmd = Command::new(&runtime_exe);
        cmd.arg(&pip_pyz)
            .arg("install")
            .arg("--target")
            .arg(&lib_dir)
            .arg("--upgrade")
            .arg("--disable-pip-version-check")
            .arg("--quiet");

        for arg in &pip_args {
            cmd.arg(arg);
        }

        let res = cmd
            .output()
            .map_err(|e| format!("Failed to run pip: {e}"))?;
        if !res.status.success() {
            let err = String::from_utf8_lossy(&res.stderr);
            return Err(format!("Pip dependency resolution failed: {err}"));
        }

        Ok((
            true,
            format!("Resolved and isolated {} in _box_lib/", summary),
        ))
    }

    fn protect_code(
        &self,
        staging_dir: &Path,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(bool, String), String> {
        if let Some(cb) = on_progress {
            cb("Compiling Python sources to bytecode (.pyc) using target runtime...");
        }

        let (runtime_exe, _, _) = self.ensure_runtime(runtime_config, on_progress)?;

        let mut py_files = Vec::new();
        for entry in WalkDir::new(staging_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                if let Ok(rel) = path.strip_prefix(staging_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.starts_with("_box_lib/") && rel_str.ends_with(".py") {
                        py_files.push(path);
                    }
                }
            }
        }

        if py_files.is_empty() {
            return Ok((true, "No Python source files to protect".to_string()));
        }

        let res = Command::new(&runtime_exe)
            .arg("-m")
            .arg("compileall")
            .arg("-b")
            .arg("-q")
            .arg(staging_dir)
            .output()
            .map_err(|e| format!("Failed to run compileall: {e}"))?;

        if !res.status.success() {
            let err = String::from_utf8_lossy(&res.stderr);
            return Err(format!("Bytecode compilation failed: {err}"));
        }

        let mut stripped_count = 0;
        for py_file in &py_files {
            let pyc_file = py_file.with_extension("pyc");
            if pyc_file.exists() {
                let _ = fs::remove_file(py_file);
                stripped_count += 1;
            }
        }

        // Clean __pycache__ folders
        for entry in WalkDir::new(staging_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() && entry.file_name() == "__pycache__" {
                let _ = fs::remove_dir_all(entry.path());
            }
        }

        Ok((
            true,
            format!(
                "Protected {} Python file(s) (compiled to bytecode, sources stripped)",
                stripped_count
            ),
        ))
    }

    fn setup_environment(
        &self,
        env: &mut HashMap<String, String>,
        sandbox_dir: &Path,
        _workdir_rel: &str,
    ) {
        env.insert("PYTHONIOENCODING".to_string(), "utf-8".to_string());
        env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
        env.insert("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string());

        let box_lib = sandbox_dir.join("_box_lib");
        let lib_dir = sandbox_dir.join("lib");
        let sep = if cfg!(windows) { ";" } else { ":" };

        let mut paths = vec![sandbox_dir.to_string_lossy().to_string()];
        if box_lib.exists() {
            paths.push(box_lib.to_string_lossy().to_string());
        }
        if lib_dir.exists() {
            paths.push(lib_dir.to_string_lossy().to_string());
        }
        if let Some(existing) = env.get("PYTHONPATH") {
            if !existing.is_empty() {
                paths.push(existing.clone());
            }
        }

        env.insert("PYTHONPATH".to_string(), paths.join(sep));
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

        if !tokens.is_empty() && tokens[0].to_lowercase() == "python" {
            tokens.remove(0);
        }

        if !tokens.is_empty() {
            let target_token = tokens[0].clone();
            let candidate_file = sandbox_dir.join(&target_token);

            if candidate_file.exists() {
                tokens[0] = candidate_file.to_string_lossy().to_string();
            } else if target_token.ends_with(".py") {
                let pyc_candidate = sandbox_dir.join(format!("{}c", target_token));
                if pyc_candidate.exists() {
                    tokens[0] = pyc_candidate.to_string_lossy().to_string();
                }
            }
        }

        let mut cmd = vec![runtime_exe.to_string_lossy().to_string()];
        cmd.extend(tokens);
        cmd
    }
}
