use crate::config;
use crate::dependencies;
use crate::modules;
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Critical,
    Fatal,
}

impl Severity {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" | "warn" => Ok(Self::Warning),
            "critical" => Ok(Self::Critical),
            "fatal" => Ok(Self::Fatal),
            _ => Err(format!("unknown notification severity: {value}")),
        }
    }

    fn urgency(self) -> &'static str {
        match self {
            Self::Info | Self::Success => "normal",
            Self::Warning => "normal",
            Self::Critical | Self::Fatal => "critical",
        }
    }

    fn mandatory(self) -> bool {
        matches!(self, Self::Critical | Self::Fatal)
    }
}

fn emit_transport(
    severity: Severity,
    application: &str,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let config = config::load_or_initialize()?;

    /*
     * Normal notifications obey the user's Boss policy.
     * Critical/Fatal notifications remain mandatory.
     */
    if !severity.mandatory() && !config.normal_notifications {
        return Ok(());
    }

    if !dependencies::command_exists("notify-send") {
        return Err(
            "notify-send is not available; install libnotify-bin for Plasma notifications"
                .to_string(),
        );
    }

    let icon =
        crate::languages::client_root()?.join("assets/branding/neebles-boss-launcher-icon.png");

    let status = Command::new("notify-send")
        .args(["-a", application, "-u", severity.urgency()])
        .arg("-i")
        .arg(icon)
        .arg(title)
        .arg(message)
        .status()
        .map_err(|error| format!("could not start Plasma notification: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("notify-send exited with status {status}"))
    }
}

pub fn emit(severity: Severity, title: &str, message: &str) -> Result<(), String> {
    emit_transport(severity, "N.E.E.B.L.E.S.", title, message)
}

pub fn emit_for_module(
    module: &str,
    severity: Severity,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let manifest = modules::installed_module_manifest(module)?;

    if !config::module_enabled(module)? {
        return Err(format!(
            "module '{}' is disabled and cannot emit notifications",
            module
        ));
    }

    let contract = manifest.notifications.as_ref().ok_or_else(|| {
        format!(
            "module '{}' does not declare a notifications capability",
            module
        )
    })?;

    if contract.protocol != modules::MODULE_NOTIFICATIONS_PROTOCOL_VERSION {
        return Err(format!(
            "module '{}' declares unsupported notifications protocol {}; expected {}",
            module,
            contract.protocol,
            modules::MODULE_NOTIFICATIONS_PROTOCOL_VERSION
        ));
    }

    /*
     * The module owns title/message and resolves them using
     * its own NEEBLES_LANGUAGE contract.
     *
     * Boss only governs transport and policy.
     */
    emit_transport(
        severity,
        &format!("N.E.E.B.L.E.S. · {}", manifest.name),
        title,
        message,
    )
}
