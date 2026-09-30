use crate::contracts::{InvocationLifecycle, StateMode};

use serde::{Deserialize, Serialize};

use serde_json::Value;

use std::collections::BTreeMap;

pub const MODULES_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleNotificationImageData {
    pub width: i32,
    pub height: i32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleNotificationOptions {
    #[serde(default)]
    pub replace_id: Option<u32>,

    #[serde(default)]
    pub expire_timeout_ms: Option<i32>,

    #[serde(default)]
    pub category: Option<String>,

    #[serde(default)]
    pub desktop_entry: Option<String>,

    #[serde(default)]
    pub resident: bool,

    #[serde(default)]
    pub transient: bool,

    #[serde(default)]
    pub sound_name: Option<String>,

    #[serde(default)]
    pub sound_file: Option<String>,

    #[serde(default)]
    pub suppress_sound: bool,

    #[serde(default)]
    pub image_path: Option<String>,

    #[serde(default)]
    pub image_data: Option<ModuleNotificationImageData>,

    #[serde(default)]
    pub kde_urls: Vec<String>,

    #[serde(default)]
    pub kde_origin_name: Option<String>,

    #[serde(default)]
    pub kde_display_appname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleNotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleNotificationReply {
    pub label: String,

    #[serde(default)]
    pub placeholder_text: Option<String>,

    #[serde(default)]
    pub submit_button_text: Option<String>,

    #[serde(default)]
    pub submit_button_icon_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleNotificationPresentation {
    pub application: String,
    pub icon: String,
    pub title: String,
    pub message: String,

    #[serde(default)]
    pub actions: Vec<ModuleNotificationAction>,

    #[serde(default)]
    pub reply: Option<ModuleNotificationReply>,

    #[serde(default)]
    pub options: ModuleNotificationOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleError {
    pub kind: String,
    pub message: String,

    #[serde(default)]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModuleMessage {
    Register {
        protocol: u32,
        module: String,
        session_id: String,

        /*
         * Endpoints que el runtime declara haber cargado
         * realmente.
         *
         * Ej:
         * commands -> ["gradient.create", "version"]
         */
        #[serde(default)]
        endpoints: BTreeMap<String, Vec<String>>,
    },

    Registered {
        protocol: u32,
        module: String,
        session_id: String,
    },

    Subscribe {
        module: String,
        session_id: String,

        #[serde(default)]
        topics: Vec<String>,
    },

    Subscribed {
        module: String,
        session_id: String,
        topics: Vec<String>,
    },

    Event {
        module: String,
        session_id: String,
        topic: String,
        event: String,

        #[serde(default)]
        payload: Value,
    },

    SettingsGet {
        id: String,
        module: String,
        session_id: String,
        path: String,
    },

    SettingsSet {
        id: String,
        module: String,
        session_id: String,
        path: String,
        value: String,
    },

    SettingsValue {
        id: String,
        module: String,
        session_id: String,
        path: String,
        value: String,
    },

    DefaultNotification {
        id: String,
        module: String,
        session_id: String,
        severity: String,
        icon: String,
        title: String,
        message: String,

        #[serde(default)]
        expire_timeout_ms: Option<i32>,

        #[serde(default)]
        replace_id: Option<u32>,
    },

    Notification {
        id: String,
        module: String,
        session_id: String,
        severity: String,
        presentation: ModuleNotificationPresentation,
    },

    NotificationAck {
        id: String,
        module: String,
        session_id: String,

        #[serde(default)]
        notification_id: Option<u32>,
    },

    NotificationClosed {
        module: String,
        session_id: String,
        notification_id: u32,
        reason: u32,
    },

    NotificationActionInvoked {
        module: String,
        session_id: String,
        notification_id: u32,
        action_key: String,
    },

    NotificationReplied {
        module: String,
        session_id: String,
        notification_id: u32,
        text: String,
    },

    NotificationActivationToken {
        module: String,
        session_id: String,
        notification_id: u32,
        activation_token: String,
    },

    Invoke {
        id: String,
        module: String,
        session_id: String,

        contract: String,
        endpoint: String,

        lifecycle: InvocationLifecycle,
        state_mode: StateMode,

        #[serde(default)]
        args: Vec<String>,

        #[serde(default)]
        payload: Option<Value>,

        #[serde(default)]
        context: BTreeMap<String, Value>,
    },

    Response {
        id: String,
        module: String,
        session_id: String,

        contract: String,
        endpoint: String,

        ok: bool,
        code: i32,

        #[serde(default)]
        result: Option<Value>,

        #[serde(default)]
        error: Option<ModuleError>,
    },

    Shutdown {
        module: String,
        session_id: String,

        #[serde(default)]
        reason: Option<String>,
    },

    ShutdownAck {
        module: String,
        session_id: String,
    },

    Unregister {
        module: String,
        session_id: String,

        #[serde(default)]
        reason: Option<String>,
    },

    Ping {
        module: String,
        session_id: String,
    },

    Pong {
        module: String,
        session_id: String,
    },

    Error {
        #[serde(default)]
        id: Option<String>,

        #[serde(default)]
        module: Option<String>,

        error: ModuleError,
    },
}

#[cfg(test)]
mod notification_wire_tests {
    use super::*;

    #[test]
    fn notification_return_wire_round_trip_preserves_all_signal_payloads() {
        let messages = vec![
            ModuleMessage::NotificationClosed {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                notification_id: 41,
                reason: 2,
            },
            ModuleMessage::NotificationActionInvoked {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                notification_id: 42,
                action_key: "open".to_string(),
            },
            ModuleMessage::NotificationReplied {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                notification_id: 43,
                text: "hello".to_string(),
            },
            ModuleMessage::NotificationActivationToken {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                notification_id: 44,
                activation_token: "token-44".to_string(),
            },
        ];

        for message in messages {
            let encoded = serde_json::to_string(&message)
                .expect("notification return message must serialize");

            let decoded: ModuleMessage = serde_json::from_str(&encoded)
                .expect("notification return message must deserialize");

            match decoded {
                ModuleMessage::NotificationClosed {
                    module,
                    session_id,
                    notification_id,
                    reason,
                } => {
                    assert_eq!(module, "alpha");
                    assert_eq!(session_id, "session-a");
                    assert_eq!(notification_id, 41);
                    assert_eq!(reason, 2);
                }

                ModuleMessage::NotificationActionInvoked {
                    module,
                    session_id,
                    notification_id,
                    action_key,
                } => {
                    assert_eq!(module, "alpha");
                    assert_eq!(session_id, "session-a");
                    assert_eq!(notification_id, 42);
                    assert_eq!(action_key, "open");
                }

                ModuleMessage::NotificationReplied {
                    module,
                    session_id,
                    notification_id,
                    text,
                } => {
                    assert_eq!(module, "alpha");
                    assert_eq!(session_id, "session-a");
                    assert_eq!(notification_id, 43);
                    assert_eq!(text, "hello");
                }

                ModuleMessage::NotificationActivationToken {
                    module,
                    session_id,
                    notification_id,
                    activation_token,
                } => {
                    assert_eq!(module, "alpha");
                    assert_eq!(session_id, "session-a");
                    assert_eq!(notification_id, 44);
                    assert_eq!(activation_token, "token-44");
                }

                other => panic!("unexpected notification return variant: {other:?}"),
            }
        }
    }

    #[test]
    fn default_notification_wire_round_trip_preserves_overrides() {
        let message = ModuleMessage::DefaultNotification {
            id: "wire-default".to_string(),
            module: "alpha".to_string(),
            session_id: "session-a".to_string(),
            severity: "info".to_string(),
            icon: "assets/alpha.png".to_string(),
            title: "Default title".to_string(),
            message: "Default body".to_string(),
            expire_timeout_ms: Some(4500),
            replace_id: Some(77),
        };

        let encoded = serde_json::to_string(&message).expect("default notification must serialize");

        let decoded: ModuleMessage =
            serde_json::from_str(&encoded).expect("default notification must deserialize");

        match decoded {
            ModuleMessage::DefaultNotification {
                id,
                module,
                session_id,
                severity,
                icon,
                title,
                message,
                expire_timeout_ms,
                replace_id,
            } => {
                assert_eq!(id, "wire-default");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(severity, "info");
                assert_eq!(icon, "assets/alpha.png");
                assert_eq!(title, "Default title");
                assert_eq!(message, "Default body");
                assert_eq!(expire_timeout_ms, Some(4500));
                assert_eq!(replace_id, Some(77));
            }

            other => panic!("expected DefaultNotification, got {other:?}"),
        }
    }

    #[test]
    fn rich_notification_wire_round_trip_preserves_nested_presentation() {
        let message = ModuleMessage::Notification {
            id: "wire-rich".to_string(),
            module: "alpha".to_string(),
            session_id: "session-a".to_string(),
            severity: "success".to_string(),
            presentation: ModuleNotificationPresentation {
                application: "Alpha".to_string(),
                icon: "assets/alpha.png".to_string(),
                title: "Rich title".to_string(),
                message: "Rich body".to_string(),
                actions: vec![
                    ModuleNotificationAction {
                        key: "open".to_string(),
                        label: "Open".to_string(),
                    },
                    ModuleNotificationAction {
                        key: "dismiss-later".to_string(),
                        label: "Later".to_string(),
                    },
                ],
                reply: Some(ModuleNotificationReply {
                    label: "Reply".to_string(),
                    placeholder_text: Some("Write a reply".to_string()),
                    submit_button_text: Some("Send".to_string()),
                    submit_button_icon_name: Some("mail-send".to_string()),
                }),
                options: ModuleNotificationOptions {
                    replace_id: Some(41),
                    expire_timeout_ms: Some(9000),
                    category: Some("transfer".to_string()),
                    resident: true,
                    kde_urls: vec!["https://example.invalid".to_string()],
                    ..ModuleNotificationOptions::default()
                },
            },
        };

        let encoded = serde_json::to_string(&message).expect("rich notification must serialize");

        let decoded: ModuleMessage =
            serde_json::from_str(&encoded).expect("rich notification must deserialize");

        match decoded {
            ModuleMessage::Notification {
                id,
                module,
                session_id,
                severity,
                presentation,
            } => {
                assert_eq!(id, "wire-rich");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(severity, "success");
                assert_eq!(presentation.application, "Alpha");
                assert_eq!(presentation.icon, "assets/alpha.png");
                assert_eq!(presentation.title, "Rich title");
                assert_eq!(presentation.message, "Rich body");
                assert_eq!(presentation.actions.len(), 2);
                assert_eq!(presentation.actions[0].key, "open");
                assert_eq!(presentation.actions[0].label, "Open");
                assert_eq!(presentation.actions[1].key, "dismiss-later");
                assert_eq!(presentation.actions[1].label, "Later");

                let reply = presentation.reply.expect("rich inline reply expected");
                assert_eq!(reply.label, "Reply");
                assert_eq!(reply.placeholder_text.as_deref(), Some("Write a reply"));
                assert_eq!(reply.submit_button_text.as_deref(), Some("Send"));
                assert_eq!(reply.submit_button_icon_name.as_deref(), Some("mail-send"));

                assert_eq!(presentation.options.replace_id, Some(41));
                assert_eq!(presentation.options.expire_timeout_ms, Some(9000));
                assert_eq!(presentation.options.category.as_deref(), Some("transfer"));
                assert!(presentation.options.resident);
                assert_eq!(
                    presentation.options.kde_urls,
                    vec!["https://example.invalid".to_string()]
                );
            }

            other => panic!("expected Notification, got {other:?}"),
        }
    }
}
