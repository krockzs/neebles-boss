use crate::config;
use crate::modules;

use std::collections::{BTreeMap, HashMap};
use std::sync::{mpsc, OnceLock, RwLock};
use std::thread;

use zbus::blocking::{connection, Proxy};
use zbus::zvariant::{Array, OwnedValue, Str, Value};

const NOTIFICATIONS_SERVICE: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATIONS_INTERFACE: &str = "org.freedesktop.Notifications";

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Critical,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationImageData {
    pub width: i32,
    pub height: i32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationOptions {
    pub replace_id: Option<u32>,
    pub expire_timeout_ms: Option<i32>,
    pub category: Option<String>,
    pub desktop_entry: Option<String>,
    pub resident: bool,
    pub transient: bool,
    pub sound_name: Option<String>,
    pub sound_file: Option<String>,
    pub suppress_sound: bool,
    pub image_path: Option<String>,
    pub image_data: Option<NotificationImageData>,
    pub kde_urls: Vec<String>,
    pub kde_origin_name: Option<String>,
    pub kde_display_appname: Option<String>,
}

pub const DEFAULT_NOTIFICATION_EXPIRE_MS: i32 = 3000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationReply {
    pub label: String,
    pub placeholder_text: Option<String>,
    pub submit_button_text: Option<String>,
    pub submit_button_icon_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationPresentation {
    pub application: String,
    pub icon: String,
    pub title: String,
    pub message: String,
    pub actions: Vec<NotificationAction>,
    pub reply: Option<NotificationReply>,
    pub options: NotificationOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationOutcome {
    Presented(u32),
    Suppressed,
}

impl NotificationOutcome {
    pub fn notification_id(self) -> Option<u32> {
        match self {
            Self::Presented(id) => Some(id),
            Self::Suppressed => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotificationOwner {
    Boss,
    Module { module: String, session_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotificationReturnEvent {
    Closed {
        notification_id: u32,
        reason: u32,
    },

    ActionInvoked {
        notification_id: u32,
        action_key: String,
    },

    Replied {
        notification_id: u32,
        text: String,
    },

    ActivationToken {
        notification_id: u32,
        activation_token: String,
    },
}

impl NotificationReturnEvent {
    pub(crate) fn notification_id(&self) -> u32 {
        match self {
            Self::Closed {
                notification_id, ..
            }
            | Self::ActionInvoked {
                notification_id, ..
            }
            | Self::Replied {
                notification_id, ..
            }
            | Self::ActivationToken {
                notification_id, ..
            } => *notification_id,
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Closed { .. })
    }
}

#[derive(Debug, Default)]
struct NotificationOwnershipRegistry {
    owners: RwLock<BTreeMap<u32, NotificationOwner>>,
}

impl NotificationOwnershipRegistry {
    fn register(&self, notification_id: u32, owner: NotificationOwner) -> Result<(), String> {
        if notification_id == 0 {
            return Err("notification ownership cannot register id 0".to_string());
        }

        let mut owners = self
            .owners
            .write()
            .map_err(|_| "notification ownership registry write lock poisoned".to_string())?;

        if let Some(existing) = owners.get(&notification_id) {
            if existing != &owner {
                return Err(format!(
                    "notification id {} is already owned by {:?}",
                    notification_id, existing
                ));
            }

            return Ok(());
        }

        owners.insert(notification_id, owner);

        Ok(())
    }

    fn owner(&self, notification_id: u32) -> Result<Option<NotificationOwner>, String> {
        let owners = self
            .owners
            .read()
            .map_err(|_| "notification ownership registry read lock poisoned".to_string())?;

        Ok(owners.get(&notification_id).cloned())
    }

    fn release(&self, notification_id: u32) -> Result<Option<NotificationOwner>, String> {
        let mut owners = self
            .owners
            .write()
            .map_err(|_| "notification ownership registry write lock poisoned".to_string())?;

        Ok(owners.remove(&notification_id))
    }

    fn validate_module_replace(
        &self,
        notification_id: u32,
        module: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let owner = self.owner(notification_id)?.ok_or_else(|| {
            format!(
                "notification replace_id {} has no active owner",
                notification_id
            )
        })?;

        match owner {
            NotificationOwner::Module {
                module: owner_module,
                session_id: owner_session,
            } if owner_module == module && owner_session == session_id => {
                Ok(())
            }

            NotificationOwner::Module {
                module: owner_module,
                session_id: owner_session,
            } => Err(format!(
                "notification replace_id {} belongs to module '{}' session '{}', not module '{}' session '{}'",
                notification_id,
                owner_module,
                owner_session,
                module,
                session_id
            )),

            NotificationOwner::Boss => Err(format!(
                "notification replace_id {} belongs to Boss",
                notification_id
            )),
        }
    }

    fn release_module_session(&self, module: &str, session_id: &str) -> Result<Vec<u32>, String> {
        let mut owners = self
            .owners
            .write()
            .map_err(|_| "notification ownership registry write lock poisoned".to_string())?;

        let ids = owners
            .iter()
            .filter_map(|(notification_id, owner)| match owner {
                NotificationOwner::Module {
                    module: owner_module,
                    session_id: owner_session,
                } if owner_module == module && owner_session == session_id => {
                    Some(*notification_id)
                }

                _ => None,
            })
            .collect::<Vec<_>>();

        for notification_id in &ids {
            owners.remove(notification_id);
        }

        Ok(ids)
    }
}

static NOTIFICATION_OWNERSHIP: OnceLock<NotificationOwnershipRegistry> = OnceLock::new();

fn notification_ownership() -> &'static NotificationOwnershipRegistry {
    NOTIFICATION_OWNERSHIP.get_or_init(NotificationOwnershipRegistry::default)
}

pub(crate) fn register_module_notification_owner(
    notification_id: u32,
    module: &str,
    session_id: &str,
) -> Result<(), String> {
    notification_ownership().register(
        notification_id,
        NotificationOwner::Module {
            module: module.to_string(),
            session_id: session_id.to_string(),
        },
    )
}

fn register_boss_notification_owner(notification_id: u32) -> Result<(), String> {
    notification_ownership().register(notification_id, NotificationOwner::Boss)
}

pub(crate) fn validate_module_notification_replace_owner(
    notification_id: u32,
    module: &str,
    session_id: &str,
) -> Result<(), String> {
    notification_ownership().validate_module_replace(notification_id, module, session_id)
}

pub(crate) fn release_notification_owner(
    notification_id: u32,
) -> Result<Option<NotificationOwner>, String> {
    notification_ownership().release(notification_id)
}

pub(crate) fn notification_owner(
    notification_id: u32,
) -> Result<Option<NotificationOwner>, String> {
    notification_ownership().owner(notification_id)
}

pub(crate) fn release_module_notification_session(
    module: &str,
    session_id: &str,
) -> Result<Vec<u32>, String> {
    notification_ownership().release_module_session(module, session_id)
}

fn present_owned_for_module<F>(
    module: &str,
    session_id: &str,
    replace_id: Option<u32>,
    present: F,
) -> Result<NotificationOutcome, String>
where
    F: FnOnce() -> Result<NotificationOutcome, String>,
{
    if let Some(replace_id) = replace_id {
        validate_module_notification_replace_owner(replace_id, module, session_id)?;
    }

    let outcome = present()?;

    let NotificationOutcome::Presented(notification_id) = outcome else {
        return Ok(outcome);
    };

    register_module_notification_owner(notification_id, module, session_id)?;

    if let Some(replace_id) = replace_id {
        if replace_id != notification_id {
            let _ = release_notification_owner(replace_id)?;
        }
    }

    Ok(outcome)
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

    fn mandatory(self) -> bool {
        matches!(self, Self::Critical | Self::Fatal)
    }
}

fn desktop_notification_connection_for(
    desktop_session: &neebles_backend::domestic_desktop_session_interface::DesktopSessionInterface,
) -> Result<zbus::blocking::Connection, String> {
    let address = desktop_session.session_bus_address();

    connection::Builder::address(address)
        .map_err(|error| {
            format!(
                "invalid authorized desktop D-Bus address: {error}"
            )
        })?
        .user_id(desktop_session.desktop_uid())
        .build()
        .map_err(|error| {
            format!(
                "could not connect Notifications presenter to authorized desktop session D-Bus: {error}"
            )
        })
}

static NOTIFICATION_RETURN_LISTENER: OnceLock<Result<(), String>> = OnceLock::new();

fn notification_return_event_from_message(
    message: &zbus::message::Message,
) -> Result<Option<NotificationReturnEvent>, String> {
    let header = message.header();

    let Some(member) = header.member() else {
        return Ok(None);
    };

    let event =
        match member.as_str() {
            "NotificationClosed" => {
                let (notification_id, reason) = message
                    .body()
                    .deserialize::<(u32, u32)>()
                    .map_err(|error| {
                        format!("could not decode Plasma NotificationClosed signal: {error}")
                    })?;

                NotificationReturnEvent::Closed {
                    notification_id,
                    reason,
                }
            }

            "ActionInvoked" => {
                let (notification_id, action_key) = message
                    .body()
                    .deserialize::<(u32, String)>()
                    .map_err(|error| {
                    format!("could not decode Plasma ActionInvoked signal: {error}")
                })?;

                NotificationReturnEvent::ActionInvoked {
                    notification_id,
                    action_key,
                }
            }

            "NotificationReplied" => {
                let (notification_id, text) = message
                    .body()
                    .deserialize::<(u32, String)>()
                    .map_err(|error| {
                        format!("could not decode Plasma NotificationReplied signal: {error}")
                    })?;

                NotificationReturnEvent::Replied {
                    notification_id,
                    text,
                }
            }

            "ActivationToken" => {
                let (notification_id, activation_token) = message
                    .body()
                    .deserialize::<(u32, String)>()
                    .map_err(|error| {
                        format!("could not decode Plasma ActivationToken signal: {error}")
                    })?;

                NotificationReturnEvent::ActivationToken {
                    notification_id,
                    activation_token,
                }
            }

            _ => {
                return Ok(None);
            }
        };

    Ok(Some(event))
}

fn run_notification_return_listener(
    address: String,
    desktop_uid: libc::uid_t,
    ready: mpsc::Sender<Result<(), String>>,
) {
    let builder = match connection::Builder::address(address.as_str()) {
        Ok(builder) => builder,

        Err(error) => {
            let message = format!("invalid authorized notification return D-Bus address: {error}");

            let _ = ready.send(Err(message));
            return;
        }
    };

    let connection = match builder.user_id(desktop_uid).build() {
        Ok(connection) => connection,

        Err(error) => {
            let message = format!(
                "could not connect notification return listener to authorized desktop session D-Bus: {error}"
            );

            let _ = ready.send(Err(message));
            return;
        }
    };

    let proxy = match Proxy::new(
        &connection,
        NOTIFICATIONS_SERVICE,
        NOTIFICATIONS_PATH,
        NOTIFICATIONS_INTERFACE,
    ) {
        Ok(proxy) => proxy,

        Err(error) => {
            let message =
                format!("could not create Plasma Notifications return listener proxy: {error}");

            let _ = ready.send(Err(message));
            return;
        }
    };

    let mut signals = match proxy.receive_all_signals() {
        Ok(signals) => signals,

        Err(error) => {
            let message =
                format!("could not subscribe to Plasma Notifications return signals: {error}");

            let _ = ready.send(Err(message));
            return;
        }
    };

    if ready.send(Ok(())).is_err() {
        return;
    }

    for message in &mut signals {
        match notification_return_event_from_message(&message) {
            Ok(Some(event)) => {
                if let Err(error) =
                    crate::module_ipc::server::route_notification_return_event(event)
                {
                    eprintln!("N.E.E.B.L.E.S. notification return routing failed: {error}");
                }
            }

            Ok(None) => {}

            Err(error) => {
                eprintln!("N.E.E.B.L.E.S. notification return signal rejected: {error}");
            }
        }
    }

    eprintln!("N.E.E.B.L.E.S. notification return listener ended");
}

fn ensure_notification_return_listener(
    desktop_session: &neebles_backend::domestic_desktop_session_interface::DesktopSessionInterface,
) -> Result<(), String> {
    NOTIFICATION_RETURN_LISTENER
        .get_or_init(|| {
            let address = desktop_session.session_bus_address().to_string();

            let desktop_uid = desktop_session.desktop_uid();

            let (ready_sender, ready_receiver) = mpsc::channel::<Result<(), String>>();

            thread::Builder::new()
                .name("neebles-notification-return-listener".to_string())
                .spawn(move || {
                    run_notification_return_listener(address, desktop_uid, ready_sender);
                })
                .map_err(|error| {
                    format!("could not spawn notification return listener: {error}")
                })?;

            ready_receiver.recv().map_err(|error| {
                format!("notification return listener startup channel failed: {error}")
            })?
        })
        .clone()
}

fn validate_notification_image_data(image: &NotificationImageData) -> Result<(), String> {
    if image.width <= 0 {
        return Err("notification image-data width must be positive".to_string());
    }

    if image.height <= 0 {
        return Err("notification image-data height must be positive".to_string());
    }

    if image.bits_per_sample != 8 {
        return Err("notification image-data bits_per_sample must be 8".to_string());
    }

    let expected_channels = if image.has_alpha { 4 } else { 3 };

    if image.channels != expected_channels {
        return Err(format!(
            "notification image-data channels must be {} when has_alpha is {}",
            expected_channels, image.has_alpha
        ));
    }

    let minimum_rowstride = image
        .width
        .checked_mul(image.channels)
        .ok_or_else(|| "notification image-data rowstride calculation overflowed".to_string())?;

    if image.rowstride < minimum_rowstride {
        return Err(format!(
            "notification image-data rowstride {} is smaller than minimum {}",
            image.rowstride, minimum_rowstride
        ));
    }

    let expected_bytes = image
        .rowstride
        .checked_mul(image.height)
        .ok_or_else(|| "notification image-data byte-size calculation overflowed".to_string())?;

    let expected_bytes = usize::try_from(expected_bytes)
        .map_err(|_| "notification image-data byte-size is not representable".to_string())?;

    if image.data.len() != expected_bytes {
        return Err(format!(
            "notification image-data contains {} bytes; expected {}",
            image.data.len(),
            expected_bytes
        ));
    }

    Ok(())
}

fn validate_module_notification_interactions(
    actions: &[NotificationAction],
    reply: Option<&NotificationReply>,
) -> Result<(), String> {
    let mut keys = std::collections::BTreeSet::<String>::new();

    for action in actions {
        let key = action.key.trim();

        if key.is_empty() {
            return Err("notification action key cannot be empty".to_string());
        }

        if key == "inline-reply" {
            return Err(
                "notification action key 'inline-reply' is reserved for inline reply".to_string(),
            );
        }

        if action.label.trim().is_empty() {
            return Err(format!(
                "notification action '{}' label cannot be empty",
                action.key
            ));
        }

        if !keys.insert(key.to_string()) {
            return Err(format!(
                "notification action key '{}' is duplicated",
                action.key
            ));
        }
    }

    if let Some(reply) = reply {
        if reply.label.trim().is_empty() {
            return Err("notification inline reply label cannot be empty".to_string());
        }

        for (field, value) in [
            ("placeholder_text", reply.placeholder_text.as_deref()),
            ("submit_button_text", reply.submit_button_text.as_deref()),
            (
                "submit_button_icon_name",
                reply.submit_button_icon_name.as_deref(),
            ),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(format!(
                    "notification inline reply {} cannot be empty when declared",
                    field
                ));
            }
        }
    }

    Ok(())
}

fn notification_actions_for_freedesktop(
    actions: &[NotificationAction],
    reply: Option<&NotificationReply>,
) -> Vec<String> {
    let mut encoded = Vec::with_capacity(actions.len() * 2 + usize::from(reply.is_some()) * 2);

    for action in actions {
        encoded.push(action.key.clone());
        encoded.push(action.label.clone());
    }

    if let Some(reply) = reply {
        encoded.push("inline-reply".to_string());
        encoded.push(reply.label.clone());
    }

    encoded
}

fn validate_module_notification_options(
    module_dir: &std::path::Path,
    options: &NotificationOptions,
) -> Result<NotificationOptions, String> {
    let mut validated = options.clone();

    if let Some(timeout) = validated.expire_timeout_ms {
        if timeout < -1 {
            return Err(format!(
                "notification expire_timeout_ms cannot be less than -1: {timeout}"
            ));
        }
    }

    if let Some(image) = validated.image_data.as_ref() {
        validate_notification_image_data(image)?;
    }

    if let Some(sound_file) = validated.sound_file.as_deref() {
        let canonical =
            modules::resolve_module_file(module_dir, sound_file, "notification sound_file")?;

        validated.sound_file = Some(canonical.to_string_lossy().into_owned());
    }

    if let Some(image_path) = validated.image_path.as_deref() {
        let canonical =
            modules::resolve_module_file(module_dir, image_path, "notification image_path")?;

        validated.image_path = Some(canonical.to_string_lossy().into_owned());
    }

    Ok(validated)
}

fn emit_transport(
    severity: Severity,
    application: &str,
    icon: &std::path::Path,
    options: &NotificationOptions,
    title: &str,
    message: &str,
    notification_actions: &[NotificationAction],
    reply: Option<&NotificationReply>,
) -> Result<NotificationOutcome, String> {
    let config = config::load_or_initialize()?;

    /*
     * Normal notifications obey the user's Boss policy.
     * Critical/Fatal notifications remain mandatory.
     *
     * A suppressed notification is represented explicitly and
     * therefore never fabricates a Plasma notification id.
     */
    if !severity.mandatory() && !config.normal_notifications {
        return Ok(NotificationOutcome::Suppressed);
    }

    let desktop_session =
        neebles_backend::domestic_desktop_session_interface::
            resolve_current_desktop_session_interface()?;

    ensure_notification_return_listener(&desktop_session)?;

    let connection = desktop_notification_connection_for(&desktop_session)?;

    let proxy = Proxy::new(
        &connection,
        NOTIFICATIONS_SERVICE,
        NOTIFICATIONS_PATH,
        NOTIFICATIONS_INTERFACE,
    )
    .map_err(|error| format!("could not create Plasma Notifications proxy: {error}"))?;

    let actions = notification_actions_for_freedesktop(notification_actions, reply);

    let mut hints = HashMap::<String, OwnedValue>::new();

    hints.insert(
        "urgency".to_string(),
        OwnedValue::from(match severity {
            Severity::Info | Severity::Success | Severity::Warning => 1u8,
            Severity::Critical | Severity::Fatal => 2u8,
        }),
    );

    if let Some(category) = options.category.as_deref() {
        hints.insert(
            "category".to_string(),
            OwnedValue::from(Str::from(category)),
        );
    }

    if let Some(desktop_entry) = options.desktop_entry.as_deref() {
        hints.insert(
            "desktop-entry".to_string(),
            OwnedValue::from(Str::from(desktop_entry)),
        );
    }

    if options.resident {
        hints.insert("resident".to_string(), OwnedValue::from(true));
    }

    if options.transient {
        hints.insert("transient".to_string(), OwnedValue::from(true));
    }

    if let Some(sound_name) = options.sound_name.as_deref() {
        hints.insert(
            "sound-name".to_string(),
            OwnedValue::from(Str::from(sound_name)),
        );
    }

    if let Some(sound_file) = options.sound_file.as_deref() {
        hints.insert(
            "sound-file".to_string(),
            OwnedValue::from(Str::from(sound_file)),
        );
    }

    if options.suppress_sound {
        hints.insert("suppress-sound".to_string(), OwnedValue::from(true));
    }

    if let Some(image_path) = options.image_path.as_deref() {
        hints.insert(
            "image-path".to_string(),
            OwnedValue::from(Str::from(image_path)),
        );
    }

    if let Some(image_data) = options.image_data.as_ref() {
        let image_value = Value::new((
            image_data.width,
            image_data.height,
            image_data.rowstride,
            image_data.has_alpha,
            image_data.bits_per_sample,
            image_data.channels,
            image_data.data.clone(),
        ));

        let image_value = OwnedValue::try_from(image_value)
            .map_err(|error| format!("could not encode notification image-data hint: {error}"))?;

        hints.insert("image-data".to_string(), image_value);
    }

    if !options.kde_urls.is_empty() {
        let urls = Array::from(options.kde_urls.clone());

        let urls = OwnedValue::try_from(urls)
            .map_err(|error| format!("could not encode x-kde-urls hint: {error}"))?;

        hints.insert("x-kde-urls".to_string(), urls);
    }

    if let Some(origin_name) = options.kde_origin_name.as_deref() {
        hints.insert(
            "x-kde-origin-name".to_string(),
            OwnedValue::from(Str::from(origin_name)),
        );
    }

    if let Some(display_appname) = options.kde_display_appname.as_deref() {
        hints.insert(
            "x-kde-display-appname".to_string(),
            OwnedValue::from(Str::from(display_appname)),
        );
    }

    if let Some(reply) = reply {
        if let Some(placeholder_text) = reply.placeholder_text.as_deref() {
            hints.insert(
                "x-kde-reply-placeholder-text".to_string(),
                OwnedValue::from(Str::from(placeholder_text)),
            );
        }

        if let Some(submit_button_text) = reply.submit_button_text.as_deref() {
            hints.insert(
                "x-kde-reply-submit-button-text".to_string(),
                OwnedValue::from(Str::from(submit_button_text)),
            );
        }

        if let Some(submit_button_icon_name) = reply.submit_button_icon_name.as_deref() {
            hints.insert(
                "x-kde-reply-submit-button-icon-name".to_string(),
                OwnedValue::from(Str::from(submit_button_icon_name)),
            );
        }
    }

    let icon = icon.to_string_lossy().into_owned();

    proxy
        .call(
            "Notify",
            &(
                application,
                options.replace_id.unwrap_or(0),
                icon.as_str(),
                title,
                message,
                actions,
                hints,
                options.expire_timeout_ms.unwrap_or(-1),
            ),
        )
        .map(NotificationOutcome::Presented)
        .map_err(|error| format!("Plasma Notify call failed: {error}"))
}

pub fn emit(severity: Severity, title: &str, message: &str) -> Result<(), String> {
    let icon =
        crate::languages::client_root()?.join("assets/branding/neebles-boss-launcher-icon.png");

    let options = NotificationOptions {
        expire_timeout_ms: Some(DEFAULT_NOTIFICATION_EXPIRE_MS),
        ..NotificationOptions::default()
    };

    let outcome = emit_transport(
        severity,
        "N.E.E.B.L.E.S.",
        &icon,
        &options,
        title,
        message,
        &[],
        None,
    )?;

    if let NotificationOutcome::Presented(notification_id) = outcome {
        register_boss_notification_owner(notification_id)?;
    }

    Ok(())
}

pub(crate) fn emit_default_for_module_owned(
    module: &str,
    session_id: &str,
    severity: Severity,
    icon: &str,
    title: &str,
    message: &str,
    expire_timeout_ms: Option<i32>,
    replace_id: Option<u32>,
) -> Result<NotificationOutcome, String> {
    present_owned_for_module(module, session_id, replace_id, || {
        emit_default_for_module(
            module,
            severity,
            icon,
            title,
            message,
            expire_timeout_ms,
            replace_id,
        )
    })
}

pub fn emit_default_for_module(
    module: &str,
    severity: Severity,
    icon: &str,
    title: &str,
    message: &str,
    expire_timeout_ms: Option<i32>,
    replace_id: Option<u32>,
) -> Result<NotificationOutcome, String> {
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

    if icon.trim().is_empty() {
        return Err(format!(
            "module '{}' default notification icon cannot be empty",
            module
        ));
    }

    if title.trim().is_empty() {
        return Err(format!(
            "module '{}' default notification title cannot be empty",
            module
        ));
    }

    if message.trim().is_empty() {
        return Err(format!(
            "module '{}' default notification message cannot be empty",
            module
        ));
    }

    let module_dir = modules::installed_module_dir(module)?;

    let canonical_icon =
        modules::resolve_module_file(&module_dir, icon, "default notification icon")?;

    let options = NotificationOptions {
        replace_id,
        expire_timeout_ms: Some(expire_timeout_ms.unwrap_or(DEFAULT_NOTIFICATION_EXPIRE_MS)),
        ..NotificationOptions::default()
    };

    let options = validate_module_notification_options(&module_dir, &options)?;

    emit_transport(
        severity,
        &format!("N.E.E.B.L.E.S. · {}", manifest.name),
        &canonical_icon,
        &options,
        title,
        message,
        &[],
        None,
    )
}

pub(crate) fn emit_for_module_with_identity_owned(
    module: &str,
    session_id: &str,
    severity: Severity,
    presentation: &NotificationPresentation,
) -> Result<NotificationOutcome, String> {
    let replace_id = presentation.options.replace_id;

    present_owned_for_module(module, session_id, replace_id, || {
        emit_for_module_with_identity(module, severity, presentation)
    })
}

pub fn emit_for_module_with_identity(
    module: &str,
    severity: Severity,
    presentation: &NotificationPresentation,
) -> Result<NotificationOutcome, String> {
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
     * The module owns its notification presentation identity
     * and human-readable content.
     *
     * Boss governs capability, authenticated transport and policy.
     */
    if presentation.application.trim().is_empty() {
        return Err(format!(
            "module '{}' notification application cannot be empty",
            module
        ));
    }

    if presentation.icon.trim().is_empty() {
        return Err(format!(
            "module '{}' notification icon cannot be empty",
            module
        ));
    }

    let module_dir = modules::installed_module_dir(module)?;

    let canonical_icon =
        modules::resolve_module_file(&module_dir, &presentation.icon, "notification icon")?;

    let options = validate_module_notification_options(&module_dir, &presentation.options)?;

    validate_module_notification_interactions(&presentation.actions, presentation.reply.as_ref())?;

    emit_transport(
        severity,
        &presentation.application,
        &canonical_icon,
        &options,
        &presentation.title,
        &presentation.message,
        &presentation.actions,
        presentation.reply.as_ref(),
    )
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

    let icon =
        crate::languages::client_root()?.join("assets/branding/neebles-boss-launcher-icon.png");

    emit_transport(
        severity,
        &format!("N.E.E.B.L.E.S. · {}", manifest.name),
        &icon,
        &NotificationOptions::default(),
        title,
        message,
        &[],
        None,
    )
    .map(|_| ())
}

#[cfg(test)]
mod rich_notification_policy_tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_module_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "neebles-notification-policy-{}-{}-{}",
            std::process::id(),
            unique,
            label
        ));

        fs::create_dir_all(&path).unwrap();

        path
    }

    fn valid_image_data() -> NotificationImageData {
        NotificationImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![10, 20, 30, 255],
        }
    }

    #[test]
    fn image_data_accepts_valid_rgba_payload() {
        validate_notification_image_data(&valid_image_data()).unwrap();
    }

    #[test]
    fn image_data_rejects_channel_alpha_mismatch() {
        let mut image = valid_image_data();

        image.channels = 3;

        let error = validate_notification_image_data(&image).unwrap_err();

        assert!(error.contains("channels"));
    }

    #[test]
    fn image_data_rejects_inconsistent_byte_length() {
        let mut image = valid_image_data();

        image.data.pop();

        let error = validate_notification_image_data(&image).unwrap_err();

        assert!(error.contains("expected"));
    }

    #[test]
    fn rich_actions_flatten_to_freedesktop_key_label_pairs() {
        let actions = vec![
            NotificationAction {
                key: "open".to_string(),
                label: "Open".to_string(),
            },
            NotificationAction {
                key: "details".to_string(),
                label: "Details".to_string(),
            },
        ];

        let encoded = notification_actions_for_freedesktop(&actions, None);

        assert_eq!(
            encoded,
            vec![
                "open".to_string(),
                "Open".to_string(),
                "details".to_string(),
                "Details".to_string(),
            ]
        );
    }

    #[test]
    fn rich_inline_reply_appends_reserved_freedesktop_action() {
        let reply = NotificationReply {
            label: "Reply".to_string(),
            placeholder_text: Some("Write a reply".to_string()),
            submit_button_text: Some("Send".to_string()),
            submit_button_icon_name: Some("mail-send".to_string()),
        };

        validate_module_notification_interactions(&[], Some(&reply)).unwrap();

        assert_eq!(
            notification_actions_for_freedesktop(&[], Some(&reply)),
            vec!["inline-reply".to_string(), "Reply".to_string()]
        );
    }

    #[test]
    fn rich_actions_reject_reserved_inline_reply_key() {
        let actions = vec![NotificationAction {
            key: "inline-reply".to_string(),
            label: "Fake reply".to_string(),
        }];

        let error = validate_module_notification_interactions(&actions, None).unwrap_err();

        assert!(error.contains("reserved"));
    }

    #[test]
    fn rich_actions_reject_duplicate_keys() {
        let actions = vec![
            NotificationAction {
                key: "open".to_string(),
                label: "Open".to_string(),
            },
            NotificationAction {
                key: "open".to_string(),
                label: "Open again".to_string(),
            },
        ];

        let error = validate_module_notification_interactions(&actions, None).unwrap_err();

        assert!(error.contains("duplicated"));
    }

    #[test]
    fn rich_interactions_reject_empty_action_and_reply_labels() {
        let action_error = validate_module_notification_interactions(
            &[NotificationAction {
                key: "open".to_string(),
                label: "   ".to_string(),
            }],
            None,
        )
        .unwrap_err();

        assert!(action_error.contains("label cannot be empty"));

        let reply = NotificationReply {
            label: "   ".to_string(),
            placeholder_text: None,
            submit_button_text: None,
            submit_button_icon_name: None,
        };

        let reply_error = validate_module_notification_interactions(&[], Some(&reply)).unwrap_err();

        assert!(reply_error.contains("reply label cannot be empty"));
    }

    #[test]
    fn rich_options_reject_timeout_below_freedesktop_default_sentinel() {
        let module_dir = temporary_module_dir("timeout");

        let options = NotificationOptions {
            expire_timeout_ms: Some(-2),
            ..NotificationOptions::default()
        };

        let error = validate_module_notification_options(&module_dir, &options).unwrap_err();

        assert!(error.contains("cannot be less than -1"));

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn rich_options_canonicalize_owned_sound_and_image_assets() {
        let module_dir = temporary_module_dir("assets");

        fs::create_dir_all(module_dir.join("assets")).unwrap();

        fs::write(
            module_dir.join("assets").join("notification.ogg"),
            b"synthetic sound",
        )
        .unwrap();

        fs::write(
            module_dir.join("assets").join("notification.png"),
            b"synthetic image",
        )
        .unwrap();

        let options = NotificationOptions {
            sound_file: Some("assets/notification.ogg".to_string()),
            image_path: Some("assets/notification.png".to_string()),
            ..NotificationOptions::default()
        };

        let validated = validate_module_notification_options(&module_dir, &options).unwrap();

        let sound = PathBuf::from(validated.sound_file.expect("sound_file expected"));

        let image = PathBuf::from(validated.image_path.expect("image_path expected"));

        assert!(sound.is_absolute());
        assert!(image.is_absolute());

        assert!(sound.starts_with(module_dir.canonicalize().unwrap()));
        assert!(image.starts_with(module_dir.canonicalize().unwrap()));

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn rich_options_reject_asset_escape() {
        let module_dir = temporary_module_dir("escape");

        let options = NotificationOptions {
            image_path: Some("../outside.png".to_string()),
            ..NotificationOptions::default()
        };

        let error = validate_module_notification_options(&module_dir, &options).unwrap_err();

        assert!(
            error.contains("invalid path")
                || error.contains("must be relative")
                || error.contains("escapes")
        );

        fs::remove_dir_all(module_dir).unwrap();
    }
}

#[cfg(test)]
mod notification_ownership_tests {
    use super::*;

    fn registry() -> NotificationOwnershipRegistry {
        NotificationOwnershipRegistry::default()
    }

    fn module_owner(module: &str, session_id: &str) -> NotificationOwner {
        NotificationOwner::Module {
            module: module.to_string(),
            session_id: session_id.to_string(),
        }
    }

    #[test]
    fn ownership_registers_exact_module_session() {
        let registry = registry();

        registry
            .register(41, module_owner("alpha", "session-a"))
            .unwrap();

        assert_eq!(
            registry.owner(41).unwrap(),
            Some(module_owner("alpha", "session-a"))
        );
    }

    #[test]
    fn ownership_rejects_zero_id() {
        let registry = registry();

        let error = registry
            .register(0, module_owner("alpha", "session-a"))
            .expect_err("id zero must never become owned");

        assert!(error.contains("id 0"));
    }

    #[test]
    fn ownership_does_not_transfer_between_modules() {
        let registry = registry();

        registry
            .register(41, module_owner("alpha", "session-a"))
            .unwrap();

        let error = registry
            .register(41, module_owner("beta", "session-b"))
            .expect_err("active notification ownership must not transfer");

        assert!(error.contains("already owned"));

        assert_eq!(
            registry.owner(41).unwrap(),
            Some(module_owner("alpha", "session-a"))
        );
    }

    #[test]
    fn same_owner_can_register_same_notification_again() {
        let registry = registry();

        let owner = module_owner("alpha", "session-a");

        registry.register(41, owner.clone()).unwrap();

        registry.register(41, owner.clone()).unwrap();

        assert_eq!(registry.owner(41).unwrap(), Some(owner));
    }

    #[test]
    fn replace_requires_exact_module_and_session_owner() {
        let registry = registry();

        registry
            .register(77, module_owner("alpha", "session-a"))
            .unwrap();

        registry
            .validate_module_replace(77, "alpha", "session-a")
            .unwrap();

        assert!(registry
            .validate_module_replace(77, "alpha", "session-old",)
            .is_err());

        assert!(registry
            .validate_module_replace(77, "beta", "session-b",)
            .is_err());
    }

    #[test]
    fn replace_rejects_unowned_notification() {
        let registry = registry();

        let error = registry
            .validate_module_replace(999, "alpha", "session-a")
            .expect_err("unowned notification cannot be replaced");

        assert!(error.contains("no active owner"));
    }

    #[test]
    fn session_cleanup_removes_only_exact_runtime_incarnation() {
        let registry = registry();

        registry
            .register(11, module_owner("alpha", "session-old"))
            .unwrap();

        registry
            .register(12, module_owner("alpha", "session-current"))
            .unwrap();

        registry
            .register(13, module_owner("beta", "session-b"))
            .unwrap();

        let released = registry
            .release_module_session("alpha", "session-old")
            .unwrap();

        assert_eq!(released, vec![11]);

        assert_eq!(registry.owner(11).unwrap(), None);

        assert_eq!(
            registry.owner(12).unwrap(),
            Some(module_owner("alpha", "session-current"))
        );

        assert_eq!(
            registry.owner(13).unwrap(),
            Some(module_owner("beta", "session-b"))
        );
    }

    #[test]
    fn closed_notification_release_returns_previous_owner() {
        let registry = registry();

        let owner = module_owner("alpha", "session-a");

        registry.register(88, owner.clone()).unwrap();

        assert_eq!(registry.release(88).unwrap(), Some(owner));

        assert_eq!(registry.owner(88).unwrap(), None);
    }

    #[test]
    fn productive_owner_registers_presented_id() {
        let notification_id = 4_500_001;

        let outcome =
            present_owned_for_module("productive-alpha", "session-productive-a", None, || {
                Ok(NotificationOutcome::Presented(notification_id))
            })
            .unwrap();

        assert_eq!(outcome, NotificationOutcome::Presented(notification_id));

        assert_eq!(
            notification_ownership().owner(notification_id).unwrap(),
            Some(module_owner("productive-alpha", "session-productive-a"))
        );

        let _ = release_notification_owner(notification_id).unwrap();
    }

    #[test]
    fn productive_replace_rejects_foreign_owner_before_presenter() {
        let notification_id = 4_500_002;

        register_module_notification_owner(
            notification_id,
            "productive-alpha",
            "session-productive-a",
        )
        .unwrap();

        let mut presenter_called = false;

        let error = present_owned_for_module(
            "productive-beta",
            "session-productive-b",
            Some(notification_id),
            || {
                presenter_called = true;
                Ok(NotificationOutcome::Presented(notification_id))
            },
        )
        .expect_err("foreign replace must be rejected");

        assert!(!presenter_called);
        assert!(error.contains("belongs to module"));

        let _ = release_notification_owner(notification_id).unwrap();
    }

    #[test]
    fn productive_replace_moves_owner_when_presenter_returns_new_id() {
        let old_id = 4_500_003;
        let new_id = 4_500_004;

        register_module_notification_owner(old_id, "productive-alpha", "session-productive-a")
            .unwrap();

        let outcome = present_owned_for_module(
            "productive-alpha",
            "session-productive-a",
            Some(old_id),
            || Ok(NotificationOutcome::Presented(new_id)),
        )
        .unwrap();

        assert_eq!(outcome, NotificationOutcome::Presented(new_id));

        assert_eq!(notification_ownership().owner(old_id).unwrap(), None);

        assert_eq!(
            notification_ownership().owner(new_id).unwrap(),
            Some(module_owner("productive-alpha", "session-productive-a"))
        );

        let _ = release_notification_owner(new_id).unwrap();
    }

    #[test]
    fn suppressed_replace_keeps_existing_owner() {
        let notification_id = 4_500_005;

        register_module_notification_owner(
            notification_id,
            "productive-alpha",
            "session-productive-a",
        )
        .unwrap();

        let outcome = present_owned_for_module(
            "productive-alpha",
            "session-productive-a",
            Some(notification_id),
            || Ok(NotificationOutcome::Suppressed),
        )
        .unwrap();

        assert_eq!(outcome, NotificationOutcome::Suppressed);

        assert_eq!(
            notification_ownership().owner(notification_id).unwrap(),
            Some(module_owner("productive-alpha", "session-productive-a"))
        );

        let _ = release_notification_owner(notification_id).unwrap();
    }

    #[test]
    fn boss_owned_notification_cannot_be_replaced_by_module() {
        let registry = registry();

        registry.register(90, NotificationOwner::Boss).unwrap();

        let error = registry
            .validate_module_replace(90, "alpha", "session-a")
            .expect_err("module must not replace Boss notification");

        assert!(error.contains("belongs to Boss"));
    }
}
