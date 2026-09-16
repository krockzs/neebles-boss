use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    #[serde(default = "default_caller")]
    pub caller: String,
}

fn default_caller() -> String {
    "cli".to_string()
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            caller: default_caller(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub target: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub context: ExecutionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    pub ok: bool,
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BossStreamMessage {
    Subscribed {
        topics: Vec<String>,
    },
    Event {
        topic: String,
        event: String,
        payload: Value,
    },
}

impl ExecutionResponse {
    pub fn ok(result: Option<Value>) -> Self {
        Self {
            ok: true,
            code: 0,
            result,
            error: None,
        }
    }

    pub fn fail(code: i32, kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            code,
            result: None,
            error: Some(ExecutionError {
                kind: kind.into(),
                message: message.into(),
            }),
        }
    }
}
