use neebles_backend::domestic_resolver::resolve_world;
use neebles_backend::domestic_test::{
    execute_tester_matrix, load_categories, test_category, validate_tester_definition,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn domestic_root() -> PathBuf {
    PathBuf::from(
        std::env::var("NEEBLES_DOMESTIC_ROOT")
            .expect("NEEBLES_DOMESTIC_ROOT must point to the domestic root under certification"),
    )
}

fn read_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));

    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("invalid JSON {}: {error}", path.display()))
}

fn temporary_root() -> PathBuf {
    let unique = format!(
        "neebles-domestic-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be valid")
            .as_nanos()
    );

    let path = std::env::temp_dir().join(unique);

    fs::create_dir_all(&path).expect("temporary domestic test directory must be created");

    path
}

fn contract() -> PathBuf {
    root().join("src/contracts/domesticacion-elfica.schema.json")
}

fn grimoire() -> PathBuf {
    root().join("src/contracts/domesticacion-elfica.grimorio.json")
}

fn target_catalog() -> PathBuf {
    root().join("src/contracts/domesticacion-elfica.targets.json")
}

#[test]
fn domestic_contract_is_valid() {
    let categories = load_categories(&contract()).expect("domestic contract must be valid");

    assert_eq!(categories.len(), 7);
}

#[test]
fn grimoire_format_is_valid() {
    let value = read_json(&grimoire());

    assert_eq!(value.get("schema").and_then(Value::as_str), Some("1"));

    assert_eq!(
        value.get("name").and_then(Value::as_str),
        Some("domesticacion-elfica")
    );

    let worlds = value
        .get("worlds")
        .and_then(Value::as_object)
        .expect("worlds must be object");

    assert!(!worlds.is_empty());
}

#[test]
fn target_catalog_is_canonical() {
    let value = read_json(&target_catalog());

    assert_eq!(value.get("schema").and_then(Value::as_str), Some("1"));

    assert_eq!(
        value.get("name").and_then(Value::as_str),
        Some("domesticacion-elfica-targets")
    );

    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .expect("canonical target catalog entries must be array");

    assert!(
        !entries.is_empty(),
        "canonical target catalog cannot be empty"
    );

    for entry in entries {
        let object = entry.as_object().expect("target entry must be object");

        let allowed = BTreeSet::from(["category", "value", "targets"]);

        let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();

        assert_eq!(actual, allowed, "target entry contains unknown fields");

        assert!(object
            .get("category")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()));

        assert!(object
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()));

        let targets = object
            .get("targets")
            .and_then(Value::as_array)
            .expect("targets must be array");

        assert!(!targets.is_empty());

        for target in targets {
            let target = target.as_str().expect("target must be String");

            assert!(!target.trim().is_empty());

            assert!(
                !Path::new(target).is_absolute(),
                "canonical target must remain relative: {target}"
            );

            assert!(
                !target.split('/').any(|component| component == ".."),
                "canonical target cannot escape domestic root: {target}"
            );
        }
    }
}

#[test]
fn physical_categories_follow_contract() {
    let expected = load_categories(&contract()).expect("domestic contract must load");

    let categories_root = root().join("testings/domesticacion/categories");

    let physical = fs::read_dir(&categories_root)
        .expect("category directory must exist")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(physical, expected);

    for category in expected {
        let definition = read_json(&categories_root.join(&category).join("tester.json"));

        assert_eq!(
            definition.get("category").and_then(Value::as_str),
            Some(category.as_str())
        );

        assert_eq!(
            definition
                .get("technology_specific")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
}

#[test]
fn every_category_accepts_string_and_targets() {
    let categories = load_categories(&contract()).expect("domestic contract must load");

    let temp = temporary_root();

    fs::write(temp.join("alpha"), b"alpha\n").expect("temporary file must exist");

    fs::create_dir_all(temp.join("beta")).expect("temporary directory must exist");

    for category in categories {
        let report = test_category(
            &contract(),
            &temp,
            &category,
            "example-string",
            &[PathBuf::from("alpha"), PathBuf::from("beta")],
        )
        .expect("generic category test must execute");

        assert!(report.ok);
    }

    fs::remove_dir_all(temp).expect("temporary directory must be removed");
}

#[test]
fn missing_target_fails() {
    let temp = temporary_root();

    let report = test_category(
        &contract(),
        &temp,
        "executable",
        "example-string",
        &[PathBuf::from("missing")],
    )
    .expect("missing target is valid test request");

    assert!(!report.ok);

    fs::remove_dir_all(temp).expect("temporary directory must be removed");
}

#[test]
fn unknown_category_is_rejected() {
    let temp = temporary_root();

    fs::write(temp.join("target"), b"x").expect("temporary target must exist");

    let error = test_category(
        &contract(),
        &temp,
        "future_unknown",
        "value",
        &[PathBuf::from("target")],
    )
    .expect_err("unknown category must fail");

    assert!(error.contains("unknown domestic category"));

    fs::remove_dir_all(temp).expect("temporary directory must be removed");
}

#[test]
fn target_catalog_matches_grimoire() {
    let grimoire_value = read_json(&grimoire());

    let target_value = read_json(&target_catalog());

    let worlds = grimoire_value
        .get("worlds")
        .and_then(Value::as_object)
        .expect("worlds must be object");

    let entries = target_value
        .get("entries")
        .and_then(Value::as_array)
        .expect("target entries must be array");

    let mut declared = BTreeSet::<String>::new();

    for world in worlds.values() {
        let object = world.as_object().expect("world must be object");

        for (category, logical_value) in object {
            declared.insert(format!(
                "{}:{}",
                category,
                logical_value
                    .as_str()
                    .expect("domestic value must be String")
            ));
        }
    }

    let mut catalog = BTreeSet::<String>::new();

    for entry in entries {
        let category = entry
            .get("category")
            .and_then(Value::as_str)
            .expect("target category must be String");

        let logical_value = entry
            .get("value")
            .and_then(Value::as_str)
            .expect("target value must be String");

        catalog.insert(format!("{category}:{logical_value}"));
    }

    assert_eq!(
        catalog, declared,
        "target catalog and grimoire declarations must remain identical"
    );
}

#[test]
#[ignore = "requires explicit domestic root certification"]
fn resolver_resolves_every_real_world() {
    let value = read_json(&grimoire());

    let worlds = value
        .get("worlds")
        .and_then(Value::as_object)
        .expect("worlds must be object");

    let home = domestic_root();

    for world_name in worlds.keys() {
        let resolved = resolve_world(
            &contract(),
            &grimoire(),
            &target_catalog(),
            &home,
            world_name,
        )
        .unwrap_or_else(|error| panic!("world {world_name} failed resolution: {error}"));

        assert_eq!(resolved.world, world_name.as_str());

        assert!(!resolved.categories.is_empty());

        for category in resolved.categories.values() {
            assert!(!category.resolved_targets.is_empty());

            for target in &category.resolved_targets {
                assert!(
                    target.exists(),
                    "resolved target does not exist: {}",
                    target.display()
                );
            }
        }
    }
}

#[test]
fn resolver_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let base = temporary_root();

    let world_root = base.join("world");

    let outside = base.join("outside");

    fs::create_dir_all(world_root.join("usr/bin"))
        .expect("world executable directory must be created");

    fs::create_dir_all(&outside).expect("outside directory must be created");

    fs::write(outside.join("real-tool"), b"outside\n").expect("outside target must be created");

    symlink(
        "../../../outside/real-tool",
        world_root.join("usr/bin/escape-tool"),
    )
    .expect("escaping symlink must be created");

    let fixture_grimoire = base.join("grimorio.json");

    let fixture_targets = base.join("targets.json");

    fs::write(
        &fixture_grimoire,
        r#"{
  "schema": "1",
  "name": "domesticacion-elfica",
  "worlds": {
    "boss.escape": {
      "executable": "escape-tool"
    }
  }
}
"#,
    )
    .expect("fixture grimoire must be written");

    fs::write(
        &fixture_targets,
        r#"{
  "schema": "1",
  "name": "domesticacion-elfica-targets",
  "entries": [
    {
      "category": "executable",
      "value": "escape-tool",
      "targets": [
        "usr/bin/escape-tool"
      ]
    }
  ]
}
"#,
    )
    .expect("fixture target catalog must be written");

    let result = resolve_world(
        &contract(),
        &fixture_grimoire,
        &fixture_targets,
        &world_root,
        "boss.escape",
    );

    assert!(
        result.is_err(),
        "domestic resolver must reject a declared target whose resolved symlink escapes the declared world"
    );

    fs::remove_dir_all(base).expect("temporary resolver fixture must be removed");
}

#[test]
fn resolver_rejects_unknown_world() {
    let temp = temporary_root();

    let error = resolve_world(
        &contract(),
        &grimoire(),
        &target_catalog(),
        &temp,
        "boss.this-world-does-not-exist",
    )
    .expect_err("unknown world must fail");

    assert!(error.contains("unknown domestic world"));

    fs::remove_dir_all(temp).expect("temporary directory must be removed");
}

#[test]
fn domestic_testing_tree_requires_no_python() {
    let testing_root = root().join("testings/domesticacion");

    let mut pending = vec![testing_root];

    let mut python = Vec::<PathBuf>::new();

    let mut pycache = Vec::<PathBuf>::new();

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).expect("testing tree must be readable") {
            let entry = entry.expect("testing entry must be readable");

            let child = entry.path();

            if child.is_dir() {
                if child.file_name().and_then(|value| value.to_str()) == Some("__pycache__") {
                    pycache.push(child.clone());
                }

                pending.push(child);

                continue;
            }

            if child.extension().and_then(|value| value.to_str()) == Some("py") {
                python.push(child);
            }
        }
    }

    assert!(
        python.is_empty(),
        "Python files remain in domestic testing tree: {python:?}"
    );

    assert!(
        pycache.is_empty(),
        "Python cache remains in domestic testing tree: {pycache:?}"
    );
}

#[test]
fn category_testers_are_ordered_spell_matrices() {
    let categories = load_categories(&contract()).expect("domestic contract must load");

    let categories_root = root().join("testings/domesticacion/categories");

    for category in categories {
        let path = categories_root.join(&category).join("tester.json");

        let cases = validate_tester_definition(&path, &category)
            .unwrap_or_else(|error| panic!("tester matrix {category} is invalid: {error}"));

        assert!(cases > 0, "tester matrix {category} must contain cases");
    }
}

#[test]
fn every_declared_matrix_case_executes() {
    let categories = load_categories(&contract()).expect("domestic contract must load");

    let categories_root = root().join("testings/domesticacion/categories");

    let mut total = 0usize;

    for category in categories {
        let tester = categories_root.join(&category).join("tester.json");

        let report = execute_tester_matrix(&contract(), &tester, &category)
            .unwrap_or_else(|error| panic!("matrix execution failed for {category}: {error}"));

        assert!(report.executed > 0);

        total += report.executed;
    }

    assert!(
        total > 0,
        "matrix corpus must contain at least one declared combination"
    );
}
