use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageSelector {
    filename: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequirement {
    pub filename: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct MaterialManifest {
    module: String,
    version: String,
    entries: Vec<MaterialManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialLayerManifest {
    entries: Vec<MaterialManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct MaterialManifestEntry {
    path: String,

    #[serde(rename = "type")]
    kind: String,

    mode: String,

    #[serde(default)]
    size: Option<u64>,

    #[serde(default)]
    sha256: Option<String>,

    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialEntry {
    pub path: String,
    pub kind: String,
    pub mode: u32,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialRecipe {
    pub module: String,
    pub version: String,
    pub packages: Vec<PackageRequirement>,
    pub entries: Vec<MaterialEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialLayer {
    pub packages: Vec<PackageRequirement>,
    pub entries: Vec<MaterialEntry>,
}

pub fn valid_module_id(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|value| value.is_ascii_hexdigit())
}

fn valid_single_filename(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." {
        return false;
    }

    let path = Path::new(value);

    !path.is_absolute()
        && path.components().count() == 1
        && path.file_name().and_then(|name| name.to_str()) == Some(value)
}

fn parse_mode(value: &str) -> Result<u32, String> {
    let digits = value
        .strip_prefix("0o")
        .ok_or_else(|| format!("material mode must use 0o notation: {value}"))?;

    let mode = u32::from_str_radix(digits, 8)
        .map_err(|error| format!("invalid material mode '{value}': {error}"))?;

    if mode > 0o7777 {
        return Err(format!("material mode is outside supported range: {value}"));
    }

    Ok(mode)
}

fn parse_packages_tsv(payload: &str) -> Result<BTreeMap<String, PackageSelector>, String> {
    let mut packages = BTreeMap::new();

    for (index, raw) in payload.lines().enumerate() {
        let line = raw.trim();

        if line.is_empty() {
            continue;
        }

        let fields = line.split('\t').collect::<Vec<_>>();

        if fields.len() != 5 {
            return Err(format!(
                "invalid module package selector line {}",
                index + 1
            ));
        }

        let package = fields[0].trim();
        let version = fields[1].trim();
        let arch = fields[2].trim();
        let filename = fields[3].trim();
        let sha256 = fields[4].trim();

        if package.is_empty() || version.is_empty() || arch.is_empty() {
            return Err(format!(
                "incomplete module package selector line {}",
                index + 1
            ));
        }

        if !valid_single_filename(filename) {
            return Err(format!("unsafe module package filename: {filename}"));
        }

        if !valid_sha256(sha256) {
            return Err(format!("invalid module package sha256 for {filename}"));
        }

        let selector = PackageSelector {
            filename: filename.to_string(),
            sha256: sha256.to_ascii_lowercase(),
        };

        if packages.insert(filename.to_string(), selector).is_some() {
            return Err(format!("duplicate module package filename: {filename}"));
        }
    }

    Ok(packages)
}

fn valid_material_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let path = Path::new(value);

    if path.is_absolute() {
        return false;
    }

    path.components().all(|component| {
        !matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::CurDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    })
}

fn parse_material_entries(
    entries: &[MaterialManifestEntry],
    authority: &str,
) -> Result<(Vec<MaterialEntry>, BTreeMap<String, usize>), String> {
    let mut material_entries = Vec::<MaterialEntry>::with_capacity(entries.len());
    let mut package_entries = BTreeMap::<String, usize>::new();

    for (index, entry) in entries.iter().enumerate() {
        if !valid_material_path(&entry.path) {
            return Err(format!("unsafe {authority} material path: {}", entry.path));
        }

        let mode = parse_mode(&entry.mode)?;

        let sha256 = match entry.sha256.as_deref() {
            Some(value) => {
                if !valid_sha256(value) {
                    return Err(format!(
                        "invalid {authority} material sha256: {}",
                        entry.path
                    ));
                }

                Some(value.to_ascii_lowercase())
            }

            None => None,
        };

        match entry.kind.as_str() {
            "file" => {
                if entry.size.is_none() {
                    return Err(format!(
                        "{authority} material file size missing: {}",
                        entry.path
                    ));
                }

                if sha256.is_none() {
                    return Err(format!(
                        "{authority} material file sha256 missing: {}",
                        entry.path
                    ));
                }

                if entry.target.is_some() {
                    return Err(format!(
                        "{authority} material file cannot declare symlink target: {}",
                        entry.path
                    ));
                }
            }

            "directory" | "dir" => {
                if entry.target.is_some() {
                    return Err(format!(
                        "{authority} material directory cannot declare symlink target: {}",
                        entry.path
                    ));
                }
            }

            "symlink" => {
                if entry.target.as_deref().unwrap_or("").is_empty() {
                    return Err(format!(
                        "{authority} material symlink target missing: {}",
                        entry.path
                    ));
                }
            }

            other => {
                return Err(format!(
                    "unsupported {authority} material type {other}: {}",
                    entry.path
                ));
            }
        }

        if let Some(filename) = entry.path.strip_prefix("packages/") {
            if !valid_single_filename(filename) {
                return Err(format!(
                    "unsafe package path in {authority} material manifest: {}",
                    entry.path
                ));
            }

            if package_entries
                .insert(filename.to_string(), index)
                .is_some()
            {
                return Err(format!(
                    "duplicate package entry in {authority} material manifest: {filename}"
                ));
            }
        }

        material_entries.push(MaterialEntry {
            path: entry.path.clone(),
            kind: entry.kind.clone(),
            mode,
            size: entry.size,
            sha256,
            target: entry.target.clone(),
        });
    }

    Ok((material_entries, package_entries))
}

fn validate_package_authority(
    selectors: &BTreeMap<String, PackageSelector>,
    entries: &[MaterialManifestEntry],
    package_entries: &BTreeMap<String, usize>,
    authority: &str,
) -> Result<Vec<PackageRequirement>, String> {
    let declared = selectors.keys().cloned().collect::<Vec<_>>();
    let manifested = package_entries.keys().cloned().collect::<Vec<_>>();

    if declared != manifested {
        return Err(format!(
            "{authority} package membership mismatch: selector={declared:?} manifest={manifested:?}"
        ));
    }

    let mut packages = Vec::with_capacity(selectors.len());

    for selector in selectors.values() {
        let index = package_entries.get(&selector.filename).ok_or_else(|| {
            format!(
                "{authority} material manifest is missing package {}",
                selector.filename
            )
        })?;

        let entry = &entries[*index];

        if entry.kind != "file" {
            return Err(format!(
                "{authority} package entry is not a file: {}",
                selector.filename
            ));
        }

        let manifest_sha = entry
            .sha256
            .as_deref()
            .ok_or_else(|| {
                format!(
                    "{authority} package manifest sha256 missing: {}",
                    selector.filename
                )
            })?
            .to_ascii_lowercase();

        if manifest_sha != selector.sha256 {
            return Err(format!(
                "{authority} package sha256 authority mismatch: {}",
                selector.filename
            ));
        }

        packages.push(PackageRequirement {
            filename: selector.filename.clone(),
            sha256: selector.sha256.clone(),
        });
    }

    Ok(packages)
}

pub fn parse_material_layer(
    authority: &str,
    packages_payload: &str,
    manifest_payload: &[u8],
) -> Result<MaterialLayer, String> {
    if authority.trim().is_empty() {
        return Err("material authority label cannot be empty".to_string());
    }

    let selectors = parse_packages_tsv(packages_payload)?;

    let manifest: MaterialLayerManifest = serde_json::from_slice(manifest_payload)
        .map_err(|error| format!("invalid {authority} material manifest JSON: {error}"))?;

    let (entries, package_entries) = parse_material_entries(&manifest.entries, authority)?;

    let packages =
        validate_package_authority(&selectors, &manifest.entries, &package_entries, authority)?;

    Ok(MaterialLayer { packages, entries })
}

pub fn parse_material_recipe(
    module_id: &str,
    module_version: &str,
    packages_payload: &str,
    manifest_payload: &[u8],
) -> Result<MaterialRecipe, String> {
    if !valid_module_id(module_id) {
        return Err(format!(
            "invalid module id for material recipe: {module_id}"
        ));
    }

    let selectors = parse_packages_tsv(packages_payload)?;

    let manifest: MaterialManifest = serde_json::from_slice(manifest_payload)
        .map_err(|error| format!("invalid module material manifest JSON: {error}"))?;

    if manifest.module != module_id {
        return Err(format!(
            "module material manifest identity mismatch: expected={module_id} actual={}",
            manifest.module
        ));
    }

    if manifest.version != module_version {
        return Err(format!(
            "module material manifest version mismatch: module={module_version} material={}",
            manifest.version
        ));
    }

    let (material_entries, package_entries) = parse_material_entries(&manifest.entries, "module")?;

    let packages =
        validate_package_authority(&selectors, &manifest.entries, &package_entries, "module")?;

    Ok(MaterialRecipe {
        module: manifest.module,
        version: manifest.version,
        packages,
        entries: material_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    #[test]
    fn shared_material_layer_parses_without_module_identity() {
        let package_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let packages = format!("base-files\t13.8\tamd64\tbase-files.deb\t{}\n", package_sha);

        let manifest = serde_json::json!({
            "entries": [
                {
                    "path": "packages/base-files.deb",
                    "type": "file",
                    "mode": "0o664",
                    "size": 10,
                    "sha256": package_sha
                },
                {
                    "path": "rootfs/lib64",
                    "type": "symlink",
                    "mode": "0o777",
                    "target": "usr/lib64"
                }
            ]
        });

        let layer = parse_material_layer("Essential", &packages, manifest.to_string().as_bytes())
            .expect("shared material layer must parse");

        assert_eq!(layer.packages.len(), 1);
        assert_eq!(layer.entries.len(), 2);
        assert_eq!(layer.packages[0].filename, "base-files.deb");

        assert!(layer.entries.iter().any(|entry| {
            entry.path == "rootfs/lib64"
                && entry.kind == "symlink"
                && entry.target.as_deref() == Some("usr/lib64")
        }));
    }

    #[test]
    fn shared_material_layer_rejects_package_authority_mismatch() {
        let packages = concat!(
            "base-files\t13.8\tamd64\tbase-files.deb\t",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "\n"
        );

        let manifest = serde_json::json!({
            "entries": [
                {
                    "path": "packages/base-files.deb",
                    "type": "file",
                    "mode": "0o664",
                    "size": 10,
                    "sha256":
                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                }
            ]
        });

        let error = parse_material_layer("Essential", packages, manifest.to_string().as_bytes())
            .expect_err("mismatched package authority must fail");

        assert!(
            error.contains("Essential package sha256 authority mismatch"),
            "{error}"
        );
    }

    #[test]
    fn package_selector_carries_filename_and_sha() {
        let payload = format!(
            "python3.13-minimal\t3.13.5\tamd64\tpython.deb\t{}\n",
            sha('a')
        );

        let parsed = parse_packages_tsv(&payload).expect("valid package selector must parse");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["python.deb"].sha256, sha('a'));
    }

    #[test]
    fn package_selector_rejects_path_escape() {
        let payload = format!(
            "python3.13-minimal\t3.13.5\tamd64\t../python.deb\t{}\n",
            sha('a')
        );

        let error =
            parse_packages_tsv(&payload).expect_err("package selector path escape must fail");

        assert!(error.contains("unsafe module package filename"));
    }

    #[test]
    fn material_recipe_requires_same_package_sha() {
        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "size": 4,
                        "sha256": "{}"
                    }}
                ]
            }}"#,
            sha('b')
        );

        let packages = format!("python\t1\tamd64\tpython.deb\t{}\n", sha('a'));

        let error = parse_material_recipe("fixture", "1.0.0", &packages, payload.as_bytes())
            .expect_err("divergent package sha must fail");

        assert!(error.contains("sha256 authority mismatch"));
    }

    #[test]
    fn material_recipe_accepts_matching_membership() {
        let expected_sha = sha('a');

        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "size": 4,
                        "sha256": "{expected_sha}"
                    }}
                ]
            }}"#
        );

        let packages = format!("python\t1\tamd64\tpython.deb\t{expected_sha}\n");

        let recipe = parse_material_recipe("fixture", "1.0.0", &packages, payload.as_bytes())
            .expect("matching material recipe must validate");

        assert_eq!(recipe.packages.len(), 1);
        assert_eq!(recipe.packages[0].filename, "python.deb");
    }
    #[test]
    fn material_recipe_exposes_rootfs_entries() {
        let expected_sha = sha('a');

        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "size": 4,
                        "sha256": "{expected_sha}"
                    }},
                    {{
                        "path": "rootfs/usr",
                        "type": "directory",
                        "mode": "0o755"
                    }},
                    {{
                        "path": "rootfs/usr/bin/python3",
                        "type": "symlink",
                        "mode": "0o777",
                        "target": "python3.13"
                    }}
                ]
            }}"#
        );

        let packages = format!("python\t1\tamd64\tpython.deb\t{expected_sha}\n");

        let recipe = parse_material_recipe("fixture", "1.0.0", &packages, payload.as_bytes())
            .expect("complete material recipe must parse");

        assert_eq!(recipe.packages.len(), 1);
        assert_eq!(recipe.entries.len(), 3);

        assert!(recipe.entries.iter().any(|entry| {
            entry.path == "rootfs/usr/bin/python3"
                && entry.kind == "symlink"
                && entry.target.as_deref() == Some("python3.13")
        }));
    }

    #[test]
    fn material_recipe_rejects_path_escape() {
        let expected_sha = sha('a');

        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "size": 4,
                        "sha256": "{expected_sha}"
                    }},
                    {{
                        "path": "rootfs/../escape",
                        "type": "directory",
                        "mode": "0o755"
                    }}
                ]
            }}"#
        );

        let packages = format!("python\t1\tamd64\tpython.deb\t{expected_sha}\n");

        let error = parse_material_recipe("fixture", "1.0.0", &packages, payload.as_bytes())
            .expect_err("material path escape must fail");

        assert!(error.contains("unsafe module material path"));
    }
}
