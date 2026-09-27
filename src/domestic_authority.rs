use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedAuthorityReport {
    pub undeclared: Vec<String>,
}

impl ObservedAuthorityReport {
    pub fn is_certified(&self) -> bool {
        self.undeclared.is_empty()
    }
}

pub fn inspect_observed_authorities(
    declared: &[String],
    observed: &[String],
) -> ObservedAuthorityReport {
    let declared = declared.iter().cloned().collect::<BTreeSet<_>>();

    let undeclared = observed
        .iter()
        .filter(|value| !declared.contains(value.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    ObservedAuthorityReport { undeclared }
}

pub fn certify_observed_authorities(
    declared: &[String],
    observed: &[String],
) -> Result<(), String> {
    let report = inspect_observed_authorities(declared, observed);

    if report.is_certified() {
        return Ok(());
    }

    Err(format!(
        "undeclared observed authorities: {}",
        report.undeclared.join(", ")
    ))
}
