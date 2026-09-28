use crate::lifecycle_battlefield::Battlefield;
fn is_reference_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == '.'
}

fn split_reference(reference: &str) -> Result<(&str, &str), String> {
    let Some((namespace, path)) = reference.split_once('.') else {
        return Err(format!(
            "dynamic reference '{reference}' must contain namespace and path"
        ));
    };

    if namespace.trim().is_empty() {
        return Err(format!(
            "dynamic reference '{reference}' has an empty namespace"
        ));
    }

    if path.trim().is_empty() {
        return Err(format!("dynamic reference '{reference}' has an empty path"));
    }

    Ok((namespace, path))
}

fn resolve_reference(battlefield: &Battlefield, reference: &str) -> Result<String, String> {
    let (namespace, path) = split_reference(reference)?;

    battlefield
        .get(namespace, path)
        .map(str::to_string)
        .ok_or_else(|| format!("dynamic reference '{reference}' could not be resolved"))
}

/*
 * Resolve every dynamic reference contained in one String.
 *
 * Syntax:
 * - one marker begins a dynamic reference
 * - two consecutive markers produce one literal marker
 *
 * A reference token accepts ASCII letters, digits, underscore,
 * hyphen and dot. The first dot separates namespace from path.
 * Additional dots belong to the path and remain opaque to the resolver.
 */
pub fn resolve(input: &str, battlefield: &Battlefield) -> Result<String, String> {
    let characters: Vec<char> = input.chars().collect();
    let mut output = String::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] != '$' {
            output.push(characters[index]);
            index += 1;
            continue;
        }

        if index + 1 < characters.len() && characters[index + 1] == '$' {
            output.push('$');
            index += 2;
            continue;
        }

        let reference_start = index + 1;
        let mut reference_end = reference_start;

        while reference_end < characters.len() && is_reference_character(characters[reference_end])
        {
            reference_end += 1;
        }

        if reference_start == reference_end {
            return Err("dynamic reference marker is not followed by a reference".to_string());
        }

        let reference: String = characters[reference_start..reference_end].iter().collect();

        let value = resolve_reference(battlefield, &reference)?;

        output.push_str(&value);
        index = reference_end;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battlefield() -> Battlefield {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "one", "VALUE_ONE");
        battlefield.insert("alpha", "nested.value", "VALUE_NESTED");
        battlefield.insert("beta", "path", "/tmp/example");
        battlefield.insert("result", "hash", "abc123");
        battlefield.insert("intelligence", "first.output", "CHAINED");

        battlefield
    }

    #[test]
    fn resolves_complete_reference() {
        assert_eq!(resolve("$alpha.one", &battlefield()).unwrap(), "VALUE_ONE");
    }

    #[test]
    fn resolves_reference_with_opaque_dotted_path() {
        assert_eq!(
            resolve("$alpha.nested.value", &battlefield()).unwrap(),
            "VALUE_NESTED"
        );
    }

    #[test]
    fn resolves_interpolation() {
        assert_eq!(
            resolve("prefix/$beta.path/suffix", &battlefield()).unwrap(),
            "prefix//tmp/example/suffix"
        );
    }

    #[test]
    fn resolves_multiple_references() {
        assert_eq!(
            resolve(
                "$alpha.one:$result.hash:$intelligence.first.output",
                &battlefield(),
            )
            .unwrap(),
            "VALUE_ONE:abc123:CHAINED"
        );
    }

    #[test]
    fn preserves_plain_string() {
        assert_eq!(
            resolve("plain technical value", &battlefield()).unwrap(),
            "plain technical value"
        );
    }

    #[test]
    fn double_marker_produces_literal_marker() {
        assert_eq!(
            resolve("$$alpha.one", &battlefield()).unwrap(),
            "$alpha.one"
        );
    }

    #[test]
    fn rejects_missing_namespace() {
        let error = resolve("$missing.value", &battlefield()).unwrap_err();

        assert!(error.contains("could not be resolved"));
    }

    #[test]
    fn rejects_missing_path() {
        let error = resolve("$alpha.missing", &battlefield()).unwrap_err();

        assert!(error.contains("could not be resolved"));
    }

    #[test]
    fn rejects_reference_without_path() {
        let error = resolve("$alpha", &battlefield()).unwrap_err();

        assert!(error.contains("must contain namespace and path"));
    }

    #[test]
    fn rejects_bare_marker() {
        let error = resolve("$", &battlefield()).unwrap_err();

        assert!(error.contains("not followed by a reference"));
    }

    #[test]
    fn namespaces_are_not_hardcoded() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("future_namespace", "whatever.path", "future-value");

        assert_eq!(
            resolve("$future_namespace.whatever.path", &battlefield,).unwrap(),
            "future-value"
        );
    }

    #[test]
    fn same_resolver_handles_result_and_intelligence_as_plain_namespaces() {
        let battlefield = battlefield();

        assert_eq!(resolve("$result.hash", &battlefield).unwrap(), "abc123");

        assert_eq!(
            resolve("$intelligence.first.output", &battlefield,).unwrap(),
            "CHAINED"
        );
    }
}
