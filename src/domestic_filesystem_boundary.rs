use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::domestic_external_data_authority::ExternalDataAuthorityDescriptor;
use crate::domestic_platform_authority::PlatformAuthorityDescriptor;
use crate::domestic_writable_data_authority::WritableDataGrant;

pub const FILESYSTEM_BOUNDARY_PROTOCOL_V1: &str = "neebles-filesystem-boundary-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemBoundaryPlan {
    pub provider: PathBuf,
    pub root: PathBuf,
    pub readonly_data: Vec<ExternalDataAuthorityDescriptor>,
    pub writable_data: Vec<WritableDataGrant>,
    pub environment: BTreeMap<String, String>,
    pub mount_proc: bool,
    pub mount_dev: bool,
    pub mount_tmp: bool,
    pub chdir: Option<PathBuf>,
}

fn validate_absolute_clean_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "{label} cannot contain parent traversal: {}",
                path.display()
            ));
        }
    }

    Ok(())
}

pub fn project_path_into_boundary(root: &Path, physical_path: &Path) -> Result<PathBuf, String> {
    validate_absolute_clean_path(root, "filesystem boundary root")?;

    validate_absolute_clean_path(physical_path, "physical boundary path")?;

    let relative = physical_path.strip_prefix(root).map_err(|_| {
        format!(
            "physical path is outside filesystem boundary root: {}",
            physical_path.display()
        )
    })?;

    let mut projected = PathBuf::from("/");
    projected.push(relative);

    Ok(projected)
}

pub fn compose_filesystem_boundary_plan(
    platform: &PlatformAuthorityDescriptor,
    root: &Path,
    readonly_data: &[ExternalDataAuthorityDescriptor],
    environment: &BTreeMap<String, String>,
    mount_proc: bool,
    mount_dev: bool,
    mount_tmp: bool,
    chdir: Option<&Path>,
) -> Result<FilesystemBoundaryPlan, String> {
    if platform.protocol() != FILESYSTEM_BOUNDARY_PROTOCOL_V1 {
        return Err(format!(
            "unsupported filesystem boundary protocol: {}",
            platform.protocol()
        ));
    }

    validate_absolute_clean_path(root, "filesystem boundary root")?;

    if !root.is_dir() {
        return Err(format!(
            "filesystem boundary root is not a directory: {}",
            root.display()
        ));
    }

    let mut destinations = BTreeSet::<PathBuf>::new();

    for grant in readonly_data {
        if grant.access != "read_only" {
            return Err(format!(
                "filesystem boundary received unsupported external data access mode: {}",
                grant.access
            ));
        }

        if !destinations.insert(grant.destination.clone()) {
            return Err(format!(
                "duplicate external data destination: {}",
                grant.destination.display()
            ));
        }
    }

    let chdir = match chdir {
        Some(path) => {
            validate_absolute_clean_path(path, "filesystem boundary chdir")?;

            Some(path.to_path_buf())
        }

        None => None,
    };

    Ok(FilesystemBoundaryPlan {
        provider: platform.provider().to_path_buf(),
        root: root.to_path_buf(),
        readonly_data: readonly_data.to_vec(),
        writable_data: Vec::new(),
        environment: environment.clone(),
        mount_proc,
        mount_dev,
        mount_tmp,
        chdir,
    })
}

pub fn compose_filesystem_boundary_plan_with_writable_data(
    platform: &PlatformAuthorityDescriptor,
    root: &Path,
    readonly_data: &[ExternalDataAuthorityDescriptor],
    writable_data: &[WritableDataGrant],
    environment: &BTreeMap<String, String>,
    mount_proc: bool,
    mount_dev: bool,
    mount_tmp: bool,
    chdir: Option<&Path>,
) -> Result<FilesystemBoundaryPlan, String> {
    let mut plan = compose_filesystem_boundary_plan(
        platform,
        root,
        readonly_data,
        environment,
        mount_proc,
        mount_dev,
        mount_tmp,
        chdir,
    )?;

    let mut destinations = plan
        .readonly_data
        .iter()
        .map(|grant| grant.destination.clone())
        .collect::<BTreeSet<_>>();

    for grant in writable_data {
        validate_absolute_clean_path(&grant.source, "filesystem boundary writable source")?;
        validate_absolute_clean_path(
            &grant.destination,
            "filesystem boundary writable destination",
        )?;

        if !grant.source.exists() {
            return Err(format!(
                "filesystem boundary writable source does not exist: {}",
                grant.source.display()
            ));
        }

        if !destinations.insert(grant.destination.clone()) {
            return Err(format!(
                "duplicate external data destination: {}",
                grant.destination.display()
            ));
        }
    }

    plan.writable_data = writable_data.to_vec();

    Ok(plan)
}

pub fn build_filesystem_boundary_command(
    plan: &FilesystemBoundaryPlan,
    program_inside_boundary: &Path,
    program_arguments: &[OsString],
) -> Result<Command, String> {
    validate_absolute_clean_path(program_inside_boundary, "filesystem boundary program")?;

    let mut command = Command::new(&plan.provider);

    command.env_clear();

    command.arg("--root").arg(&plan.root);

    for grant in &plan.readonly_data {
        command
            .arg("--readonly-data")
            .arg(&grant.source)
            .arg(&grant.destination);
    }

    for grant in &plan.writable_data {
        command
            .arg("--writable-data")
            .arg(&grant.source)
            .arg(&grant.destination);
    }

    for (key, value) in &plan.environment {
        if key.is_empty()
            || !key.chars().enumerate().all(|(index, character)| {
                if index == 0 {
                    character == '_' || character.is_ascii_alphabetic()
                } else {
                    character == '_' || character.is_ascii_alphanumeric()
                }
            })
        {
            return Err(format!(
                "filesystem boundary environment key is invalid: {key}"
            ));
        }

        command.arg("--environment").arg(key).arg(value);
    }

    if plan.mount_proc {
        command.arg("--proc");
    }

    if plan.mount_dev {
        command.arg("--dev");
    }

    if plan.mount_tmp {
        command.arg("--tmp");
    }

    if let Some(chdir) = &plan.chdir {
        command.arg("--chdir").arg(chdir);
    }

    command
        .arg("--")
        .arg(program_inside_boundary)
        .args(program_arguments);

    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domestic_external_data_authority::load_external_data_authority_descriptor;
    use crate::domestic_platform_authority::test_platform_authority_descriptor;

    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-boundary-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn boundary_plan_separates_available_from_granted() {
        let base = fixture_root("grant");

        let world = base.join("rootfs");

        let provider = base.join("provider");

        let source = base.join("resolver.conf");

        fs::create_dir_all(&world).expect("world must exist");

        fs::write(&provider, b"provider").expect("provider must exist");

        fs::write(&source, b"nameserver fixture\n").expect("source must exist");

        let platform_path = base.join("platform.json");

        let external_path = base.join("external.json");

        fs::write(
            &platform_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name":
                    "neebles-platform-authority",
                "authority":
                    "platform.filesystem_boundary",
                "protocol":
                    "neebles-filesystem-boundary-v1",
                "provider":
                    provider,
            }))
            .expect("platform descriptor must serialize"),
        )
        .expect("platform descriptor must exist");

        fs::write(
            &external_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name":
                    "neebles-external-data-authority",
                "authority":
                    "system.dns_resolver_config",
                "source":
                    source,
                "destination":
                    "/etc/resolv.conf",
                "access":
                    "read_only",
            }))
            .expect("external descriptor must serialize"),
        )
        .expect("external descriptor must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            platform_path.clone(),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider.clone(),
        );

        let dns =
            load_external_data_authority_descriptor(&external_path, "system.dns_resolver_config")
                .expect("external data authority must load");

        let without_grant = compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[],
            &BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )
        .expect("available authority without grant must still compose");

        assert!(without_grant.readonly_data.is_empty());

        let with_grant = compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[dns.clone()],
            &BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )
        .expect("explicitly granted authority must compose");

        assert_eq!(with_grant.readonly_data, vec![dns]);

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn boundary_command_projects_only_explicit_environment_through_neebles_protocol() {
        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            PathBuf::from("/usr/lib/neebles/platform/authority/fixture.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            PathBuf::from("/usr/lib/neebles/platform/bin/fixture-provider"),
        );

        let environment = BTreeMap::from([
            ("DISPLAY".to_string(), ":0".to_string()),
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
        ]);

        let plan = compose_filesystem_boundary_plan(
            &platform,
            Path::new("/tmp"),
            &[],
            &environment,
            false,
            false,
            false,
            None,
        )
        .expect("explicit environment must compose");

        let command = build_filesystem_boundary_command(&plan, Path::new("/usr/bin/python3"), &[])
            .expect("boundary command must build");

        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args
            .windows(3)
            .any(|window| { window == ["--environment", "DISPLAY", ":0"] }));

        assert!(args
            .windows(3)
            .any(|window| { window == ["--environment", "XDG_RUNTIME_DIR", "/run/user/1000"] }));

        assert!(
            !args.iter().any(|value| value == "--setenv"),
            "Boss must speak NEEBLES boundary protocol, not backend bwrap protocol",
        );
    }

    #[test]
    fn boundary_command_rejects_invalid_environment_key() {
        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            PathBuf::from("/usr/lib/neebles/platform/authority/fixture.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            PathBuf::from("/usr/lib/neebles/platform/bin/fixture-provider"),
        );

        let environment = BTreeMap::from([("INVALID=KEY".to_string(), "value".to_string())]);

        let plan = compose_filesystem_boundary_plan(
            &platform,
            Path::new("/tmp"),
            &[],
            &environment,
            false,
            false,
            false,
            None,
        )
        .expect("plan construction may retain opaque environment until command certification");

        let error = build_filesystem_boundary_command(&plan, Path::new("/usr/bin/python3"), &[])
            .expect_err("invalid environment key must be rejected");

        assert!(error.contains("environment"));
    }

    #[test]
    fn boundary_command_speaks_neebles_protocol_not_backend_protocol() {
        let base = fixture_root("command");

        let world = base.join("rootfs");

        let provider = base.join("provider");

        fs::create_dir_all(&world).expect("world must exist");

        fs::write(&provider, b"provider").expect("provider must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider.clone(),
        );

        let plan = compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[],
            &BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )
        .expect("plan must compose");

        let command = build_filesystem_boundary_command(
            &plan,
            Path::new("/usr/bin/tool"),
            &[OsString::from("--fixture")],
        )
        .expect("command must compose");

        assert_eq!(command.get_program(), provider.as_os_str());

        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        let expected = vec![
            "--root".to_string(),
            world.to_string_lossy().to_string(),
            "--proc".to_string(),
            "--dev".to_string(),
            "--tmp".to_string(),
            "--chdir".to_string(),
            "/tmp".to_string(),
            "--".to_string(),
            "/usr/bin/tool".to_string(),
            "--fixture".to_string(),
        ];

        assert_eq!(args, expected);

        assert!(
            args.iter().all(|arg| { !arg.contains("bwrap",) },),
            "Boss-side boundary protocol must not contain backend technology"
        );

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn boundary_command_keeps_dynamic_readonly_on_readonly_protocol() {
        let base = fixture_root("dynamic-readonly-command");
        let world = base.join("rootfs");
        let provider = base.join("provider");
        let source = base.join("module");

        fs::create_dir_all(&world).expect("world must exist");
        fs::create_dir_all(&source).expect("source must exist");
        fs::write(&provider, b"provider").expect("provider must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider,
        );

        let grant = ExternalDataAuthorityDescriptor {
            authority: "modules.installed_runtime".to_string(),
            descriptor_path: base.join("dynamic.json"),
            source: source.clone(),
            destination: PathBuf::from("/modules/test-module"),
            access: "read_only".to_string(),
        };

        let plan = compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[grant],
            &BTreeMap::new(),
            false,
            false,
            false,
            None,
        )
        .expect("dynamic readonly plan must compose");

        let command = build_filesystem_boundary_command(&plan, Path::new("/usr/bin/tool"), &[])
            .expect("boundary command must compose");

        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.iter().any(|value| value == "--readonly-data"));
        assert!(!args.iter().any(|value| value == "--writable-data"));

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn boundary_projection_turns_physical_authority_into_inside_path() {
        let root = Path::new("/authority/rootfs");

        assert_eq!(
            project_path_into_boundary(root, Path::new("/authority/rootfs/usr/bin/git",),)
                .expect("inside path must project",),
            PathBuf::from("/usr/bin/git",)
        );

        assert!(project_path_into_boundary(root, Path::new("/host/usr/bin/git",),).is_err());
    }

    #[test]
    fn boundary_plan_rejects_duplicate_external_destination() {
        let base = fixture_root("duplicates");

        let world = base.join("rootfs");

        let provider = base.join("provider");

        let source_a = base.join("a");

        let source_b = base.join("b");

        fs::create_dir_all(&world).expect("world must exist");

        fs::write(&provider, b"provider").expect("provider must exist");

        fs::write(&source_a, b"a").expect("source a must exist");

        fs::write(&source_b, b"b").expect("source b must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider,
        );

        let first = ExternalDataAuthorityDescriptor {
            authority: "system.first".to_string(),
            descriptor_path: base.join("first.json"),
            source: source_a,
            destination: PathBuf::from("/etc/shared"),
            access: "read_only".to_string(),
        };

        let second = ExternalDataAuthorityDescriptor {
            authority: "system.second".to_string(),
            descriptor_path: base.join("second.json"),
            source: source_b,
            destination: PathBuf::from("/etc/shared"),
            access: "read_only".to_string(),
        };

        assert!(compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[first, second,],
            &BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp"),),
        )
        .is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn boundary_plan_rejects_unknown_provider_protocol() {
        let base = fixture_root("protocol");

        let world = base.join("rootfs");

        let provider = base.join("provider");

        fs::create_dir_all(&world).expect("world must exist");

        fs::write(&provider, b"provider").expect("provider must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            "random-provider-language".to_string(),
            provider,
        );

        assert!(compose_filesystem_boundary_plan(
            &platform,
            &world,
            &[],
            &BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp"),),
        )
        .is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}
