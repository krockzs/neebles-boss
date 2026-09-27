use serde_json::Value;

use std::collections::{BTreeMap, BTreeSet};

use std::fs;

use std::path::{Path, PathBuf};

pub const AUTHORITY_SUPPLY_SCHEMA: &str = "1";
pub const AUTHORITY_SUPPLY_NAME: &str = "neebles-authority-supply";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritySupplyEntry {
    pub authority: String,
    pub descriptor_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritySupply {
    source_path: PathBuf,
    entries: Vec<AuthoritySupplyEntry>,
}

impl AuthoritySupply {
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn entries(&self) -> &[AuthoritySupplyEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppliedAuthorityRegistry {
    entries: BTreeMap<String, PathBuf>,
}

impl SuppliedAuthorityRegistry {
    pub fn contains(&self, authority: &str) -> bool {
        self.entries.contains_key(authority)
    }

    pub fn descriptor_path(&self, authority: &str) -> Result<&Path, String> {
        self.entries
            .get(authority)
            .map(PathBuf::as_path)
            .ok_or_else(|| format!("authority was not supplied and registered: {authority}"))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityGrantSet {
    authorities: BTreeSet<String>,
}

impl AuthorityGrantSet {
    pub fn contains(&self, authority: &str) -> bool {
        self.authorities.contains(authority)
    }

    pub fn is_empty(&self) -> bool {
        self.authorities.is_empty()
    }

    pub fn len(&self) -> usize {
        self.authorities.len()
    }

    pub fn descriptor_path(
        &self,
        registry: &SuppliedAuthorityRegistry,
        authority: &str,
    ) -> Result<PathBuf, String> {
        if !self.contains(authority) {
            return Err(format!("authority was not granted: {authority}"));
        }

        Ok(registry.descriptor_path(authority)?.to_path_buf())
    }
}

fn require_absolute_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }

    Ok(())
}

pub fn load_authority_supply(supply_path: &Path) -> Result<AuthoritySupply, String> {
    require_absolute_path(supply_path, "authority supply path")?;

    let raw = fs::read_to_string(supply_path).map_err(|error| {
        format!(
            "could not read authority supply {}: {error}",
            supply_path.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid authority supply JSON {}: {error}",
            supply_path.display()
        )
    })?;

    if value.get("schema").and_then(Value::as_str) != Some(AUTHORITY_SUPPLY_SCHEMA) {
        return Err("authority supply schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some(AUTHORITY_SUPPLY_NAME) {
        return Err("invalid authority supply name".to_string());
    }

    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "authority supply entries must be an array".to_string())?;

    let mut parsed = Vec::<AuthoritySupplyEntry>::new();

    let mut identities = BTreeSet::<String>::new();

    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| "authority supply entry must be an object".to_string())?;

        let authority = object
            .get("authority")
            .and_then(Value::as_str)
            .ok_or_else(|| "authority supply entry authority must be String".to_string())?
            .trim();

        if authority.is_empty() {
            return Err("authority supply entry authority cannot be empty".to_string());
        }

        if !identities.insert(authority.to_string()) {
            return Err(format!("duplicate authority supply identity: {authority}"));
        }

        let location = object
            .get("location")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("authority supply entry {authority} location must be String"))?
            .trim();

        if location.is_empty() {
            return Err(format!(
                "authority supply entry {authority} location cannot be empty"
            ));
        }

        let descriptor_path = PathBuf::from(location);

        require_absolute_path(
            &descriptor_path,
            &format!("authority supply entry {authority} descriptor location"),
        )?;

        parsed.push(AuthoritySupplyEntry {
            authority: authority.to_string(),
            descriptor_path,
        });
    }

    Ok(AuthoritySupply {
        source_path: supply_path.to_path_buf(),
        entries: parsed,
    })
}

pub fn register_supplied_authorities(
    supply: &AuthoritySupply,
) -> Result<SuppliedAuthorityRegistry, String> {
    let mut registered = BTreeMap::<String, PathBuf>::new();

    for entry in supply.entries() {
        if !entry.descriptor_path.is_file() {
            return Err(format!(
                "supplied authority descriptor is not a file: authority={} path={}",
                entry.authority,
                entry.descriptor_path.display()
            ));
        }

        let raw = fs::read_to_string(&entry.descriptor_path).map_err(|error| {
            format!(
                "could not read supplied authority descriptor {}: {error}",
                entry.descriptor_path.display()
            )
        })?;

        let value: Value = serde_json::from_str(&raw).map_err(|error| {
            format!(
                "invalid supplied authority descriptor JSON {}: {error}",
                entry.descriptor_path.display()
            )
        })?;

        let declared = value
            .get("authority")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "supplied authority descriptor {} must declare authority String",
                    entry.descriptor_path.display()
                )
            })?
            .trim();

        if declared != entry.authority {
            return Err(format!(
                "supplied authority identity mismatch: supply={} descriptor={declared}",
                entry.authority
            ));
        }

        registered.insert(entry.authority.clone(), entry.descriptor_path.clone());
    }

    Ok(SuppliedAuthorityRegistry {
        entries: registered,
    })
}

pub fn build_authority_grant_set<I, S>(
    registry: &SuppliedAuthorityRegistry,
    requested: I,
) -> Result<AuthorityGrantSet, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut granted = BTreeSet::<String>::new();

    for requested_authority in requested {
        let authority = requested_authority.as_ref().trim();

        if authority.is_empty() {
            return Err("granted authority identity cannot be empty".to_string());
        }

        if !registry.contains(authority) {
            return Err(format!(
                "cannot grant authority that was not supplied and registered: {authority}"
            ));
        }

        granted.insert(authority.to_string());
    }

    Ok(AuthorityGrantSet {
        authorities: granted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domestic_external_data_authority::load_external_data_authority_descriptor;

    use crate::domestic_platform_authority::load_platform_authority_descriptor;

    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-authority-supply-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn write_json(path: &Path, value: &Value) {
        fs::write(
            path,
            serde_json::to_vec_pretty(value).expect("fixture JSON must serialize"),
        )
        .expect("fixture JSON must exist");
    }

    fn write_supply(path: &Path, entries: Value) {
        write_json(
            path,
            &serde_json::json!({
                "schema": "1",
                "name":
                    "neebles-authority-supply",
                "entries":
                    entries,
            }),
        );
    }

    #[test]
    fn supply_registers_only_explicit_references_without_discovery() {
        let base = fixture_root("explicit");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let platform = base.join("platform.json");

        let external = base.join("external.json");

        let hidden = base.join("hidden.json");

        write_json(
            &platform,
            &serde_json::json!({
                "authority":
                    "platform.filesystem_boundary"
            }),
        );

        write_json(
            &external,
            &serde_json::json!({
                "authority":
                    "system.dns_resolver_config"
            }),
        );

        write_json(
            &hidden,
            &serde_json::json!({
                "authority":
                    "system.hidden_fixture"
            }),
        );

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "platform.filesystem_boundary",
                    "location":
                        platform
                },
                {
                    "authority":
                        "system.dns_resolver_config",
                    "location":
                        external
                }
            ]),
        );

        let supply = load_authority_supply(&supply_path).expect("explicit supply must load");

        assert_eq!(supply.len(), 2);

        let registry = register_supplied_authorities(&supply)
            .expect("explicit supplied references must register");

        assert_eq!(registry.len(), 2);

        assert!(registry.contains("platform.filesystem_boundary"));

        assert!(registry.contains("system.dns_resolver_config"));

        assert!(!registry.contains("system.hidden_fixture"));

        assert!(hidden.exists());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn supplied_reference_can_fail_registration() {
        let base = fixture_root("missing");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let missing = base.join("missing.json");

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "system.missing_fixture",
                    "location":
                        missing
                }
            ]),
        );

        let supply = load_authority_supply(&supply_path).expect("supply itself must load");

        assert_eq!(supply.len(), 1);

        assert!(register_supplied_authorities(&supply,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn registration_checks_identity_without_stealing_family_semantics() {
        let base = fixture_root("semantics");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let platform = base.join("platform.json");

        let external = base.join("external.json");

        write_json(
            &platform,
            &serde_json::json!({
                "schema": "1",
                "name":
                    "neebles-platform-authority",
                "authority":
                    "platform.filesystem_boundary",
                "protocol":
                    "neebles-filesystem-boundary-v1",
                "provider":
                    base.join(
                        "provider-that-does-not-exist"
                    )
            }),
        );

        write_json(
            &external,
            &serde_json::json!({
                "schema": "1",
                "name":
                    "neebles-external-data-authority",
                "authority":
                    "system.dns_resolver_config",
                "source":
                    "/etc/resolv.conf",
                "destination":
                    "/etc/resolv.conf",
                "access":
                    "read_write"
            }),
        );

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "platform.filesystem_boundary",
                    "location":
                        platform
                },
                {
                    "authority":
                        "system.dns_resolver_config",
                    "location":
                        external
                }
            ]),
        );

        let supply = load_authority_supply(&supply_path).expect("supply must load");

        let registry = register_supplied_authorities(&supply)
            .expect("identity-valid descriptors must register before family resolution");

        let grants = build_authority_grant_set(
            &registry,
            ["platform.filesystem_boundary", "system.dns_resolver_config"],
        )
        .expect("registered identities may be explicitly granted");

        let platform_path = grants
            .descriptor_path(&registry, "platform.filesystem_boundary")
            .expect("platform grant must expose registered descriptor reference");

        let external_path = grants
            .descriptor_path(&registry, "system.dns_resolver_config")
            .expect("external grant must expose registered descriptor reference");

        assert!(
            load_platform_authority_descriptor(&platform_path, "platform.filesystem_boundary",)
                .is_err()
        );

        assert!(load_external_data_authority_descriptor(
            &external_path,
            "system.dns_resolver_config",
        )
        .is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn grant_set_is_exact_and_deduplicated() {
        let base = fixture_root("grants");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let first = base.join("first.json");

        let second = base.join("second.json");

        write_json(
            &first,
            &serde_json::json!({
                "authority":
                    "system.first"
            }),
        );

        write_json(
            &second,
            &serde_json::json!({
                "authority":
                    "system.second"
            }),
        );

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "system.first",
                    "location":
                        first
                },
                {
                    "authority":
                        "system.second",
                    "location":
                        second
                }
            ]),
        );

        let supply = load_authority_supply(&supply_path).expect("supply must load");

        let registry = register_supplied_authorities(&supply).expect("registry must build");

        let grants = build_authority_grant_set(&registry, ["system.first", "system.first"])
            .expect("duplicate requested grant must remain one authority");

        assert_eq!(grants.len(), 1);

        assert!(grants.contains("system.first"));

        assert!(!grants.contains("system.second"));

        assert!(build_authority_grant_set(&registry, ["system.unknown"],).is_err());

        assert!(grants.descriptor_path(&registry, "system.second",).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn supply_rejects_duplicate_authority_identity() {
        let base = fixture_root("duplicate");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let descriptor = base.join("descriptor.json");

        write_json(
            &descriptor,
            &serde_json::json!({
                "authority":
                    "system.fixture"
            }),
        );

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "system.fixture",
                    "location":
                        descriptor
                },
                {
                    "authority":
                        "system.fixture",
                    "location":
                        descriptor
                }
            ]),
        );

        assert!(load_authority_supply(&supply_path,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn supply_requires_absolute_descriptor_references() {
        let base = fixture_root("relative");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "system.fixture",
                    "location":
                        "relative-descriptor.json"
                }
            ]),
        );

        assert!(load_authority_supply(&supply_path,).is_err());

        assert!(load_authority_supply(Path::new("relative-supply.json"),).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn registration_rejects_descriptor_identity_rewrite() {
        let base = fixture_root("identity");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let descriptor = base.join("descriptor.json");

        write_json(
            &descriptor,
            &serde_json::json!({
                "authority":
                    "system.actual"
            }),
        );

        let supply_path = base.join("supply.json");

        write_supply(
            &supply_path,
            serde_json::json!([
                {
                    "authority":
                        "system.claimed",
                    "location":
                        descriptor
                }
            ]),
        );

        let supply =
            load_authority_supply(&supply_path).expect("supply transport itself must load");

        assert!(register_supplied_authorities(&supply,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}
