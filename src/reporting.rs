use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ReportingEnvelope {
    #[serde(rename = "type")]
    pub report_type: String,
    pub endpoint: String,
    pub activate: bool,
    pub package: Value,
    pub message: Value,
}

pub fn build_report<PackageBuilder, MessageBuilder>(
    report_type: impl Into<String>,
    endpoint: impl Into<String>,
    activate: bool,
    package_builder: PackageBuilder,
    message_builder: MessageBuilder,
) -> Option<ReportingEnvelope>
where
    PackageBuilder: FnOnce() -> Value,
    MessageBuilder: FnOnce() -> Value,
{
    if !activate {
        return None;
    }

    Some(ReportingEnvelope {
        report_type: report_type.into(),
        endpoint: endpoint.into(),
        activate,
        package: package_builder(),
        message: message_builder(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    #[test]
    fn disabled_reporting_returns_before_building_payload() {
        let package_called = Cell::new(false);
        let message_called = Cell::new(false);

        let report = build_report(
            "stage0",
            "https://example.invalid/report",
            false,
            || {
                package_called.set(true);
                json!({"package": true})
            },
            || {
                message_called.set(true);
                json!({"message": true})
            },
        );

        assert!(report.is_none());
        assert!(!package_called.get());
        assert!(!message_called.get());
    }

    #[test]
    fn enabled_reporting_builds_generic_envelope() {
        let report = build_report(
            "stage0",
            "https://example.invalid/report",
            true,
            || json!({"version": "1.0.0"}),
            || json!({"error": "example"}),
        )
        .expect("report should be built");

        assert_eq!(report.report_type, "stage0");
        assert_eq!(report.endpoint, "https://example.invalid/report");
        assert!(report.activate);
        assert_eq!(report.package, json!({"version": "1.0.0"}));
        assert_eq!(report.message, json!({"error": "example"}));

        let serialized = serde_json::to_value(report).expect("report should serialize");

        assert_eq!(serialized["type"], "stage0");
        assert_eq!(serialized["endpoint"], "https://example.invalid/report");
        assert_eq!(serialized["activate"], true);
        assert_eq!(serialized["package"], json!({"version": "1.0.0"}));
        assert_eq!(serialized["message"], json!({"error": "example"}));
    }
}
