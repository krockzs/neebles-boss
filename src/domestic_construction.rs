use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domestic_authority_supply::{build_authority_grant_set, SuppliedAuthorityRegistry};
use crate::domestic_platform_control::authenticate_platform_controlled_file;
use crate::domestic_workspace_execution::{
    build_workspace_execution_command, execute_workspace_execution_request,
    WorkspaceExecutionRequest, WorkspaceReadonlyGrant, WorkspaceWritableGrant,
};

pub const DOMESTIC_CONSTRUCTION_SCHEMA: &str = "1";
pub const DOMESTIC_CONSTRUCTION_NAME: &str = "neebles-domestic-construction";

pub const DOMESTIC_CONSTRUCTION_ROOT: &str = "/usr/lib/neebles/domestic/construction";

pub fn validate_domestic_construction_subject(subject: &str) -> Result<(), String> {
    if subject.is_empty() {
        return Err("domestic construction subject cannot be empty".to_string());
    }

    if subject.trim() != subject {
        return Err(
            "domestic construction subject cannot contain surrounding whitespace".to_string(),
        );
    }

    if subject == "." || subject == ".." {
        return Err(format!(
            "domestic construction subject is not a valid canonical identity: {subject}"
        ));
    }

    if !subject.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '.'
            || character == '-'
            || character == '_'
    }) {
        return Err(format!(
            "domestic construction subject contains unsafe filename characters: {subject}"
        ));
    }

    Ok(())
}

pub fn canonical_domestic_construction_declaration_path(subject: &str) -> Result<PathBuf, String> {
    validate_domestic_construction_subject(subject)?;

    Ok(Path::new(DOMESTIC_CONSTRUCTION_ROOT).join(format!("{subject}.json")))
}

pub fn load_canonical_domestic_construction_declaration(
    subject: &str,
) -> Result<DomesticConstructionDeclaration, String> {
    let path = canonical_domestic_construction_declaration_path(subject)?;

    let controlled = authenticate_platform_controlled_file(&path)
        .map_err(|error| {
            format!(
                "domestic construction declaration is not platform-controlled: subject={subject} path={} error={error}",
                path.display()
            )
        })?;

    let raw = fs::read_to_string(controlled.as_path())
        .map_err(|error| {
            format!(
                "could not read domestic construction declaration: subject={subject} path={} error={error}",
                controlled.as_path().display()
            )
        })?;

    let declaration = DomesticConstructionDeclaration::parse(&raw)?;

    if declaration.subject != subject {
        return Err(format!(
            "domestic construction declaration subject mismatch: requested={subject} declared={}",
            declaration.subject
        ));
    }

    Ok(declaration)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomesticConstructionDeclaration {
    pub schema: String,
    pub name: String,
    pub subject: String,
    pub steps: Vec<DomesticConstructionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomesticConstructionStep {
    pub id: String,
    pub world: String,

    #[serde(default)]
    pub readonly: Vec<String>,

    #[serde(default)]
    pub writable: Vec<DomesticConstructionWritable>,

    #[serde(default)]
    pub mounts: DomesticConstructionMounts,

    #[serde(default)]
    pub chdir: Option<PathBuf>,

    #[serde(default)]
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomesticConstructionWritable {
    pub authority: String,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DomesticConstructionMounts {
    #[serde(default)]
    pub proc: bool,

    #[serde(default)]
    pub dev: bool,

    #[serde(default)]
    pub tmp: bool,
}

impl DomesticConstructionDeclaration {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let declaration: Self = serde_json::from_str(raw)
            .map_err(|error| format!("invalid domestic construction declaration JSON: {error}"))?;

        declaration.validate()?;

        Ok(declaration)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != DOMESTIC_CONSTRUCTION_SCHEMA {
            return Err(format!(
                "domestic construction schema must be String {DOMESTIC_CONSTRUCTION_SCHEMA}"
            ));
        }

        if self.name != DOMESTIC_CONSTRUCTION_NAME {
            return Err(format!(
                "domestic construction name must be {DOMESTIC_CONSTRUCTION_NAME}"
            ));
        }

        validate_domestic_construction_subject(&self.subject)?;

        if self.steps.is_empty() {
            return Err("domestic construction must declare at least one step".to_string());
        }

        let mut ids = BTreeSet::<String>::new();

        for step in &self.steps {
            step.validate()?;

            if !ids.insert(step.id.clone()) {
                return Err(format!(
                    "duplicate domestic construction step id: {}",
                    step.id
                ));
            }
        }

        Ok(())
    }

    pub fn step(&self, id: &str) -> Result<&DomesticConstructionStep, String> {
        if id.trim().is_empty() {
            return Err("domestic construction step id cannot be empty".to_string());
        }

        self.steps
            .iter()
            .find(|step| step.id == id)
            .ok_or_else(|| format!("unknown domestic construction step: {id}"))
    }
}

impl DomesticConstructionStep {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("domestic construction step id cannot be empty".to_string());
        }

        if self.world.trim().is_empty() {
            return Err(format!(
                "domestic construction step {} world cannot be empty",
                self.id
            ));
        }

        for authority in &self.readonly {
            if authority.trim().is_empty() {
                return Err(format!(
                    "domestic construction step {} readonly authority cannot be empty",
                    self.id
                ));
            }
        }

        let mut writable_authorities = BTreeSet::<String>::new();

        for writable in &self.writable {
            writable.validate(&self.id)?;

            if !writable_authorities.insert(writable.authority.clone()) {
                return Err(format!(
                    "domestic construction step {} contains duplicate writable authority {}",
                    self.id, writable.authority
                ));
            }
        }

        if let Some(chdir) = &self.chdir {
            require_absolute_clean_path(
                chdir,
                &format!("domestic construction step {} chdir", self.id),
            )?;
        }

        Ok(())
    }

    pub fn requested_authorities(&self) -> Vec<String> {
        let mut requested = Vec::<String>::new();

        requested.push("platform.filesystem_boundary".to_string());

        for authority in &self.readonly {
            if !requested.contains(authority) {
                requested.push(authority.clone());
            }
        }

        for writable in &self.writable {
            if !requested.contains(&writable.authority) {
                requested.push(writable.authority.clone());
            }
        }

        requested
    }
}

impl DomesticConstructionWritable {
    fn validate(&self, step_id: &str) -> Result<(), String> {
        if self.authority.trim().is_empty() {
            return Err(format!(
                "domestic construction step {step_id} writable authority cannot be empty"
            ));
        }

        require_absolute_clean_path(
            &self.source,
            &format!("domestic construction step {step_id} writable source"),
        )?;

        require_absolute_clean_path(
            &self.destination,
            &format!("domestic construction step {step_id} writable destination"),
        )?;

        Ok(())
    }
}

fn require_absolute_clean_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }

    for component in path.components() {
        if matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        ) {
            return Err(format!(
                "{label} must not contain relative traversal components: {}",
                path.display()
            ));
        }
    }

    Ok(())
}

pub fn project_construction_step(
    declaration: &DomesticConstructionDeclaration,
    step_id: &str,
    runtime_manifest_path: &Path,
    registry: &SuppliedAuthorityRegistry,
) -> Result<WorkspaceExecutionRequest, String> {
    declaration.validate()?;

    require_absolute_clean_path(
        runtime_manifest_path,
        "domestic construction runtime manifest path",
    )?;

    let step = declaration.step(step_id)?;

    let requested = step.requested_authorities();

    let grants = build_authority_grant_set(registry, requested.iter().map(String::as_str))?;

    let platform_descriptor_path =
        grants.descriptor_path(registry, "platform.filesystem_boundary")?;

    let readonly = step
        .readonly
        .iter()
        .map(|authority| {
            let descriptor_path = grants.descriptor_path(registry, authority)?;

            Ok(WorkspaceReadonlyGrant {
                authority: authority.clone(),
                descriptor_path,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let writable = step
        .writable
        .iter()
        .map(|entry| {
            let descriptor_path = grants.descriptor_path(registry, &entry.authority)?;

            Ok(WorkspaceWritableGrant {
                authority: entry.authority.clone(),
                descriptor_path,
                source: entry.source.clone(),
                destination: entry.destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(WorkspaceExecutionRequest {
        manifest_path: runtime_manifest_path.to_path_buf(),
        world: step.world.clone(),
        platform_descriptor_path,
        readonly,
        writable,
        mount_proc: step.mounts.proc,
        mount_dev: step.mounts.dev,
        mount_tmp: step.mounts.tmp,
        chdir: step.chdir.clone(),
        arguments: step.arguments.iter().map(OsString::from).collect(),
    })
}

pub fn build_construction_step_command(
    declaration: &DomesticConstructionDeclaration,
    step_id: &str,
    runtime_manifest_path: &Path,
    registry: &SuppliedAuthorityRegistry,
) -> Result<std::process::Command, String> {
    let request = project_construction_step(declaration, step_id, runtime_manifest_path, registry)?;

    build_workspace_execution_command(&request)
}

pub fn execute_construction_step(
    declaration: &DomesticConstructionDeclaration,
    step_id: &str,
    runtime_manifest_path: &Path,
    registry: &SuppliedAuthorityRegistry,
) -> Result<i32, String> {
    let request = project_construction_step(declaration, step_id, runtime_manifest_path, registry)?;

    execute_workspace_execution_request(&request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domestic_authority_supply::{load_authority_supply, register_supplied_authorities};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must work")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "neebles-domestic-construction-{label}-{}-{unique}",
            std::process::id()
        ));

        fs::create_dir_all(&path).expect("fixture root must exist");

        path
    }

    fn valid_json() -> String {
        serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "fixture.subject",
            "steps": [
                {
                    "id": "fetch",
                    "world": "boss.git",
                    "readonly": [
                        "system.fixture.readonly"
                    ],
                    "writable": [
                        {
                            "authority": "neebles.domestic_workspace",
                            "source": "/opt/neebles-build/modules/fixture",
                            "destination": "/work/fixture"
                        }
                    ],
                    "mounts": {
                        "proc": false,
                        "dev": false,
                        "tmp": true
                    },
                    "chdir": "/work",
                    "arguments": [
                        "clone",
                        "opaque-value"
                    ]
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn valid_generic_declaration_parses() {
        let declaration = DomesticConstructionDeclaration::parse(&valid_json())
            .expect("generic construction declaration must parse");

        assert_eq!(declaration.subject, "fixture.subject");
        assert_eq!(declaration.steps.len(), 1);
        assert_eq!(declaration.steps[0].world, "boss.git");
    }

    #[test]
    fn technology_specific_root_field_is_rejected() {
        let raw = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "fixture.subject",
            "technology": "qt",
            "steps": [
                {
                    "id": "build",
                    "world": "boss.git"
                }
            ]
        })
        .to_string();

        assert!(DomesticConstructionDeclaration::parse(&raw).is_err());
    }

    #[test]
    fn consumer_cannot_supply_descriptor_paths() {
        let raw = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "fixture.subject",
            "steps": [
                {
                    "id": "build",
                    "world": "boss.git",
                    "writable": [
                        {
                            "authority": "neebles.domestic_workspace",
                            "source": "/opt/neebles-build/modules/fixture",
                            "destination": "/work/fixture",
                            "descriptor_path": "/tmp/forged.json"
                        }
                    ]
                }
            ]
        })
        .to_string();

        assert!(DomesticConstructionDeclaration::parse(&raw).is_err());
    }

    #[test]
    fn duplicate_step_identity_is_rejected() {
        let raw = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "fixture.subject",
            "steps": [
                {
                    "id": "same",
                    "world": "boss.git"
                },
                {
                    "id": "same",
                    "world": "boss.tar"
                }
            ]
        })
        .to_string();

        assert!(DomesticConstructionDeclaration::parse(&raw).is_err());
    }

    #[test]
    fn relative_writable_source_is_rejected() {
        let raw = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "fixture.subject",
            "steps": [
                {
                    "id": "build",
                    "world": "boss.git",
                    "writable": [
                        {
                            "authority": "neebles.domestic_workspace",
                            "source": "modules/fixture",
                            "destination": "/work/fixture"
                        }
                    ]
                }
            ]
        })
        .to_string();

        assert!(DomesticConstructionDeclaration::parse(&raw).is_err());
    }

    #[test]
    fn projection_resolves_descriptor_paths_from_registered_supply() {
        let base = fixture_root("projection");
        let descriptors = base.join("authority");

        fs::create_dir_all(&descriptors).expect("descriptor directory must exist");

        let platform = descriptors.join("platform.json");
        let readonly = descriptors.join("readonly.json");
        let writable = descriptors.join("writable.json");
        let supply_path = base.join("authority-supply.json");

        fs::write(
            &platform,
            serde_json::json!({
                "authority": "platform.filesystem_boundary"
            })
            .to_string(),
        )
        .expect("platform descriptor must exist");

        fs::write(
            &readonly,
            serde_json::json!({
                "authority": "system.fixture.readonly"
            })
            .to_string(),
        )
        .expect("readonly descriptor must exist");

        fs::write(
            &writable,
            serde_json::json!({
                "authority": "neebles.domestic_workspace"
            })
            .to_string(),
        )
        .expect("writable descriptor must exist");

        fs::write(
            &supply_path,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": [
                    {
                        "authority": "platform.filesystem_boundary",
                        "location": platform
                    },
                    {
                        "authority": "system.fixture.readonly",
                        "location": readonly
                    },
                    {
                        "authority": "neebles.domestic_workspace",
                        "location": writable
                    }
                ]
            })
            .to_string(),
        )
        .expect("authority supply must exist");

        let supply = load_authority_supply(&supply_path).expect("authority supply must load");

        let registry =
            register_supplied_authorities(&supply).expect("authority supply must register");

        let declaration =
            DomesticConstructionDeclaration::parse(&valid_json()).expect("declaration must parse");

        let request = project_construction_step(
            &declaration,
            "fetch",
            Path::new("/opt/neebles-build/runtime/domestic-runtime.json"),
            &registry,
        )
        .expect("construction step must project");

        assert_eq!(request.platform_descriptor_path, platform);

        assert_eq!(request.readonly[0].descriptor_path, readonly);

        assert_eq!(request.writable[0].descriptor_path, writable);

        assert_eq!(request.writable[0].authority, "neebles.domestic_workspace");

        assert_eq!(request.world, "boss.git");

        assert_eq!(
            request.arguments,
            vec![OsString::from("clone"), OsString::from("opaque-value")]
        );

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn projection_rejects_authority_not_supplied_and_registered() {
        let base = fixture_root("missing-authority");
        let descriptors = base.join("authority");

        fs::create_dir_all(&descriptors).expect("descriptor directory must exist");

        let platform = descriptors.join("platform.json");
        let supply_path = base.join("authority-supply.json");

        fs::write(
            &platform,
            serde_json::json!({
                "authority": "platform.filesystem_boundary"
            })
            .to_string(),
        )
        .expect("platform descriptor must exist");

        fs::write(
            &supply_path,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": [
                    {
                        "authority": "platform.filesystem_boundary",
                        "location": platform
                    }
                ]
            })
            .to_string(),
        )
        .expect("authority supply must exist");

        let supply = load_authority_supply(&supply_path).expect("authority supply must load");

        let registry =
            register_supplied_authorities(&supply).expect("authority supply must register");

        let declaration =
            DomesticConstructionDeclaration::parse(&valid_json()).expect("declaration must parse");

        assert!(project_construction_step(
            &declaration,
            "fetch",
            Path::new("/opt/neebles-build/runtime/domestic-runtime.json"),
            &registry,
        )
        .is_err());

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn construction_command_reaches_existing_workspace_engine() {
        let base = fixture_root("engine-handoff");
        let descriptors = base.join("authority");

        fs::create_dir_all(&descriptors).expect("descriptor directory must exist");

        let platform = descriptors.join("platform.json");
        let writable = descriptors.join("writable.json");
        let supply_path = base.join("authority-supply.json");

        fs::write(
            &platform,
            serde_json::json!({
                "schema": "1",
                "name": "intentionally-invalid-platform-fixture",
                "authority": "platform.filesystem_boundary"
            })
            .to_string(),
        )
        .expect("platform descriptor fixture must exist");

        fs::write(
            &writable,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-writable-data-authority",
                "authority": "neebles.domestic_workspace",
                "root": base.join("workspace")
            })
            .to_string(),
        )
        .expect("writable descriptor fixture must exist");

        fs::write(
            &supply_path,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": [
                    {
                        "authority": "platform.filesystem_boundary",
                        "location": platform
                    },
                    {
                        "authority": "neebles.domestic_workspace",
                        "location": writable
                    }
                ]
            })
            .to_string(),
        )
        .expect("authority supply must exist");

        let supply = load_authority_supply(&supply_path).expect("authority supply must load");

        let registry =
            register_supplied_authorities(&supply).expect("authority supply must register");

        let declaration =
            DomesticConstructionDeclaration::parse(&valid_json()).expect("declaration must parse");

        let error = build_construction_step_command(
            &declaration,
            "fetch",
            Path::new("/opt/neebles-build/runtime/domestic-runtime.json"),
            &registry,
        )
        .expect_err("invalid platform descriptor must be rejected by existing workspace engine");

        assert!(
            error.contains("platform")
                || error.contains("descriptor")
                || error.contains("authority")
        );

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn canonical_subject_maps_deterministically_to_json() {
        let path = canonical_domestic_construction_declaration_path("fixture.subject")
            .expect("safe subject must resolve");

        assert_eq!(
            path,
            PathBuf::from("/usr/lib/neebles/domestic/construction/fixture.subject.json")
        );
    }

    #[test]
    fn canonical_subject_rejects_path_escape_characters() {
        for subject in [
            "../escape",
            "folder/name",
            "folder\\name",
            "name with spaces",
            "/absolute",
        ] {
            assert!(
                canonical_domestic_construction_declaration_path(subject).is_err(),
                "unsafe subject must be rejected: {subject}"
            );
        }
    }

    #[test]
    fn canonical_subject_accepts_generic_safe_identity() {
        for subject in ["fixture.subject", "neebles-test-module", "alpha_beta", "A1"] {
            validate_domestic_construction_subject(subject)
                .expect("generic safe identity must be accepted");
        }
    }

    #[test]
    fn declaration_subject_validation_uses_canonical_identity_law() {
        let raw = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-construction",
            "subject": "../escape",
            "steps": [
                {
                    "id": "build",
                    "world": "boss.git"
                }
            ]
        })
        .to_string();

        assert!(DomesticConstructionDeclaration::parse(&raw).is_err());
    }

    #[test]
    fn canonical_loader_rejects_untrusted_non_platform_file() {
        let base = fixture_root("canonical-provenance");
        let declaration_path = base.join("fixture.subject.json");

        fs::write(&declaration_path, valid_json()).expect("fixture declaration must exist");

        let error = authenticate_platform_controlled_file(&declaration_path)
            .expect_err("ordinary user-owned fixture must not be platform-controlled");

        assert!(
            error.contains("root-owned")
                || error.contains("group-writable")
                || error.contains("other-writable")
        );

        fs::remove_dir_all(base).expect("fixture must clean");
    }
}
