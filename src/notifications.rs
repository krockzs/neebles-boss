use crate::config;
use crate::dependencies;
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

pub fn emit(severity: Severity, title: &str, message: &str) -> Result<(), String> {
    let config = config::load_or_initialize()?;
    if !severity.mandatory() && !config.normal_notifications {
        return Ok(());
    }

    if !dependencies::command_exists("notify-send") {
        return Err("notify-send is not available; install libnotify-bin for Plasma notifications".to_string());
    }

    let icon = crate::languages::client_root()
        .join("assets/branding/neebles-boss-launcher-icon.png");

    let status = Command::new("notify-send")
        .args(["-a", "N.E.E.B.L.E.S.", "-u", severity.urgency()])
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
