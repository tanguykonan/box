pub mod node;
pub mod python;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub trait RuntimeAdapter: Send + Sync {
    #[allow(dead_code)]
    fn get_runtime_type(&self) -> &'static str;

    fn ensure_runtime(
        &self,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(PathBuf, String, bool), String>;

    fn install_dependencies(
        &self,
        project_dir: &Path,
        staging_dir: &Path,
        runtime_config: &serde_json::Value,
        deps_config: Option<&serde_json::Value>,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(bool, String), String>;

    fn protect_code(
        &self,
        staging_dir: &Path,
        runtime_config: &serde_json::Value,
        on_progress: Option<&dyn Fn(&str)>,
    ) -> Result<(bool, String), String>;

    fn setup_environment(
        &self,
        env: &mut HashMap<String, String>,
        sandbox_dir: &Path,
        workdir_rel: &str,
    );

    fn build_launch_command(
        &self,
        runtime_exe: &Path,
        entrypoint: &str,
        sandbox_dir: &Path,
    ) -> Vec<String>;
}

pub fn get_runtime_adapter(runtime_type: &str) -> Result<Box<dyn RuntimeAdapter>, String> {
    match runtime_type.to_lowercase().as_str() {
        "python" | "py" => Ok(Box::new(python::PythonRuntimeAdapter::new())),
        "node" | "nodejs" | "typescript" | "ts" => Ok(Box::new(node::NodeRuntimeAdapter::new())),
        other => Err(format!(
            "Unsupported runtime '{}'. Supported: python, node, typescript",
            other
        )),
    }
}
