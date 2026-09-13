use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalInstallerRequest {
    pub installer: String,
    pub operation: String,

    #[serde(default)]
    pub variables: HashMap<String, String>,
}
