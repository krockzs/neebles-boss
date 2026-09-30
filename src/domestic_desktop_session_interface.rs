use std::collections::{BTreeMap, BTreeSet};

use crate::domestic_authority_supply::build_authority_grant_set;
use crate::domestic_capability_provider::resolve_platform_capability;
use crate::domestic_platform_authority::{
    load_platform_authority_descriptor, PlatformAuthorityDescriptor,
};

pub const DESKTOP_SESSION_INTERFACE_AUTHORITY: &str = "platform.desktop_session_interface";

pub const DESKTOP_SESSION_INTERFACE_PROTOCOL_V1: &str = "neebles-desktop-session-interface-v1";

pub const XDG_RUNTIME_DIR_KEY: &str = "XDG_RUNTIME_DIR";

pub const DBUS_SESSION_BUS_ADDRESS_KEY: &str = "DBUS_SESSION_BUS_ADDRESS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSessionInterface {
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
    environment: BTreeMap<String, String>,
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
}

fn explicit_identity_inputs(
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("NEEBLES_DESKTOP_UID".to_string(), desktop_uid.to_string()),
        ("NEEBLES_DESKTOP_GID".to_string(), desktop_gid.to_string()),
    ])
}

fn validate_desktop_session_values(
    values: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let expected = BTreeSet::from([
        XDG_RUNTIME_DIR_KEY.to_string(),
        DBUS_SESSION_BUS_ADDRESS_KEY.to_string(),
    ]);

    let actual = values.keys().cloned().collect::<BTreeSet<_>>();

    if actual != expected {
        return Err(format!(
            "desktop session interface values mismatch: expected={expected:?} actual={actual:?}"
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

    Ok(DesktopSessionInterface {
        desktop_uid,
        desktop_gid,
        environment,
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

            assert_eq!(inputs.len(), 2);

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
            error.contains("values mismatch"),
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
            error.contains("values mismatch"),
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
