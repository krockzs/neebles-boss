use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use crate::domestic_authority_supply::{
    load_authority_supply, register_supplied_authorities, AuthoritySupply,
    SuppliedAuthorityRegistry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformControlledPath {
    path: PathBuf,
}

impl PlatformControlledPath {
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }
}

fn inspect_platform_controlled_component(
    path: &Path,
    require_directory: bool,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not inspect platform-controlled path component {}: {error}",
            path.display()
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "platform-controlled path component is symlink: {}",
            path.display()
        ));
    }

    if metadata.uid() != 0 {
        return Err(format!(
            "platform-controlled path component is not root-owned: {} uid={}",
            path.display(),
            metadata.uid()
        ));
    }

    let mode = metadata.permissions().mode() & 0o7777;

    if mode & 0o020 != 0 {
        return Err(format!(
            "platform-controlled path component is group-writable: {} mode={:#o}",
            path.display(),
            mode
        ));
    }

    if mode & 0o002 != 0 {
        return Err(format!(
            "platform-controlled path component is other-writable: {} mode={:#o}",
            path.display(),
            mode
        ));
    }

    if require_directory && !metadata.is_dir() {
        return Err(format!(
            "platform-controlled path ancestor is not directory: {}",
            path.display()
        ));
    }

    Ok(())
}

pub fn authenticate_platform_controlled_file(
    path: &Path,
) -> Result<PlatformControlledPath, String> {
    if !path.is_absolute() {
        return Err(format!(
            "platform-controlled path must be absolute: {}",
            path.display()
        ));
    }

    inspect_platform_controlled_component(Path::new("/"), true)?;

    let normal_components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(Ok(part)),
            Component::RootDir => None,
            Component::CurDir => Some(Err(format!(
                "platform-controlled path cannot contain current-directory components: {}",
                path.display()
            ))),
            Component::ParentDir => Some(Err(format!(
                "platform-controlled path cannot contain parent traversal: {}",
                path.display()
            ))),
            Component::Prefix(_) => Some(Err(format!(
                "unsupported platform-controlled path prefix: {}",
                path.display()
            ))),
        })
        .collect::<Result<Vec<_>, String>>()?;

    if normal_components.is_empty() {
        return Err(format!(
            "platform-controlled target must be a regular file, not filesystem root: {}",
            path.display()
        ));
    }

    let mut current = PathBuf::from("/");

    for (index, part) in normal_components.iter().enumerate() {
        current.push(part);

        let is_target = index + 1 == normal_components.len();

        inspect_platform_controlled_component(&current, !is_target)?;
    }

    let target = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not inspect platform-controlled target {}: {error}",
            path.display()
        )
    })?;

    if !target.is_file() {
        return Err(format!(
            "platform-controlled target is not regular file: {}",
            path.display()
        ));
    }

    Ok(PlatformControlledPath {
        path: path.to_path_buf(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformControlledAuthoritySupply {
    supply_path: PlatformControlledPath,
    supply: AuthoritySupply,
}

impl PlatformControlledAuthoritySupply {
    pub fn supply_path(&self) -> &Path {
        self.supply_path.as_path()
    }

    pub fn supply(&self) -> &AuthoritySupply {
        &self.supply
    }
}

pub fn load_platform_controlled_authority_supply(
    supply_path: &Path,
) -> Result<PlatformControlledAuthoritySupply, String> {
    let controlled = authenticate_platform_controlled_file(supply_path)?;

    let supply = load_authority_supply(controlled.as_path())?;

    Ok(PlatformControlledAuthoritySupply {
        supply_path: controlled,
        supply,
    })
}

pub fn register_platform_controlled_authorities(
    supplied: &PlatformControlledAuthoritySupply,
) -> Result<SuppliedAuthorityRegistry, String> {
    for entry in supplied.supply().entries() {
        authenticate_platform_controlled_file(
            &entry.descriptor_path,
        )
        .map_err(|error| {
            format!(
                "supplied authority descriptor is not platform-controlled: authority={} path={} error={error}",
                entry.authority,
                entry.descriptor_path.display()
            )
        })?;
    }

    register_supplied_authorities(supplied.supply())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-platform-control-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn known_system_file_can_be_platform_controlled() {
        let path = Path::new("/etc/passwd");

        let controlled = authenticate_platform_controlled_file(path)
            .expect("root-controlled system file must authenticate");

        assert_eq!(controlled.as_path(), path,);
    }

    #[test]
    fn current_boundary_provider_can_be_platform_controlled() {
        let path = Path::new("/usr/bin/bwrap");

        if !path.exists() {
            return;
        }

        let controlled = authenticate_platform_controlled_file(path)
            .expect("system boundary provider must authenticate on this host");

        assert_eq!(controlled.as_path(), path,);
    }

    #[test]
    fn user_owned_file_is_not_platform_controlled() {
        let base = fixture_root("user");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let path = base.join("supply.json");

        fs::write(&path, b"{}\n").expect("fixture file must exist");

        assert!(authenticate_platform_controlled_file(&path,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn resolver_symlink_is_not_platform_metadata() {
        let path = Path::new("/etc/resolv.conf");

        if !path.exists() {
            return;
        }

        let metadata = fs::symlink_metadata(path).expect("resolver identity must be inspectable");

        if metadata.file_type().is_symlink() {
            assert!(authenticate_platform_controlled_file(path,).is_err());
        }
    }

    #[test]
    fn generic_supply_load_does_not_authenticate_origin() {
        let base = fixture_root("supply");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let supply_path = base.join("supply.json");

        fs::write(
            &supply_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": []
            }))
            .expect("fixture supply must serialize"),
        )
        .expect("fixture supply must exist");

        assert!(crate::domestic_authority_supply::load_authority_supply(&supply_path,).is_ok());

        assert!(load_platform_controlled_authority_supply(&supply_path,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn descriptor_binding_requires_platform_control() {
        let base = fixture_root("descriptor");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let descriptor = base.join("descriptor.json");

        fs::write(
            &descriptor,
            serde_json::to_vec_pretty(&serde_json::json!({
                "authority": "system.fixture"
            }))
            .expect("fixture descriptor must serialize"),
        )
        .expect("fixture descriptor must exist");

        let supply_path = base.join("supply.json");

        fs::write(
            &supply_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": [
                    {
                        "authority": "system.fixture",
                        "location": descriptor
                    }
                ]
            }))
            .expect("fixture supply must serialize"),
        )
        .expect("fixture supply must exist");

        let generic = crate::domestic_authority_supply::load_authority_supply(&supply_path)
            .expect("generic supply may load");

        let synthetic = PlatformControlledAuthoritySupply {
            supply_path: PlatformControlledPath {
                path: supply_path.clone(),
            },
            supply: generic,
        };

        assert!(register_platform_controlled_authorities(&synthetic,).is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn platform_control_does_not_reinterpret_external_data_source() {
        let resolver = Path::new("/etc/resolv.conf");

        if !resolver.exists() {
            return;
        }

        let descriptor = crate::domestic_external_data_authority::ExternalDataAuthorityDescriptor {
            authority: "system.dns_resolver_config".to_string(),
            descriptor_path: PathBuf::from("/fixture/descriptor.json"),
            source: resolver.to_path_buf(),
            destination: PathBuf::from("/etc/resolv.conf"),
            access: "read_only".to_string(),
        };

        assert_eq!(descriptor.source, resolver,);
    }
}
