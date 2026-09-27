use crate::config;
use crate::modules;

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

    let icon =
        crate::languages::client_root()?.join("assets/branding/neebles-boss-launcher-icon.png");

    let (desktop_uid, _) = crate::runtime_identity::desktop_identity()?;

    let runtime_dir = format!("/run/user/{desktop_uid}");

    let session_bus = format!("unix:path={runtime_dir}/bus");

    let protocol_environment = std::collections::BTreeMap::from([
        ("XDG_RUNTIME_DIR".to_string(), runtime_dir),
        ("DBUS_SESSION_BUS_ADDRESS".to_string(), session_bus),
    ]);

    let mut command = neebles_backend::domestic_environment::build_process_command(
        neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.notify-send")?,
        neebles_backend::domestic_environment::ProcessEnvironmentClass::SystemInterface,
        &protocol_environment,
        &std::collections::BTreeMap::new(),
        &std::collections::BTreeSet::new(),
    )?;

    let status = command
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
