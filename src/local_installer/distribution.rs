use serde::Serialize;
use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct DistributionIdentity {
    pub id: String,
    pub id_like: Vec<String>,
    pub version_id: Option<String>,
    pub version_codename: Option<String>,
    pub pretty_name: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DistributionResolution {
    pub detected: DistributionIdentity,
    pub distribution: String,
    pub matched_by: String,
    pub candidates: Vec<String>,
}

pub fn detect() -> Result<DistributionIdentity, String> {
    let path = os_release_path();

    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "could not read OS identity from {}: {error}",
            path.display()
        )
    })?;

    parse_os_release(&raw, &path.display().to_string())
}

pub fn resolve(distributions: &Map<String, Value>) -> Result<DistributionResolution, String> {
    let detected = detect()?;
    resolve_identity(detected, distributions)
}

fn resolve_identity(
    detected: DistributionIdentity,
    distributions: &Map<String, Value>,
) -> Result<DistributionResolution, String> {
    let mut candidates = Vec::new();

    push_unique(&mut candidates, &detected.id);

    for item in &detected.id_like {
        push_unique(&mut candidates, item);
    }

    for candidate in &candidates {
        if distributions.contains_key(candidate) {
            let matched_by = if candidate == &detected.id {
                "ID"
            } else {
                "ID_LIKE"
            };

            return Ok(DistributionResolution {
                detected,
                distribution: candidate.clone(),
                matched_by: matched_by.to_string(),
                candidates,
            });
        }
    }

    Err(format!(
        "no compatible installer dictionary distribution found; \
detected ID='{}', ID_LIKE={:?}, candidates={:?}",
        detected.id, detected.id_like, candidates
    ))
}

fn os_release_path() -> PathBuf {
    env::var("NEEBLES_OS_RELEASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/os-release"))
}

fn parse_os_release(raw: &str, source: &str) -> Result<DistributionIdentity, String> {
    let mut id = None;
    let mut id_like = Vec::new();
    let mut version_id = None;
    let mut version_codename = None;
    let mut pretty_name = None;

    for raw_line in raw.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let value = decode_value(raw_value.trim());

        match key.trim() {
            "ID" => id = Some(normalize_id(&value)),
            "ID_LIKE" => {
                id_like = value
                    .split_whitespace()
                    .map(normalize_id)
                    .filter(|value| !value.is_empty())
                    .collect();
            }
            "VERSION_ID" => version_id = Some(value),
            "VERSION_CODENAME" => version_codename = Some(value),
            "PRETTY_NAME" => pretty_name = Some(value),
            _ => {}
        }
    }

    let id = id
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("OS identity source '{}' does not define a valid ID", source))?;

    Ok(DistributionIdentity {
        id,
        id_like,
        version_id,
        version_codename,
        pretty_name,
        source: source.to_string(),
    })
}

fn decode_value(value: &str) -> String {
    let value = value.trim();

    let unquoted = if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };

    unquoted
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\\\", "\\")
}

fn normalize_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    let value = normalize_id(value);

    if value.is_empty() {
        return;
    }

    if !values.iter().any(|item| item == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn distributions(ids: &[&str]) -> Map<String, Value> {
        let mut map = Map::new();

        for id in ids {
            map.insert((*id).to_string(), json!({}));
        }

        map
    }

    fn identity(id: &str, id_like: &[&str]) -> DistributionIdentity {
        DistributionIdentity {
            id: id.to_string(),
            id_like: id_like.iter().map(|value| (*value).to_string()).collect(),
            version_id: None,
            version_codename: None,
            pretty_name: None,
            source: "test".to_string(),
        }
    }

    #[test]
    fn resolves_debian_directly_by_id() {
        let result = resolve_identity(identity("debian", &[]), &distributions(&["debian"]))
            .expect("Debian must resolve directly");

        assert_eq!(result.distribution, "debian");
        assert_eq!(result.matched_by, "ID");
        assert_eq!(result.candidates, vec!["debian"]);
    }

    #[test]
    fn resolves_kubuntu_to_debian_through_id_like() {
        let result = resolve_identity(
            identity("kubuntu", &["ubuntu", "debian"]),
            &distributions(&["debian"]),
        )
        .expect("Kubuntu must inherit Debian compatibility through ID_LIKE");

        assert_eq!(result.distribution, "debian");
        assert_eq!(result.matched_by, "ID_LIKE");
        assert_eq!(result.candidates, vec!["kubuntu", "ubuntu", "debian"]);
    }

    #[test]
    fn resolves_ubuntu_to_debian_through_id_like() {
        let result = resolve_identity(identity("ubuntu", &["debian"]), &distributions(&["debian"]))
            .expect("Ubuntu must inherit Debian compatibility through ID_LIKE");

        assert_eq!(result.distribution, "debian");
        assert_eq!(result.matched_by, "ID_LIKE");
    }

    #[test]
    fn resolves_neebles_to_debian_through_id_like() {
        let result = resolve_identity(
            identity("neebles", &["debian"]),
            &distributions(&["debian"]),
        )
        .expect("NEEBLES must inherit Debian compatibility through ID_LIKE");

        assert_eq!(result.distribution, "debian");
        assert_eq!(result.matched_by, "ID_LIKE");
    }

    #[test]
    fn prefers_exact_id_over_id_like() {
        let result = resolve_identity(
            identity("ubuntu", &["debian"]),
            &distributions(&["ubuntu", "debian"]),
        )
        .expect("Exact distribution must win over family compatibility");

        assert_eq!(result.distribution, "ubuntu");
        assert_eq!(result.matched_by, "ID");
    }

    #[test]
    fn rejects_unknown_distribution_family() {
        let error = resolve_identity(
            identity("unknown", &["foo", "bar"]),
            &distributions(&["debian"]),
        )
        .expect_err("Unknown distribution family must fail explicitly");

        assert!(error.contains("no compatible installer dictionary distribution found"));
        assert!(error.contains("unknown"));
    }

    #[test]
    fn normalizes_os_release_ids() {
        let parsed = parse_os_release(
            r#"
ID=KUBUNTU
ID_LIKE="Ubuntu Debian"
VERSION_ID="26.04"
PRETTY_NAME="Kubuntu Test"
"#,
            "test-os-release",
        )
        .expect("test os-release must parse");

        assert_eq!(parsed.id, "kubuntu");
        assert_eq!(parsed.id_like, vec!["ubuntu", "debian"]);
        assert_eq!(parsed.version_id.as_deref(), Some("26.04"));
    }
}
