use crate::domestic_environment::{build_process_command, ProcessEnvironmentClass};

use crate::domestic_platform_authority::PlatformAuthorityDescriptor;

use serde::Deserialize;

use std::collections::{BTreeMap, BTreeSet};

pub const CAPABILITY_PROVIDER_OUTPUT_SCHEMA: &str = "1";

pub const CAPABILITY_PROVIDER_OUTPUT_NAME: &str = "neebles-capability-provider-output";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityProviderOutput {
    schema: String,
    name: String,
    authority: String,
    protocol: String,
    values: BTreeMap<String, String>,
}

pub fn parse_capability_provider_output(
    raw: &[u8],
    expected_authority: &str,
    expected_protocol: &str,
) -> Result<BTreeMap<String, String>, String> {
    if expected_authority.trim().is_empty() {
        return Err("expected capability provider authority cannot be empty".to_string());
    }

    if expected_protocol.trim().is_empty() {
        return Err("expected capability provider protocol cannot be empty".to_string());
    }

    let output: CapabilityProviderOutput = serde_json::from_slice(raw)
        .map_err(|error| format!("invalid capability provider output JSON: {error}"))?;

    if output.schema != CAPABILITY_PROVIDER_OUTPUT_SCHEMA {
        return Err("capability provider output schema must be String 1".to_string());
    }

    if output.name != CAPABILITY_PROVIDER_OUTPUT_NAME {
        return Err("invalid capability provider output name".to_string());
    }

    let authority = output.authority.trim();

    if authority.is_empty() {
        return Err("capability provider output authority cannot be empty".to_string());
    }

    if authority != expected_authority {
        return Err(format!(
            "capability provider authority mismatch: expected={expected_authority} output={authority}"
        ));
    }

    let protocol = output.protocol.trim();

    if protocol.is_empty() {
        return Err("capability provider output protocol cannot be empty".to_string());
    }

    if protocol != expected_protocol {
        return Err(format!(
            "capability provider protocol mismatch: expected={expected_protocol} output={protocol}"
        ));
    }

    for key in output.values.keys() {
        if key.trim().is_empty() {
            return Err("capability provider output contains empty value key".to_string());
        }
    }

    Ok(output.values)
}

pub fn resolve_platform_capability(
    descriptor: &PlatformAuthorityDescriptor,
    inputs: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut command = build_process_command(
        descriptor.provider(),
        ProcessEnvironmentClass::Pure,
        inputs,
        &BTreeMap::new(),
        &BTreeSet::new(),
    )?;

    let output = command.output().map_err(|error| {
        format!(
            "could not execute capability provider {}: {error}",
            descriptor.provider().display()
        )
    })?;

    if !output.status.success() {
        return Err(format!(
            "capability provider {} failed with status {}",
            descriptor.provider().display(),
            output.status
        ));
    }

    parse_capability_provider_output(
        &output.stdout,
        descriptor.authority(),
        descriptor.protocol(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domestic_platform_authority::test_platform_authority_descriptor;

    use std::fs;

    use std::os::unix::fs::PermissionsExt;

    use std::path::PathBuf;

    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-capability-provider-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn write_provider(path: &std::path::Path, body: &str) {
        fs::write(path, body).expect("provider fixture must exist");

        let mut permissions = fs::metadata(path)
            .expect("provider fixture metadata must exist")
            .permissions();

        permissions.set_mode(0o700);

        fs::set_permissions(path, permissions).expect("provider fixture permissions must apply");
    }

    fn descriptor(provider: PathBuf) -> PlatformAuthorityDescriptor {
        test_platform_authority_descriptor(
            "platform.example".to_string(),
            PathBuf::from("/authority/example.json"),
            "neebles-example-v1".to_string(),
            provider,
        )
    }

    #[test]
    fn parser_accepts_exact_output() {
        let values = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.example",
                "protocol": "neebles-example-v1",
                "values": {
                    "EXAMPLE_ALPHA": "one",
                    "EXAMPLE_BETA": "two"
                }
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect("exact capability output must parse");

        assert_eq!(values.get("EXAMPLE_ALPHA").map(String::as_str), Some("one"),);

        assert_eq!(values.get("EXAMPLE_BETA").map(String::as_str), Some("two"),);
    }

    #[test]
    fn parser_accepts_empty_values() {
        let values = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.example",
                "protocol": "neebles-example-v1",
                "values": {}
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect("empty capability values must remain valid");

        assert!(values.is_empty());
    }

    #[test]
    fn parser_rejects_unknown_field() {
        let error = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.example",
                "protocol": "neebles-example-v1",
                "values": {},
                "magic": true
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect_err("unknown fields must fail");

        assert!(error.contains("unknown field"), "unexpected error: {error}");
    }

    #[test]
    fn parser_rejects_authority_mismatch() {
        let error = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.other",
                "protocol": "neebles-example-v1",
                "values": {}
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect_err("authority mismatch must fail");

        assert!(
            error.contains("authority mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parser_rejects_protocol_mismatch() {
        let error = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.example",
                "protocol": "neebles-other-v1",
                "values": {}
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect_err("protocol mismatch must fail");

        assert!(
            error.contains("protocol mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parser_rejects_empty_value_key() {
        let error = parse_capability_provider_output(
            br#"{
                "schema": "1",
                "name": "neebles-capability-provider-output",
                "authority": "platform.example",
                "protocol": "neebles-example-v1",
                "values": {
                    "": "invalid"
                }
            }"#,
            "platform.example",
            "neebles-example-v1",
        )
        .expect_err("empty value keys must fail");

        assert!(
            error.contains("empty value key"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn provider_executes_with_sealed_explicit_inputs() {
        let root = fixture_root("sealed-input");

        fs::create_dir_all(&root).expect("fixture root must exist");

        let provider = root.join("provider");

        write_provider(
            &provider,
            r#"#!/bin/sh
printf '%s\n' '{"schema":"1","name":"neebles-capability-provider-output","authority":"platform.example","protocol":"neebles-example-v1","values":{"RESULT":"resolved"}}'
"#,
        );

        let descriptor = descriptor(provider);

        let values = resolve_platform_capability(
            &descriptor,
            &BTreeMap::from([("NEEBLES_EXPLICIT_INPUT".to_string(), "explicit".to_string())]),
        )
        .expect("provider must resolve");

        assert_eq!(values.get("RESULT").map(String::as_str), Some("resolved"),);

        fs::remove_dir_all(&root).expect("fixture must clean");
    }

    #[test]
    fn provider_nonzero_exit_is_error() {
        let root = fixture_root("nonzero");

        fs::create_dir_all(&root).expect("fixture root must exist");

        let provider = root.join("provider");

        write_provider(
            &provider,
            r#"#!/bin/sh
exit 9
"#,
        );

        let descriptor = descriptor(provider);

        let error = resolve_platform_capability(&descriptor, &BTreeMap::new())
            .expect_err("nonzero provider must fail");

        assert!(
            error.contains("failed with status"),
            "unexpected error: {error}"
        );

        fs::remove_dir_all(&root).expect("fixture must clean");
    }
}
