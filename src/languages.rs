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

pub fn client_root() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("NEEBLES_CLIENT_ROOT") {
        let value = value.trim();

        if value.is_empty() {
            return Err("NEEBLES_CLIENT_ROOT is explicitly set but empty".to_string());
        }

        return Ok(PathBuf::from(value));
    }

    if Path::new("client/languages/manifest.json").exists() {
        return Ok(PathBuf::from("client"));
    }

    Ok(PathBuf::from("/opt/neebles/client"))
}

pub fn load_manifest() -> Result<LanguageManifest, String> {
    let path = client_root()?.join("languages/manifest.json");
    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "could not read language manifest {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid language manifest {}: {error}", path.display()))
}

pub fn normalize_locale(value: &str) -> String {
    let trimmed = value.trim();
    let without_encoding = trimmed.split('.').next().unwrap_or(trimmed);
    let without_modifier = without_encoding
        .split('@')
        .next()
        .unwrap_or(without_encoding);
    let normalized = without_modifier.replace('-', "_");
    let parts = normalized
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    match parts.as_slice() {
        [] => String::new(),
        [language] => language.to_ascii_lowercase(),
        [language, region, ..] => format!(
            "{}_{}",
            language.to_ascii_lowercase(),
            region.to_ascii_uppercase()
        ),
    }
}

fn canonical_language(manifest: &LanguageManifest, requested: &str) -> Option<String> {
    let requested = normalize_locale(requested);

    if requested.is_empty() {
        return None;
    }

    manifest
        .languages
        .iter()
        .find(|language| normalize_locale(&language.code) == requested)
        .map(|language| language.code.clone())
}

pub fn default_language(manifest: &LanguageManifest) -> Result<String, String> {
    let default = manifest.default.trim();

    if default.is_empty() {
        return Err("Boss language manifest must declare a default language".to_string());
    }

    canonical_language(manifest, default).ok_or_else(|| {
        format!(
            "Boss language manifest declares default '{}' but that language is not listed",
            manifest.default
        )
    })
}

pub fn resolve_language(code: &str) -> Result<Option<String>, String> {
    let manifest = load_manifest()?;
    Ok(canonical_language(&manifest, code))
}

pub fn detect_initial_language() -> Result<String, String> {
    let manifest = load_manifest()?;

    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key) {
            if value.trim().is_empty() {
                continue;
            }

            if let Some(language) = canonical_language(&manifest, &value) {
                return Ok(language);
            }
        }
    }

    default_language(&manifest)
}

pub fn load_strings(code: &str) -> Result<BTreeMap<String, String>, String> {
    let manifest = load_manifest()?;
    let resolved = canonical_language(&manifest, code)
        .map(Ok)
        .unwrap_or_else(|| default_language(&manifest))?;

    let language = manifest
        .languages
        .iter()
        .find(|language| language.code == resolved)
        .ok_or_else(|| format!("unsupported N.E.E.B.L.E.S. language: {resolved}"))?;

    let path = client_root()?.join("languages").join(&language.file);
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("could not read language file {}: {error}", path.display()))?;

    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid language file {}: {error}", path.display()))
}
