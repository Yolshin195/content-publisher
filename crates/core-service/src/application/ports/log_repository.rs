use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::{DomainError, LogStatus, PublicationLog};

#[async_trait]
pub trait LogRepository: Send + Sync {
    async fn record(
        &self,
        task_id: Uuid,
        attempt_no: i32,
        status: LogStatus,
        error_message: Option<String>,
        gateway_response: Option<Value>,
    ) -> Result<PublicationLog, DomainError>;
    async fn list_by_task(&self, task_id: Uuid) -> Result<Vec<PublicationLog>, DomainError>;
    async fn list_recent(&self, status: Option<LogStatus>, limit: i64) -> Result<Vec<PublicationLog>, DomainError>;
}
