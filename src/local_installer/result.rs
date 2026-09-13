use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalInstallerResult {
    pub installer: String,
    pub operation: String,
    pub command: String,
    pub args: Vec<String>,
    pub requires_root: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}
