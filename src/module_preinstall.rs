use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use url::Url;

use neebles_backend::domestic_construction::DomesticConstructionDeclaration;
use neebles_backend::module_material::{
    parse_material_layer, parse_material_recipe, valid_module_id, PackageRequirement,
};
use neebles_backend::module_material_binding::MaterialBindingInput;

const CUSTOM_REPOSITORY: &str = "krockzs/neebles-custom";
const CUSTOM_REPOSITORY_GIT: &str = "https://github.com/krockzs/neebles-custom.git";
const CUSTOM_BRANCH_CANDIDATES: [&str; 2] = ["main", "master"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePreinstallReport {
    pub module: String,
    pub custom_revision: String,
    pub required: usize,
    pub reused: usize,
    pub downloaded: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedModulePackages {
    pub(crate) report: ModulePreinstallReport,
    pub(crate) binding_input: MaterialBindingInput,
    pub(crate) material_root: PathBuf,
}

fn resolve_custom_revision() -> Result<String, String> {
    let mut failures = Vec::new();

    for branch in CUSTOM_BRANCH_CANDIDATES {
        let reference = format!("refs/heads/{branch}");

        let arguments = [
            OsString::from("ls-remote"),
            OsString::from(CUSTOM_REPOSITORY_GIT),
            OsString::from(&reference),
        ];

        let output = match crate::network_boundary::command("boss.git", &arguments) {
            Ok(mut command) => match command.output() {
                Ok(output) => output,
                Err(error) => {
                    failures.push(format!("{branch}: {error}"));
                    continue;
                }
            },
            Err(error) => {
                failures.push(format!("{branch}: {error}"));
                continue;
            }
        };

        if !output.status.success() {
            failures.push(format!(
                "{branch}: git ls-remote returned {}",
                output.status
            ));
            continue;
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("invalid CUSTOM git revision output: {error}"))?;

        let Some(revision) = stdout.split_whitespace().next() else {
            failures.push(format!("{branch}: no revision returned"));
            continue;
        };

        if revision.len() != 40 || !revision.bytes().all(|value| value.is_ascii_hexdigit()) {
            failures.push(format!("{branch}: invalid revision returned: {revision}"));
            continue;
        }

        return Ok(revision.to_ascii_lowercase());
    }

    Err(format!(
        "could not resolve canonical N.E.E.B.L.E.S. CUSTOM revision: {}",
        failures.join(" | ")
    ))
}

fn custom_raw_url(revision: &str, path: &str) -> Result<String, String> {
    if revision.len() != 40 || !revision.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(format!("invalid CUSTOM raw revision: {revision}"));
    }

    if path.is_empty() || path.starts_with('/') || path.ends_with('/') {
        return Err(format!("invalid CUSTOM raw path: {path}"));
    }

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("invalid CUSTOM raw path segment in: {path}"));
        }

        if segment.chars().any(char::is_control) {
            return Err(format!(
                "CUSTOM raw path contains control characters: {path:?}"
            ));
        }
    }

    let mut url = Url::parse("https://raw.githubusercontent.com/")
        .map_err(|error| format!("could not construct CUSTOM raw URL base: {error}"))?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "CUSTOM raw URL base cannot accept path segments".to_string())?;

        segments.pop_if_empty();

        for segment in CUSTOM_REPOSITORY.split('/') {
            segments.push(segment);
        }

        segments.push(revision);

        for segment in path.split('/') {
            segments.push(segment);
        }
    }

    Ok(url.to_string())
}

fn fetch_remote_bytes(revision: &str, path: &str, timeout_seconds: u32) -> Result<Vec<u8>, String> {
    let url = custom_raw_url(revision, path)?;

    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from(timeout_seconds.to_string()),
        OsString::from(&url),
    ];

    let output = crate::network_boundary::command("boss.curl", &arguments)?
        .output()
        .map_err(|error| format!("could not fetch module material metadata: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "module material metadata download failed for {path}: {}",
            output.status
        ));
    }

    Ok(output.stdout)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];

    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("could not hash {}: {error}", path.display()))?;

        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn verify_package(path: &Path, expected_sha256: &str) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(error) => {
            return Err(format!(
                "could not inspect module package {}: {error}",
                path.display()
            ));
        }
    };

    if !metadata.file_type().is_file() {
        return Err(format!(
            "module package path exists but is not a regular file: {}",
            path.display()
        ));
    }

    let actual = sha256_file(path)?;

    if actual != expected_sha256 {
        return Err(format!(
            "module package sha256 mismatch: {}",
            path.display()
        ));
    }

    Ok(true)
}

fn download_package(
    revision: &str,
    pool: &Path,
    remote_pool: &str,
    requirement: &PackageRequirement,
) -> Result<bool, String> {
    let target = pool.join(&requirement.filename);

    if verify_package(&target, &requirement.sha256)? {
        return Ok(false);
    }

    let temporary = tempfile::NamedTempFile::new_in(pool).map_err(|error| {
        format!(
            "could not create module package staging file in {}: {error}",
            pool.display()
        )
    })?;

    let output = temporary
        .reopen()
        .map_err(|error| format!("could not reopen module package staging file: {error}"))?;

    let remote_path = format!("{remote_pool}/{}", requirement.filename);

    let url = custom_raw_url(revision, &remote_path)?;

    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from("300"),
        OsString::from(&url),
    ];

    let status = crate::network_boundary::command("boss.curl", &arguments)?
        .stdout(Stdio::from(output))
        .status()
        .map_err(|error| {
            format!(
                "could not download module package '{}': {error}",
                requirement.filename
            )
        })?;

    if !status.success() {
        return Err(format!(
            "module package '{}' download failed with status {}",
            requirement.filename, status
        ));
    }

    if !verify_package(temporary.path(), &requirement.sha256)? {
        return Err(format!(
            "downloaded module package disappeared before verification: {}",
            requirement.filename
        ));
    }

    match temporary.persist_noclobber(&target) {
        Ok(_) => Ok(true),

        Err(error) => {
            let persist_error = error.error.to_string();
            drop(error.file);

            if verify_package(&target, &requirement.sha256)? {
                return Ok(false);
            }

            Err(format!(
                "could not publish module package '{}': {}",
                requirement.filename, persist_error
            ))
        }
    }
}

fn ensure_package_requirements(
    revision: &str,
    pool: &Path,
    remote_pool: &str,
    requirements: &[PackageRequirement],
) -> Result<(usize, usize), String> {
    let mut reused = 0usize;
    let mut downloaded = 0usize;

    for requirement in requirements {
        let target = pool.join(&requirement.filename);

        if verify_package(&target, &requirement.sha256)? {
            reused += 1;
            continue;
        }

        if download_package(revision, pool, remote_pool, requirement)? {
            downloaded += 1;
        } else {
            reused += 1;
        }
    }

    Ok((reused, downloaded))
}

pub(crate) fn ensure_module_packages(module_id: &str) -> Result<PreparedModulePackages, String> {
    if !valid_module_id(module_id) {
        return Err(format!("invalid module id for preinstall: {module_id}"));
    }

    let installed = crate::modules::installed_module_manifest(module_id)?;

    let revision = resolve_custom_revision()?;

    let essentials_payload = fetch_remote_bytes(
        &revision,
        "runtime/manifests/modules/essentials.packages.tsv",
        30,
    )?;

    let essentials_text = std::str::from_utf8(&essentials_payload)
        .map_err(|error| format!("Essential package selector is not UTF-8: {error}"))?;

    let essentials_manifest_payload = fetch_remote_bytes(
        &revision,
        "runtime/manifests/modules/essentials.manifest.json",
        30,
    )?;

    let essential_layer =
        parse_material_layer("Essential", essentials_text, &essentials_manifest_payload)?;

    let packages_path = format!("runtime/manifests/modules/{module_id}.packages.tsv");
    let manifest_path = format!("runtime/manifests/modules/{module_id}.manifest.json");

    let packages_payload = fetch_remote_bytes(&revision, &packages_path, 30)?;

    let packages_text = std::str::from_utf8(&packages_payload)
        .map_err(|error| format!("module package selector is not UTF-8: {error}"))?;

    let manifest_payload = fetch_remote_bytes(&revision, &manifest_path, 30)?;

    let recipe = parse_material_recipe(
        module_id,
        &installed.version,
        packages_text,
        &manifest_payload,
    )?;

    for requirement in &recipe.packages {
        if essential_layer
            .packages
            .iter()
            .any(|essential| essential.filename.as_str() == requirement.filename.as_str())
        {
            return Err(format!(
                "module package delta repeats Essential package: {}",
                requirement.filename
            ));
        }
    }

    let runtime_manifest_payload =
        fetch_remote_bytes(&revision, "runtime/modules/domestic-runtime.json", 30)?;

    let construction_path = format!("runtime/construction/{module_id}.json");
    let construction_payload = fetch_remote_bytes(&revision, &construction_path, 30)?;

    let construction_text = std::str::from_utf8(&construction_payload)
        .map_err(|error| format!("module Construction declaration is not UTF-8: {error}"))?;

    let construction = DomesticConstructionDeclaration::parse(construction_text)?;

    if construction.subject != module_id {
        return Err(format!(
            "module Construction subject mismatch: expected={module_id} actual={}",
            construction.subject
        ));
    }

    let territory =
        neebles_backend::module_material_territory::resolve_module_material_territory()?;

    let (essential_reused, essential_downloaded) = ensure_package_requirements(
        &revision,
        &territory.essential_package_pool,
        "runtime/modules/packages/essentials",
        &essential_layer.packages,
    )?;

    let (module_reused, module_downloaded) = ensure_package_requirements(
        &revision,
        &territory.package_pool,
        "runtime/modules/packages",
        &recipe.packages,
    )?;

    let binding_input = MaterialBindingInput {
        module: module_id.to_string(),
        version: installed.version.clone(),
        custom_revision: revision.clone(),
        essential_packages_payload: essentials_payload,
        essential_manifest_payload: essentials_manifest_payload,
        module_packages_payload: packages_payload,
        module_manifest_payload: manifest_payload,
        runtime_manifest_payload: runtime_manifest_payload.clone(),
        construction_payload: construction_payload.clone(),
    };

    Ok(PreparedModulePackages {
        report: ModulePreinstallReport {
            module: module_id.to_string(),
            custom_revision: revision,
            required: essential_layer.packages.len() + recipe.packages.len(),
            reused: essential_reused + module_reused,
            downloaded: essential_downloaded + module_downloaded,
        },
        binding_input,
        material_root: territory.material_root,
    })
}

#[cfg(test)]
mod custom_raw_url_tests {
    use super::custom_raw_url;

    const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

    fn build(path: &str) -> String {
        custom_raw_url(REVISION, path).expect("CUSTOM raw URL must build")
    }

    #[test]
    fn normal_debian_filename_remains_semantically_unchanged() {
        let url =
            build("runtime/modules/packages/essentials/base-files_13.8+deb13u7~fixture_amd64.deb");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/krockzs/neebles-custom/0123456789abcdef0123456789abcdef01234567/runtime/modules/packages/essentials/base-files_13.8+deb13u7~fixture_amd64.deb"
        );
    }

    #[test]
    fn literal_percent_epoch_is_not_decoded_by_the_url() {
        let url =
            build("runtime/modules/packages/essentials/bsdutils_1%3a2.41.5-0+deb13u1_amd64.deb");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/krockzs/neebles-custom/0123456789abcdef0123456789abcdef01234567/runtime/modules/packages/essentials/bsdutils_1%253a2.41.5-0+deb13u1_amd64.deb"
        );
    }

    #[test]
    fn encoded_slash_text_cannot_become_a_path_separator() {
        let url = build("runtime/modules/packages/fixture_%2f_payload.deb");

        assert!(url.contains("fixture_%252f_payload.deb"), "{url}");
        assert!(!url.contains("fixture_%2f_payload.deb"), "{url}");
    }

    #[test]
    fn encoded_dot_segments_cannot_become_traversal() {
        let url = build("runtime/modules/packages/fixture_%2e%2e_payload.deb");

        assert!(url.contains("fixture_%252e%252e_payload.deb"), "{url}");
    }

    #[test]
    fn query_fragment_and_space_are_data_inside_the_segment() {
        let url = build("runtime/modules/packages/name #part?.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(url.contains("name%20%23part%3F.deb"), "{url}");
        assert!(
            parsed.query().is_none(),
            "query must not escape package filename"
        );
        assert!(
            parsed.fragment().is_none(),
            "fragment must not escape package filename"
        );
    }

    #[test]
    fn unicode_is_encoded_as_segment_data() {
        let url = build("runtime/modules/packages/módulo_ñ.deb");

        assert!(url.contains("m%C3%B3dulo_%C3%B1.deb"), "{url}");
    }

    #[test]
    fn backslash_cannot_become_url_path_structure() {
        let url = build("runtime/modules/packages/name\\payload.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(url.contains("name%5Cpayload.deb"), "{url}");
        assert_eq!(
            parsed.path_segments().expect("hierarchical URL").count(),
            7,
            "backslash must remain inside one package path segment"
        );
    }

    #[test]
    fn raw_colon_ampersand_equals_semicolon_and_at_remain_path_data() {
        let url = build("runtime/modules/packages/name:@&=;.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(parsed.query().is_none());
        assert!(parsed.fragment().is_none());
        assert_eq!(parsed.path_segments().expect("hierarchical URL").count(), 7);
    }

    #[test]
    fn revision_cannot_inject_url_structure() {
        assert!(custom_raw_url("main/../../escape", "runtime/modules/packages/a.deb").is_err());
        assert!(custom_raw_url(
            "0123456789abcdef0123456789abcdef0123456g",
            "runtime/modules/packages/a.deb"
        )
        .is_err());
    }

    #[test]
    fn structural_path_ambiguity_is_rejected() {
        for path in [
            "",
            "/runtime/modules/packages/a.deb",
            "runtime/modules/packages/a.deb/",
            "runtime//modules/packages/a.deb",
            "runtime/./packages/a.deb",
            "runtime/../packages/a.deb",
        ] {
            assert!(
                custom_raw_url(REVISION, path).is_err(),
                "path should be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn control_characters_are_rejected() {
        for path in [
            "runtime/modules/packages/a\n.deb",
            "runtime/modules/packages/a\0.deb",
        ] {
            assert!(
                custom_raw_url(REVISION, path).is_err(),
                "control path should be rejected: {path:?}"
            );
        }
    }
    fn percent_hex_value(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("invalid percent hex byte: {byte:?}"),
        }
    }

    fn decode_serialized_segment(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0usize;

        while index < bytes.len() {
            if bytes[index] == b'%' {
                assert!(
                    index + 2 < bytes.len(),
                    "truncated percent escape in serialized segment: {value:?}"
                );

                decoded.push(
                    (percent_hex_value(bytes[index + 1]) << 4)
                        | percent_hex_value(bytes[index + 2]),
                );

                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }

        String::from_utf8(decoded)
            .unwrap_or_else(|error| panic!("decoded segment is not UTF-8 for {value:?}: {error}"))
    }

    fn assert_filename_round_trip(filename: &str) {
        let path = format!("runtime/modules/packages/{filename}");
        let generated = custom_raw_url(REVISION, &path)
            .unwrap_or_else(|error| panic!("URL rejected filename {filename:?}: {error}"));

        let parsed = url::Url::parse(&generated).unwrap_or_else(|error| {
            panic!("generated URL does not parse for {filename:?}: {error}")
        });

        assert_eq!(parsed.scheme(), "https", "scheme changed for {filename:?}");
        assert_eq!(
            parsed.host_str(),
            Some("raw.githubusercontent.com"),
            "host changed for {filename:?}: {generated}"
        );

        assert!(
            parsed.query().is_none(),
            "filename escaped into query for {filename:?}: {generated}"
        );

        assert!(
            parsed.fragment().is_none(),
            "filename escaped into fragment for {filename:?}: {generated}"
        );

        let segments = parsed
            .path_segments()
            .expect("raw GitHub URL must be hierarchical")
            .collect::<Vec<_>>();

        assert_eq!(
            segments.len(),
            7,
            "filename changed URL path structure for {filename:?}: {segments:?}"
        );

        assert_eq!(segments[0], "krockzs");
        assert_eq!(segments[1], "neebles-custom");
        assert_eq!(segments[2], REVISION);
        assert_eq!(segments[3], "runtime");
        assert_eq!(segments[4], "modules");
        assert_eq!(segments[5], "packages");

        let decoded = decode_serialized_segment(segments[6]);

        assert_eq!(
            decoded, filename,
            "filename failed exact one-layer URL round-trip: original={filename:?} serialized={:?}",
            segments[6]
        );
    }

    #[test]
    fn every_printable_ascii_filename_character_round_trips() {
        let mut tested = 0usize;

        for code in 0x20u32..=0x7eu32 {
            let character = char::from_u32(code).expect("printable ASCII");

            if character == '/' {
                continue;
            }

            let filename = format!("fixture{character}payload.deb");
            assert_filename_round_trip(&filename);
            tested += 1;
        }

        eprintln!("printable ASCII filename cases: {tested}");
        assert_eq!(tested, 94);
    }

    #[test]
    fn every_percent_hex_triplet_remains_literal_filename_text() {
        let mut tested = 0usize;

        for value in 0u16..=255u16 {
            let upper = format!("fixture%{value:02X}payload.deb");
            let lower = format!("fixture%{value:02x}payload.deb");

            assert_filename_round_trip(&upper);
            assert_filename_round_trip(&lower);

            tested += 2;
        }

        eprintln!("literal percent escape-looking cases: {tested}");
        assert_eq!(tested, 512);
    }

    #[test]
    fn every_ascii_control_character_is_rejected() {
        let mut tested = 0usize;

        for code in (0u32..=31u32).chain(std::iter::once(127u32)) {
            let character = char::from_u32(code).expect("ASCII control character");
            let path = format!("runtime/modules/packages/fixture{character}payload.deb");

            assert!(
                custom_raw_url(REVISION, &path).is_err(),
                "ASCII control character U+{code:04X} was accepted"
            );

            tested += 1;
        }

        eprintln!("ASCII control rejection cases: {tested}");
        assert_eq!(tested, 33);
    }

    #[test]
    fn combinatorial_sensitive_filename_matrix_round_trips() {
        let tokens = [
            "%3a", "%3A", "%2f", "%2F", "%2e", "%2E", "%2e%2e", "%2E%2E", "%00", "%0a", "%0A",
            "%0d", "%0D", "%25", "%ff", "%FF", "%", "%z", "%zz", "+", "~", "#", "?", " ", ":", "@",
            "&", "=", ";", "\\", "[", "]", "(", ")", "'", "\"", "<", ">", "`", "{", "}", "|", "^",
            ",", "!", "$", "*", "é", "ñ", "中", "😀", "\u{0301}", "\u{200f}",
        ];

        let mut singles = 0usize;
        let mut pairs = 0usize;
        let mut triples = 0usize;

        for first in tokens {
            let filename = format!("fixture{first}payload.deb");
            assert_filename_round_trip(&filename);
            singles += 1;
        }

        for first in tokens {
            for second in tokens {
                let filename = format!("fixture{first}{second}payload.deb");
                assert_filename_round_trip(&filename);
                pairs += 1;
            }
        }

        for first in tokens {
            for second in tokens {
                for third in tokens {
                    let filename = format!("fixture{first}{second}{third}payload.deb");
                    assert_filename_round_trip(&filename);
                    triples += 1;
                }
            }
        }

        eprintln!("sensitive token singles: {singles}");
        eprintln!("sensitive token ordered pairs: {pairs}");
        eprintln!("sensitive token ordered triples: {triples}");
        eprintln!("sensitive token total: {}", singles + pairs + triples);

        assert_eq!(singles, tokens.len());
        assert_eq!(pairs, tokens.len() * tokens.len());
        assert_eq!(triples, tokens.len() * tokens.len() * tokens.len());
    }

    #[test]
    fn revision_validation_matrix_is_fail_closed() {
        let lower = "0123456789abcdef0123456789abcdef01234567";
        let upper = "0123456789ABCDEF0123456789ABCDEF01234567";

        assert!(custom_raw_url(lower, "runtime/modules/packages/a.deb").is_ok());
        assert!(custom_raw_url(upper, "runtime/modules/packages/a.deb").is_ok());

        let invalid = [
            "",
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789abcdef0123456789abcdef012345678",
            "0123456789abcdef0123456789abcdef0123456g",
            "0123456789abcdef0123456789abcdef0123456/",
            "0123456789abcdef0123456789abcdef0123456?",
            "0123456789abcdef0123456789abcdef0123456#",
            "0123456789abcdef0123456789abcdef0123456%",
        ];

        for revision in invalid {
            assert!(
                custom_raw_url(revision, "runtime/modules/packages/a.deb").is_err(),
                "invalid revision was accepted: {revision:?}"
            );
        }
    }
}
