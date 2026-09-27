use serde_json::Value;

use std::fs;

use std::path::{Path, PathBuf};

pub const MATERIALIZED_RUNTIME_SCHEMA: &str = "1";

pub const MATERIALIZED_RUNTIME_NAME: &str = "neebles-domestic-runtime";

fn load_materialized_runtime_context(manifest_path: &Path) -> Result<(Value, PathBuf), String> {
    if !manifest_path.is_absolute() {
        return Err(format!(
            "materialized runtime manifest path must be absolute: {}",
            manifest_path.display()
        ));
    }

    let raw = fs::read_to_string(manifest_path).map_err(|error| {
        format!(
            "could not read materialized runtime manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid materialized runtime manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    if manifest.get("schema").and_then(Value::as_str) != Some(MATERIALIZED_RUNTIME_SCHEMA) {
        return Err("unsupported materialized runtime schema".to_string());
    }

    if manifest.get("name").and_then(Value::as_str) != Some(MATERIALIZED_RUNTIME_NAME) {
        return Err("invalid materialized runtime identity".to_string());
    }

    let root_value = manifest
        .get("root")
        .and_then(Value::as_str)
        .ok_or_else(|| "materialized runtime root is missing".to_string())?;

    if root_value.trim().is_empty() {
        return Err("materialized runtime root is empty".to_string());
    }

    let root_reference = PathBuf::from(root_value);

    if root_reference.is_absolute() {
        return Err(format!(
            "materialized runtime root must be relative: {}",
            root_reference.display()
        ));
    }

    let manifest_directory = manifest_path.parent().ok_or_else(|| {
        format!(
            "materialized runtime manifest has no parent: {}",
            manifest_path.display()
        )
    })?;

    let runtime_root = crate::domestic_world::resolve_world_reference(
        manifest_directory,
        manifest_directory,
        &root_reference,
    )
    .map_err(|error| format!("materialized runtime root is outside authority: {error}"))?;

    if !runtime_root.is_dir() {
        return Err(format!(
            "materialized runtime root is not a directory: {}",
            runtime_root.display()
        ));
    }

    Ok((manifest, runtime_root))
}

pub fn resolve_materialized_runtime_root(manifest_path: &Path) -> Result<PathBuf, String> {
    let (_, runtime_root) = load_materialized_runtime_context(manifest_path)?;

    Ok(runtime_root)
}

pub fn resolve_materialized_runtime_targets(
    manifest_path: &Path,
    world_name: &str,
    category_name: &str,
) -> Result<Vec<PathBuf>, String> {
    let (manifest, runtime_root) = load_materialized_runtime_context(manifest_path)?;

    if world_name.trim().is_empty() {
        return Err("materialized runtime world name is empty".to_string());
    }

    if category_name.trim().is_empty() {
        return Err("materialized runtime category name is empty".to_string());
    }

    let worlds = manifest
        .get("worlds")
        .and_then(Value::as_object)
        .ok_or_else(|| "materialized runtime worlds is not an object".to_string())?;

    let world = worlds
        .get(world_name)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("unknown materialized runtime world: {world_name}"))?;

    let categories = world
        .get("categories")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("materialized runtime world {world_name} has no categories"))?;

    let category = categories
        .get(category_name)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!("materialized runtime world {world_name} has no category {category_name}")
        })?;

    if category.get("category").and_then(Value::as_str) != Some(category_name) {
        return Err(format!(
            "materialized runtime category identity mismatch for {world_name}:{category_name}"
        ));
    }

    let targets = category
        .get("resolved_targets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "materialized runtime category {world_name}:{category_name} has no resolved_targets array"
            )
        })?;

    if targets.is_empty() {
        return Err(format!(
            "materialized runtime category {world_name}:{category_name} has no resolved targets"
        ));
    }

    let mut resolved = Vec::<PathBuf>::with_capacity(targets.len());

    for (index, target_value) in targets.iter().enumerate() {
        let target_value = target_value.as_str().ok_or_else(|| {
            format!(
                "materialized runtime target {index} for {world_name}:{category_name} is not a string"
            )
        })?;

        if target_value.trim().is_empty() {
            return Err(format!(
                "materialized runtime target {index} for {world_name}:{category_name} is empty"
            ));
        }

        let target_reference = PathBuf::from(target_value);

        if target_reference.is_absolute() {
            return Err(format!(
                "materialized runtime target must be relative for {world_name}:{category_name}: {}",
                target_reference.display()
            ));
        }

        let target =
            crate::domestic_world::resolve_world_reference(
                &runtime_root,
                &runtime_root,
                &target_reference,
            )
            .map_err(|error| {
                format!(
                    "materialized runtime target is outside authority for {world_name}:{category_name}: {error}"
                )
            })?;

        if category_name == "executable" && !target.is_file() {
            return Err(format!(
                "materialized runtime executable target is not a file for {world_name}: {}",
                target.display()
            ));
        }

        resolved.push(target);
    }

    Ok(resolved)
}

pub fn resolve_materialized_runtime_target(
    manifest_path: &Path,
    world_name: &str,
    category_name: &str,
) -> Result<PathBuf, String> {
    let mut targets =
        resolve_materialized_runtime_targets(manifest_path, world_name, category_name)?;

    if targets.len() != 1 {
        return Err(format!(
            "materialized runtime category {world_name}:{category_name} requires exactly one target, found {}",
            targets.len()
        ));
    }

    Ok(targets.remove(0))
}

fn validate_boss_world_name(world_name: &str) -> Result<(), String> {
    let suffix = world_name.strip_prefix("boss.").ok_or_else(|| {
        format!("Boss runtime authority requires a boss-qualified world: {world_name}")
    })?;

    if suffix.trim().is_empty() {
        return Err("Boss runtime authority world suffix is empty".to_string());
    }

    Ok(())
}

pub fn boss_runtime_manifest_for_executable(executable: &Path) -> Result<PathBuf, String> {
    if !executable.is_absolute() {
        return Err(format!(
            "Boss executable path must be absolute: {}",
            executable.display()
        ));
    }

    let backend_directory = executable.parent().ok_or_else(|| {
        format!(
            "Boss executable has no backend directory: {}",
            executable.display()
        )
    })?;

    let client_root = backend_directory.parent().ok_or_else(|| {
        format!(
            "Boss executable has no client root: {}",
            executable.display()
        )
    })?;

    Ok(client_root
        .join("runtime")
        .join("boss")
        .join("domestic-runtime.json"))
}

pub fn current_boss_runtime_manifest() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not determine current Boss executable: {error}"))?;

    boss_runtime_manifest_for_executable(&executable)
}

pub fn resolve_boss_runtime_target(
    world_name: &str,
    category_name: &str,
) -> Result<PathBuf, String> {
    validate_boss_world_name(world_name)?;

    let manifest = current_boss_runtime_manifest()?;

    resolve_materialized_runtime_target(&manifest, world_name, category_name)
}

pub fn resolve_boss_executable(world_name: &str) -> Result<PathBuf, String> {
    resolve_boss_runtime_target(world_name, "executable")
}

#[cfg(test)]
mod installed_authority_tests {
    use super::*;

    #[test]
    fn boss_manifest_location_is_relative_to_client_layout() {
        let executable = Path::new("/example/neebles/client/backend/neebles-backend");

        let manifest = boss_runtime_manifest_for_executable(executable)
            .expect("Boss runtime manifest layout must resolve");

        assert_eq!(
            manifest,
            PathBuf::from("/example/neebles/client/runtime/boss/domestic-runtime.json")
        );
    }

    #[test]
    fn boss_runtime_rejects_foreign_world_identity() {
        assert!(validate_boss_world_name("modules.git").is_err());

        assert!(validate_boss_world_name("boss.").is_err());

        assert!(validate_boss_world_name("boss.git").is_ok());
    }

    #[test]
    fn plural_runtime_targets_preserve_order_and_singular_contract() {
        let base = std::env::temp_dir().join(format!(
            "neebles-runtime-authority-plural-{}",
            std::process::id()
        ));

        if base.exists() {
            std::fs::remove_dir_all(&base).expect("old plural runtime fixture must be removable");
        }

        let runtime_root = base.join("rootfs");

        let multiarch = runtime_root.join("usr/lib/x86_64-linux-gnu");

        let systemd = multiarch.join("systemd");

        let executable = runtime_root.join("usr/bin/tool");

        std::fs::create_dir_all(&systemd).expect("plural runtime directories must be created");

        std::fs::create_dir_all(
            executable
                .parent()
                .expect("fixture executable must have parent"),
        )
        .expect("fixture executable parent must be created");

        std::fs::write(&executable, b"fixture").expect("fixture executable must be written");

        let manifest = base.join("domestic-runtime.json");

        let value = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-runtime",
            "root": "rootfs",
            "worlds": {
                "boss.fixture": {
                    "categories": {
                        "executable": {
                            "category": "executable",
                            "value": "fixture",
                            "declared_targets": [
                                "usr/bin/tool"
                            ],
                            "resolved_targets": [
                                "usr/bin/tool"
                            ]
                        },
                        "library_paths": {
                            "category": "library_paths",
                            "value": "fixture-libraries",
                            "declared_targets": [
                                "usr/lib/x86_64-linux-gnu",
                                "usr/lib/x86_64-linux-gnu/systemd"
                            ],
                            "resolved_targets": [
                                "usr/lib/x86_64-linux-gnu",
                                "usr/lib/x86_64-linux-gnu/systemd"
                            ]
                        }
                    }
                }
            }
        });

        std::fs::write(
            &manifest,
            serde_json::to_vec_pretty(&value).expect("fixture manifest must serialize"),
        )
        .expect("fixture manifest must be written");

        let plural =
            resolve_materialized_runtime_targets(&manifest, "boss.fixture", "library_paths")
                .expect("plural runtime category must resolve");

        assert_eq!(plural, vec![multiarch.clone(), systemd.clone(),]);

        let singular_error =
            resolve_materialized_runtime_target(&manifest, "boss.fixture", "library_paths")
                .expect_err("singular runtime resolver must reject plural category");

        assert!(singular_error.contains("requires exactly one target, found 2"));

        assert_eq!(
            resolve_materialized_runtime_target(&manifest, "boss.fixture", "executable",)
                .expect("singular executable must still resolve"),
            executable
        );

        std::fs::remove_dir_all(&base).expect("plural runtime fixture must be removable");
    }
}
