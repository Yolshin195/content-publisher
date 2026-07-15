use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Publishing,
    Published,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Publishing => "publishing",
            TaskStatus::Published => "published",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "publishing" => Ok(TaskStatus::Publishing),
            "published" => Ok(TaskStatus::Published),
            "failed" => Ok(TaskStatus::Failed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            other => Err(DomainError::Validation(format!("unknown task status: {other}"))),
        }
    }
}

/// Одна запись на пару (статья, целевая площадка). Именно здесь живёт факт
/// публикации и её результат — статус публикации никогда не хранится на самой статье.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationTask {
    pub id: Uuid,
    pub article_id: Uuid,
    pub target_id: Uuid,
    pub status: TaskStatus,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub external_post_id: Option<String>,
    pub permalink: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
