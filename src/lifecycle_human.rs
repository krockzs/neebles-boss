use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_resolver;

/*
 * Lifecycle human communication.
 *
 * Technical responsibility:
 * Hold module-provided human-readable communication and resolve
 * dynamic values through the generic Battlefield resolver.
 *
 * This layer does not:
 * - choose a presentation surface
 * - know Plasma notifications
 * - know Lifecycle Modal
 * - define events
 * - define logs
 * - define severity vocabulary
 * - execute Lifecycle operations
 *
 * Module owns message content.
 * Boss presentation is a separate responsibility.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanMessageTemplate {
    message: String,
    detail: Option<String>,
}

impl HumanMessageTemplate {
    pub fn new(message: impl Into<String>) -> Result<Self, String> {
        let message = message.into();

        if message.trim().is_empty() {
            return Err("human message cannot be empty".to_string());
        }

        Ok(Self {
            message,
            detail: None,
        })
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Result<Self, String> {
        let detail = detail.into();

        if detail.trim().is_empty() {
            return Err("human message detail cannot be empty".to_string());
        }

        self.detail = Some(detail);

        Ok(self)
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn detail_text(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedHumanMessage {
    message: String,
    detail: Option<String>,
}

impl ResolvedHumanMessage {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

pub fn resolve(
    template: &HumanMessageTemplate,
    battlefield: &Battlefield,
) -> Result<ResolvedHumanMessage, String> {
    let message = lifecycle_resolver::resolve(template.message(), battlefield)
        .map_err(|error| format!("could not resolve human message: {error}"))?;

    let detail = match template.detail_text() {
        Some(value) => Some(
            lifecycle_resolver::resolve(value, battlefield)
                .map_err(|error| format!("could not resolve human message detail: {error}"))?,
        ),

        None => None,
    };

    Ok(ResolvedHumanMessage { message, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(namespace: &str, path: &str) -> String {
        let marker = char::from_u32(36).unwrap();

        format!("{marker}{namespace}.{path}")
    }

    #[test]
    fn human_message_requires_nonempty_message() {
        let result = HumanMessageTemplate::new("   ");

        assert!(result.is_err());
    }

    #[test]
    fn human_message_accepts_arbitrary_text() {
        let template = HumanMessageTemplate::new("Anything the module wants to explain").unwrap();

        assert_eq!(template.message(), "Anything the module wants to explain");
    }

    #[test]
    fn detail_is_optional() {
        let template = HumanMessageTemplate::new("Operation running").unwrap();

        assert_eq!(template.detail_text(), None);
    }

    #[test]
    fn detail_can_be_supplied() {
        let template = HumanMessageTemplate::new("Operation failed")
            .unwrap()
            .detail("Technical information")
            .unwrap();

        assert_eq!(template.detail_text(), Some("Technical information"));
    }

    #[test]
    fn empty_detail_is_rejected() {
        let result = HumanMessageTemplate::new("message").unwrap().detail("   ");

        assert!(result.is_err());
    }

    #[test]
    fn plain_message_resolves_unchanged() {
        let battlefield = Battlefield::new();

        let template = HumanMessageTemplate::new("Plain human message").unwrap();

        let resolved = resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.message(), "Plain human message");
    }

    #[test]
    fn dynamic_message_resolves_from_battlefield() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("runtime", "module_name", "Synthetic Module");

        let dynamic = reference("runtime", "module_name");

        let template = HumanMessageTemplate::new(format!("Starting {dynamic}")).unwrap();

        let resolved = resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.message(), "Starting Synthetic Module");
    }

    #[test]
    fn dynamic_detail_resolves_from_battlefield() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("result", "technical", "synthetic detail");

        let dynamic = reference("result", "technical");

        let template = HumanMessageTemplate::new("Operation failed")
            .unwrap()
            .detail(format!("Detail: {dynamic}"))
            .unwrap();

        let resolved = resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.detail(), Some("Detail: synthetic detail"));
    }

    #[test]
    fn missing_message_reference_is_reported() {
        let battlefield = Battlefield::new();

        let dynamic = reference("missing", "value");

        let template = HumanMessageTemplate::new(dynamic).unwrap();

        let error = resolve(&template, &battlefield).unwrap_err();

        assert!(error.contains("could not resolve human message"));
    }

    #[test]
    fn missing_detail_reference_is_reported() {
        let battlefield = Battlefield::new();

        let dynamic = reference("missing", "detail");

        let template = HumanMessageTemplate::new("message")
            .unwrap()
            .detail(dynamic)
            .unwrap();

        let error = resolve(&template, &battlefield).unwrap_err();

        assert!(error.contains("could not resolve human message detail"));
    }

    #[test]
    fn resolver_does_not_prescribe_human_vocabulary() {
        let battlefield = Battlefield::new();

        let template = HumanMessageTemplate::new("future module wording")
            .unwrap()
            .detail("future module detail")
            .unwrap();

        let resolved = resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.message(), "future module wording");

        assert_eq!(resolved.detail(), Some("future module detail"));
    }

    #[test]
    fn resolving_message_does_not_execute_anything() {
        let battlefield = Battlefield::new();

        let template = HumanMessageTemplate::new("Communication only").unwrap();

        let resolved = resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.message(), "Communication only");
    }
}
