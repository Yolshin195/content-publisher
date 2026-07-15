use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStatus {
    Success,
    Failure,
}

impl LogStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogStatus::Success => "success",
            LogStatus::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationLog {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: i32,
    pub status: LogStatus,
    pub error_message: Option<String>,
    pub gateway_response: Option<Value>,
    pub attempted_at: DateTime<Utc>,
}
