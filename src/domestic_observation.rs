#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomesticObservation {
    pub evidence: Vec<String>,
    pub findings: Vec<String>,
}

impl DomesticObservation {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_evidence(evidence: Vec<String>) -> Self {
        Self {
            evidence,
            findings: Vec::new(),
        }
    }

    pub fn with_findings(findings: Vec<String>) -> Self {
        Self {
            evidence: Vec::new(),
            findings,
        }
    }

    pub fn with_evidence_and_findings(evidence: Vec<String>, findings: Vec<String>) -> Self {
        Self { evidence, findings }
    }

    pub fn has_findings(&self) -> bool {
        !self.findings.is_empty()
    }
}
