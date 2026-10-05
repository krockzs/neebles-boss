use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Component, Path, PathBuf};

use crate::domestic_authority_supply::build_authority_grant_set;
use crate::domestic_capability_provider::resolve_platform_capability;
use crate::domestic_platform_authority::{
    load_platform_authority_descriptor, PlatformAuthorityDescriptor,
};

pub const DESKTOP_SESSION_INTERFACE_AUTHORITY: &str = "platform.desktop_session_interface";

pub const DESKTOP_SESSION_INTERFACE_PROTOCOL_V1: &str = "neebles-desktop-session-interface-v1";

pub const XDG_RUNTIME_DIR_KEY: &str = "XDG_RUNTIME_DIR";

pub const DBUS_SESSION_BUS_ADDRESS_KEY: &str = "DBUS_SESSION_BUS_ADDRESS";

pub const DISPLAY_KEY: &str = "DISPLAY";

pub const WAYLAND_DISPLAY_KEY: &str = "WAYLAND_DISPLAY";

pub const XAUTHORITY_KEY: &str = "XAUTHORITY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSessionReadonlyPath {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSessionInterface {
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
    environment: BTreeMap<String, String>,
    readonly_paths: Vec<DesktopSessionReadonlyPath>,
}

impl DesktopSessionInterface {
    pub fn desktop_uid(&self) -> libc::uid_t {
        self.desktop_uid
    }

    pub fn desktop_gid(&self) -> libc::gid_t {
        self.desktop_gid
    }

    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }

    pub fn readonly_paths(&self) -> &[DesktopSessionReadonlyPath] {
        &self.readonly_paths
    }

    pub fn runtime_dir(&self) -> &str {
        self.environment
            .get(XDG_RUNTIME_DIR_KEY)
            .expect("validated desktop session interface must contain XDG_RUNTIME_DIR")
    }

    pub fn session_bus_address(&self) -> &str {
        self.environment
            .get(DBUS_SESSION_BUS_ADDRESS_KEY)
            .expect("validated desktop session interface must contain DBUS_SESSION_BUS_ADDRESS")
    }

    pub fn grant_runtime_readonly(
        &self,
        relative: &Path,
    ) -> Result<DesktopSessionReadonlyPath, String> {
        if relative.as_os_str().is_empty() {
            return Err("desktop session runtime readonly path cannot be empty".to_string());
        }

        if relative.is_absolute() {
            return Err(format!(
                "desktop session runtime readonly path must be relative: {}",
                relative.display()
            ));
        }

        for component in relative.components() {
            if !matches!(component, Component::Normal(_)) {
                return Err(format!(
                    "desktop session runtime readonly path must contain only normal relative components: {}",
                    relative.display()
                ));
            }
        }

        let runtime_root = fs::canonicalize(self.runtime_dir()).map_err(|error| {
            format!(
                "could not canonicalize certified desktop runtime directory {}: {error}",
                self.runtime_dir()
            )
        })?;

        let declared = Path::new(self.runtime_dir()).join(relative);

        let metadata = fs::symlink_metadata(&declared).map_err(|error| {
            format!(
                "could not inspect desktop session runtime readonly resource {}: {error}",
                declared.display()
            )
        })?;

        if metadata.file_type().is_symlink() {
            return Err(format!(
                "desktop session runtime readonly resource cannot be a symlink: {}",
                declared.display()
            ));
        }

        if metadata.is_dir() {
            return Err(format!(
                "desktop session runtime readonly resource cannot expose a directory: {}",
                declared.display()
            ));
        }

        if !metadata.is_file() && !metadata.file_type().is_socket() {
            return Err(format!(
                "desktop session runtime readonly resource must be a regular file or Unix socket: {}",
                declared.display()
            ));
        }

        if metadata.uid() != self.desktop_uid {
            return Err(format!(
                "desktop session runtime readonly resource owner mismatch: path={} owner={} expected={}",
                declared.display(),
                metadata.uid(),
                self.desktop_uid
            ));
        }

        let source = fs::canonicalize(&declared).map_err(|error| {
            format!(
                "could not canonicalize desktop session runtime readonly resource {}: {error}",
                declared.display()
            )
        })?;

        if source == runtime_root || !source.starts_with(&runtime_root) {
            return Err(format!(
                "desktop session runtime readonly resource escapes certified runtime directory: {}",
                source.display()
            ));
        }

        Ok(DesktopSessionReadonlyPath {
            source: source.clone(),
            destination: source,
        })
    }
}

fn explicit_identity_inputs(
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
) -> BTreeMap<String, String> {
    let mut inputs = BTreeMap::from([
        ("NEEBLES_DESKTOP_UID".to_string(), desktop_uid.to_string()),
        ("NEEBLES_DESKTOP_GID".to_string(), desktop_gid.to_string()),
    ]);

    for key in [DISPLAY_KEY, WAYLAND_DISPLAY_KEY, XAUTHORITY_KEY] {
        if let Some(value) = std::env::var_os(key) {
            let value = value.to_string_lossy().into_owned();

            if !value.trim().is_empty() {
                inputs.insert(key.to_string(), value);
            }
        }
    }

    inputs
}

fn desktop_session_readonly_paths(
    environment: &BTreeMap<String, String>,
) -> Result<Vec<DesktopSessionReadonlyPath>, String> {
    let runtime_dir = PathBuf::from(
        environment
            .get(XDG_RUNTIME_DIR_KEY)
            .ok_or_else(|| "desktop session missing XDG_RUNTIME_DIR".to_string())?,
    );

    let bus_address = environment
        .get(DBUS_SESSION_BUS_ADDRESS_KEY)
        .ok_or_else(|| "desktop session missing DBUS_SESSION_BUS_ADDRESS".to_string())?;

    let bus_path = bus_address
        .strip_prefix("unix:path=")
        .ok_or_else(|| "desktop session bus address is not unix:path".to_string())?;

    let mut paths = Vec::<DesktopSessionReadonlyPath>::new();

    let mut push_path = |path: PathBuf| {
        if !paths.iter().any(|entry| entry.source == path) {
            paths.push(DesktopSessionReadonlyPath {
                source: path.clone(),
                destination: path,
            });
        }
    };

    push_path(PathBuf::from(bus_path));

    if let Some(wayland) = environment.get(WAYLAND_DISPLAY_KEY) {
        push_path(runtime_dir.join(wayland));
    }

    if let Some(xauthority) = environment.get(XAUTHORITY_KEY) {
        push_path(PathBuf::from(xauthority));
    }

    if let Some(display) = environment.get(DISPLAY_KEY) {
        let number = display
            .strip_prefix(":")
            .ok_or_else(|| format!("desktop session DISPLAY is not local: {display}"))?;

        if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
            return Err(format!(
                "desktop session DISPLAY does not contain a numeric display: {display}"
            ));
        }

        push_path(PathBuf::from("/tmp/.X11-unix").join(format!("X{number}")));
    }

    Ok(paths)
}

fn validate_desktop_session_values(
    values: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let required = BTreeSet::from([
        XDG_RUNTIME_DIR_KEY.to_string(),
        DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
    ]);

    let allowed = BTreeSet::from([
        XDG_RUNTIME_DIR_KEY.to_string(),
        DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
        DISPLAY_KEY.to_string(),
        WAYLAND_DISPLAY_KEY.to_string(),
        XAUTHORITY_KEY.to_string(),
    ]);

    let actual = values.keys().cloned().collect::<BTreeSet<_>>();

    if !required.is_subset(&actual) {
        return Err(format!(
            "desktop session interface missing required values: required={required:?} actual={actual:?}"
        ));
    }

    if !actual.is_subset(&allowed) {
        return Err(format!(
            "desktop session interface contains unauthorized values: allowed={allowed:?} actual={actual:?}"
        ));
    }

    let runtime_dir = values
        .get(XDG_RUNTIME_DIR_KEY)
        .expect("exact desktop session key validation must preserve XDG_RUNTIME_DIR")
        .trim();

    if runtime_dir.is_empty() {
        return Err("desktop session XDG_RUNTIME_DIR cannot be empty".to_string());
    }

    if !runtime_dir.starts_with('/') {
        return Err(format!(
            "desktop session XDG_RUNTIME_DIR must be absolute: {runtime_dir}"
        ));
    }

    let bus_address = values
        .get(DBUS_SESSION_BUS_ADDRESS_KEY)
        .expect("exact desktop session key validation must preserve DBUS_SESSION_BUS_ADDRESS")
        .trim();

    if bus_address.is_empty() {
        return Err("desktop session DBUS_SESSION_BUS_ADDRESS cannot be empty".to_string());
    }

    if !bus_address.starts_with("unix:path=/") {
        return Err(format!(
            "desktop session DBUS_SESSION_BUS_ADDRESS must use an absolute unix:path address: {bus_address}"
        ));
    }

    Ok(values)
}

fn resolve_with<F>(
    descriptor: &PlatformAuthorityDescriptor,
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
    resolver: F,
) -> Result<DesktopSessionInterface, String>
where
    F: FnOnce(
        &PlatformAuthorityDescriptor,
        &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, String>,
{
    if descriptor.authority() != DESKTOP_SESSION_INTERFACE_AUTHORITY {
        return Err(format!(
            "desktop session interface authority mismatch: expected={} descriptor={}",
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            descriptor.authority(),
        ));
    }

    if descriptor.protocol() != DESKTOP_SESSION_INTERFACE_PROTOCOL_V1 {
        return Err(format!(
            "unsupported desktop session interface protocol: {}",
            descriptor.protocol(),
        ));
    }

    let inputs = explicit_identity_inputs(desktop_uid, desktop_gid);

    let values = resolver(descriptor, &inputs)?;

    let environment = validate_desktop_session_values(values)?;
    let readonly_paths = desktop_session_readonly_paths(&environment)?;

    Ok(DesktopSessionInterface {
        desktop_uid,
        desktop_gid,
        environment,
        readonly_paths,
    })
}

pub fn resolve_desktop_session_interface_from_descriptor(
    descriptor: &PlatformAuthorityDescriptor,
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
) -> Result<DesktopSessionInterface, String> {
    resolve_with(
        descriptor,
        desktop_uid,
        desktop_gid,
        resolve_platform_capability,
    )
}

pub fn resolve_current_desktop_session_interface() -> Result<DesktopSessionInterface, String> {
    let registry = crate::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = build_authority_grant_set(registry, [DESKTOP_SESSION_INTERFACE_AUTHORITY])?;

    let descriptor_path = grants.descriptor_path(registry, DESKTOP_SESSION_INTERFACE_AUTHORITY)?;

    let descriptor =
        load_platform_authority_descriptor(&descriptor_path, DESKTOP_SESSION_INTERFACE_AUTHORITY)?;

    let (desktop_uid, desktop_gid) = crate::runtime_identity::desktop_identity()?;

    resolve_desktop_session_interface_from_descriptor(&descriptor, desktop_uid, desktop_gid)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domestic_platform_authority::test_platform_authority_descriptor;

    use std::path::PathBuf;

    fn descriptor(authority: &str, protocol: &str) -> PlatformAuthorityDescriptor {
        test_platform_authority_descriptor(
            authority.to_string(),
            PathBuf::from("/usr/lib/neebles/platform/authority/fixture.json"),
            protocol.to_string(),
            PathBuf::from("/usr/lib/neebles/platform/bin/fixture-provider"),
        )
    }

    fn valid_values() -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                XDG_RUNTIME_DIR_KEY.to_string(),
                "/run/user/1000".to_string(),
            ),
            (
                DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
                "unix:path=/run/user/1000/bus".to_string(),
            ),
        ])
    }

    #[test]
    fn family_binds_identity_to_explicit_provider_inputs() {
        let descriptor = descriptor(
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            DESKTOP_SESSION_INTERFACE_PROTOCOL_V1,
        );

        let interface = resolve_with(&descriptor, 1000, 1001, |received_descriptor, inputs| {
            assert_eq!(
                received_descriptor.authority(),
                DESKTOP_SESSION_INTERFACE_AUTHORITY,
            );

            assert_eq!(
                inputs.get("NEEBLES_DESKTOP_UID").map(String::as_str),
                Some("1000"),
            );

            assert_eq!(
                inputs.get("NEEBLES_DESKTOP_GID").map(String::as_str),
                Some("1001"),
            );

            let allowed_inputs = BTreeSet::from([
                "NEEBLES_DESKTOP_UID".to_string(),
                "NEEBLES_DESKTOP_GID".to_string(),
                DISPLAY_KEY.to_string(),
                WAYLAND_DISPLAY_KEY.to_string(),
                XAUTHORITY_KEY.to_string(),
            ]);

            let actual_inputs = inputs.keys().cloned().collect::<BTreeSet<_>>();

            assert!(
                actual_inputs.is_subset(&allowed_inputs),
                "desktop session resolver received unauthorized inputs: {actual_inputs:?}",
            );

            assert!(
                actual_inputs.contains("NEEBLES_DESKTOP_UID"),
                "desktop session resolver must receive desktop UID",
            );

            assert!(
                actual_inputs.contains("NEEBLES_DESKTOP_GID"),
                "desktop session resolver must receive desktop GID",
            );

            Ok(valid_values())
        })
        .expect("desktop session family must resolve exact capability");

        assert_eq!(interface.desktop_uid(), 1000);
        assert_eq!(interface.desktop_gid(), 1001);

        assert_eq!(interface.runtime_dir(), "/run/user/1000",);

        assert_eq!(
            interface.session_bus_address(),
            "unix:path=/run/user/1000/bus",
        );
    }

    #[test]
    fn family_rejects_wrong_authority() {
        let descriptor = descriptor("platform.other", DESKTOP_SESSION_INTERFACE_PROTOCOL_V1);

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(valid_values()))
            .expect_err("wrong authority must fail before capability resolution");

        assert!(
            error.contains("authority mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn family_rejects_wrong_protocol() {
        let descriptor = descriptor(DESKTOP_SESSION_INTERFACE_AUTHORITY, "neebles-other-v1");

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(valid_values()))
            .expect_err("wrong protocol must fail before capability resolution");

        assert!(
            error.contains("unsupported desktop session interface protocol"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn family_rejects_missing_value() {
        let descriptor = descriptor(
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            DESKTOP_SESSION_INTERFACE_PROTOCOL_V1,
        );

        let values = BTreeMap::from([(
            XDG_RUNTIME_DIR_KEY.to_string(),
            "/run/user/1000".to_string(),
        )]);

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(values))
            .expect_err("missing protocol value must fail");

        assert!(
            error.contains("missing required values"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn family_rejects_extra_value() {
        let descriptor = descriptor(
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            DESKTOP_SESSION_INTERFACE_PROTOCOL_V1,
        );

        let mut values = valid_values();

        values.insert("UNAUTHORIZED_EXTRA".to_string(), "no".to_string());

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(values))
            .expect_err("extra protocol value must fail");

        assert!(
            error.contains("unauthorized values"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn family_rejects_non_absolute_runtime_dir() {
        let descriptor = descriptor(
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            DESKTOP_SESSION_INTERFACE_PROTOCOL_V1,
        );

        let mut values = valid_values();

        values.insert(XDG_RUNTIME_DIR_KEY.to_string(), "run/user/1000".to_string());

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(values))
            .expect_err("relative runtime directory must fail");

        assert!(
            error.contains("must be absolute"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn family_rejects_non_unix_path_bus_address() {
        let descriptor = descriptor(
            DESKTOP_SESSION_INTERFACE_AUTHORITY,
            DESKTOP_SESSION_INTERFACE_PROTOCOL_V1,
        );

        let mut values = valid_values();

        values.insert(
            DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
            "tcp:host=localhost".to_string(),
        );

        let error = resolve_with(&descriptor, 1000, 1000, |_, _| Ok(values))
            .expect_err("non unix path bus address must fail");

        assert!(
            error.contains("absolute unix:path"),
            "unexpected error: {error}"
        );
    }
}

#[cfg(test)]
mod point2_session_runtime_readonly_tests {
    use super::*;

    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-session-runtime-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn fixture_interface(runtime_dir: &Path) -> DesktopSessionInterface {
        DesktopSessionInterface {
            desktop_uid: unsafe { libc::geteuid() },
            desktop_gid: unsafe { libc::getegid() },
            environment: BTreeMap::from([
                (
                    XDG_RUNTIME_DIR_KEY.to_string(),
                    runtime_dir.display().to_string(),
                ),
                (
                    DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
                    format!("unix:path={}", runtime_dir.join("bus").display()),
                ),
            ]),
            readonly_paths: Vec::new(),
        }
    }

    #[test]
    fn authority_grants_owned_runtime_socket_only_by_relative_path() {
        let base = fixture_root("socket");
        let neebles = base.join("neebles");
        let socket = neebles.join("tray.sock");

        fs::create_dir_all(&neebles).expect("fixture runtime directory must exist");

        let listener = UnixListener::bind(&socket).expect("fixture tray socket must bind");

        let interface = fixture_interface(&base);

        let grant = interface
            .grant_runtime_readonly(Path::new("neebles/tray.sock"))
            .expect("owned session socket must be granted");

        assert_eq!(grant.source, fs::canonicalize(&socket).unwrap(),);

        assert_eq!(grant.destination, grant.source,);

        drop(listener);

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn authority_rejects_escape_directory_and_symlink() {
        let base = fixture_root("reject");
        let neebles = base.join("neebles");
        let socket = neebles.join("tray.sock");
        let link = neebles.join("tray-link.sock");

        fs::create_dir_all(&neebles).expect("fixture runtime directory must exist");

        let listener = UnixListener::bind(&socket).expect("fixture tray socket must bind");

        symlink(&socket, &link).expect("fixture symlink must exist");

        let interface = fixture_interface(&base);

        assert!(
            interface
                .grant_runtime_readonly(Path::new("../outside"),)
                .is_err(),
            "parent traversal must fail"
        );

        assert!(
            interface
                .grant_runtime_readonly(Path::new("neebles"),)
                .is_err(),
            "directory exposure must fail"
        );

        assert!(
            interface
                .grant_runtime_readonly(Path::new("neebles/tray-link.sock"),)
                .is_err(),
            "symlink exposure must fail"
        );

        drop(listener);

        fs::remove_dir_all(base).expect("fixture must clean");
    }
}
