use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageEntry {
    pub code: String,
    pub name: String,
    pub flag: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageManifest {
    pub schema: u32,
    pub default: String,
    pub languages: Vec<LanguageEntry>,
}

pub fn client_root() -> PathBuf {
    if let Ok(value) = env::var("NEEBLES_CLIENT_ROOT") {
        return PathBuf::from(value);
    }

    if Path::new("client/languages/manifest.json").exists() {
        return PathBuf::from("client");
    }

    PathBuf::from("/opt/neebles/client")
}

pub fn load_manifest() -> Result<LanguageManifest, String> {
    let path = client_root().join("languages/manifest.json");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("could not read language manifest {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid language manifest {}: {error}", path.display()))
}

pub fn is_supported(code: &str) -> bool {
    load_manifest()
        .map(|manifest| manifest.languages.iter().any(|language| language.code == code))
        .unwrap_or(false)
}

pub fn normalize_locale(value: &str) -> String {
    let trimmed = value.trim();
    let without_encoding = trimmed.split('.').next().unwrap_or(trimmed);
    let without_modifier = without_encoding.split('@').next().unwrap_or(without_encoding);
    without_modifier.replace('-', "_")
}

pub fn detect_initial_language() -> String {
    let supported = load_manifest()
        .map(|manifest| manifest.languages.into_iter().map(|item| item.code).collect::<Vec<_>>())
        .unwrap_or_else(|_| vec!["es_CL".to_string(), "en_US".to_string()]);

    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key) {
            if value.trim().is_empty() {
                continue;
            }
            let normalized = normalize_locale(&value);
            if supported.iter().any(|code| code == &normalized) {
                return normalized;
            }
        }
    }

    "en_US".to_string()
}

pub fn load_strings(code: &str) -> Result<BTreeMap<String, String>, String> {
    let manifest = load_manifest()?;
    let language = manifest
        .languages
        .iter()
        .find(|language| language.code == code)
        .ok_or_else(|| format!("unsupported N.E.E.B.L.E.S. language: {code}"))?;

    let path = client_root().join("languages").join(&language.file);
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("could not read language file {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid language file {}: {error}", path.display()))
}
