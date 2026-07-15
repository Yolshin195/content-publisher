use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::LogRepository;
use crate::domain::{DomainError, LogStatus, PublicationLog};

pub struct PgLogRepository {
    pool: PgPool,
}

impl PgLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const LOG_COLUMNS: &str = "id, task_id, attempt_no, status, error_message, gateway_response, attempted_at";

fn row_to_log(row: &sqlx::postgres::PgRow) -> Result<PublicationLog, DomainError> {
    let status_str: String = row.try_get("status")?;
    Ok(PublicationLog {
        id: row.try_get("id")?,
        task_id: row.try_get("task_id")?,
        attempt_no: row.try_get("attempt_no")?,
        status: if status_str == "success" { LogStatus::Success } else { LogStatus::Failure },
        error_message: row.try_get("error_message")?,
        gateway_response: row.try_get("gateway_response")?,
        attempted_at: row.try_get("attempted_at")?,
    })
}

#[async_trait]
impl LogRepository for PgLogRepository {
    async fn record(
        &self,
        task_id: Uuid,
        attempt_no: i32,
        status: LogStatus,
        error_message: Option<String>,
        gateway_response: Option<Value>,
    ) -> Result<PublicationLog, DomainError> {
        let sql = format!(
            "INSERT INTO publication_logs (task_id, attempt_no, status, error_message, gateway_response) \
             VALUES ($1, $2, $3, $4, $5) RETURNING {LOG_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(task_id)
            .bind(attempt_no)
            .bind(status.as_str())
            .bind(&error_message)
            .bind(&gateway_response)
            .fetch_one(&self.pool)
            .await?;
        row_to_log(&row)
    }

    async fn list_by_task(&self, task_id: Uuid) -> Result<Vec<PublicationLog>, DomainError> {
        let sql = format!("SELECT {LOG_COLUMNS} FROM publication_logs WHERE task_id = $1 ORDER BY attempted_at DESC");
        let rows = sqlx::query(&sql).bind(task_id).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_log).collect()
    }

    async fn list_recent(&self, status: Option<LogStatus>, limit: i64) -> Result<Vec<PublicationLog>, DomainError> {
        let status_str = status.map(|s| s.as_str().to_string());
        let sql = format!(
            "SELECT {LOG_COLUMNS} FROM publication_logs \
             WHERE ($1::text IS NULL OR status = $1) ORDER BY attempted_at DESC LIMIT $2"
        );
        let rows = sqlx::query(&sql).bind(&status_str).bind(limit).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_log).collect()
    }
}
