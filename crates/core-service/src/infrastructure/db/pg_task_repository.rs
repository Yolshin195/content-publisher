use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::TaskRepository;
use crate::domain::{DomainError, PublicationTask, TaskStatus};

pub struct PgTaskRepository {
    pool: PgPool,
}

impl PgTaskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const TASK_COLUMNS: &str = "id, article_id, target_id, status, attempts, last_error, \
     external_post_id, permalink, published_at, created_at, updated_at";

fn row_to_task(row: &sqlx::postgres::PgRow) -> Result<PublicationTask, DomainError> {
    let status_str: String = row.try_get("status")?;
    Ok(PublicationTask {
        id: row.try_get("id")?,
        article_id: row.try_get("article_id")?,
        target_id: row.try_get("target_id")?,
        status: TaskStatus::parse(&status_str)?,
        attempts: row.try_get("attempts")?,
        last_error: row.try_get("last_error")?,
        external_post_id: row.try_get("external_post_id")?,
        permalink: row.try_get("permalink")?,
        published_at: row.try_get("published_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[async_trait]
impl TaskRepository for PgTaskRepository {
    async fn create(&self, article_id: Uuid, target_id: Uuid) -> Result<PublicationTask, DomainError> {
        let sql = format!(
            "INSERT INTO publication_tasks (article_id, target_id) VALUES ($1, $2) \
             ON CONFLICT (article_id, target_id) DO UPDATE \
               SET status = 'pending', attempts = 0, last_error = NULL, updated_at = now() \
             RETURNING {TASK_COLUMNS}"
        );
        let row = sqlx::query(&sql).bind(article_id).bind(target_id).fetch_one(&self.pool).await?;
        row_to_task(&row)
    }

    async fn get(&self, id: Uuid) -> Result<PublicationTask, DomainError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM publication_tasks WHERE id = $1");
        let row = sqlx::query(&sql).bind(id).fetch_optional(&self.pool).await?.ok_or(DomainError::TaskNotFound)?;
        row_to_task(&row)
    }

    async fn get_by_article_and_target(&self, article_id: Uuid, target_id: Uuid) -> Result<PublicationTask, DomainError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM publication_tasks WHERE article_id = $1 AND target_id = $2");
        let row = sqlx::query(&sql)
            .bind(article_id)
            .bind(target_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::TaskNotFound)?;
        row_to_task(&row)
    }

    async fn list_by_article(&self, article_id: Uuid) -> Result<Vec<PublicationTask>, DomainError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM publication_tasks WHERE article_id = $1 ORDER BY created_at ASC");
        let rows = sqlx::query(&sql).bind(article_id).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_task).collect()
    }

    async fn fetch_due(&self, limit: i64) -> Result<Vec<(PublicationTask, DateTime<Utc>)>, DomainError> {
        let cols: Vec<String> = TASK_COLUMNS.split(',').map(|c| format!("t.{}", c.trim())).collect();
        let sql = format!(
            "SELECT {}, a.scheduled_at as article_scheduled_at \
             FROM publication_tasks t \
             JOIN articles a ON a.id = t.article_id \
             WHERE t.status = 'pending' AND a.scheduled_at IS NOT NULL AND a.scheduled_at <= now() AND a.deleted_at IS NULL \
             ORDER BY a.scheduled_at ASC \
             LIMIT $1 \
             FOR UPDATE OF t SKIP LOCKED",
            cols.join(", ")
        );
        let rows = sqlx::query(&sql).bind(limit).fetch_all(&self.pool).await?;

        rows.iter()
            .map(|row| {
                let task = row_to_task(row)?;
                let scheduled_at: DateTime<Utc> = row.try_get("article_scheduled_at")?;
                Ok((task, scheduled_at))
            })
            .collect()
    }

    async fn mark_publishing(&self, id: Uuid) -> Result<(), DomainError> {
        sqlx::query("UPDATE publication_tasks SET status = 'publishing', updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_published(
        &self,
        id: Uuid,
        external_post_id: Option<String>,
        permalink: Option<String>,
    ) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE publication_tasks SET status = 'published', external_post_id = $1, permalink = $2, \
             published_at = now(), updated_at = now() WHERE id = $3",
        )
        .bind(&external_post_id)
        .bind(&permalink)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_failed(&self, id: Uuid, error: String, terminal: bool) -> Result<(), DomainError> {
        let status = if terminal { "failed" } else { "pending" };
        sqlx::query(
            "UPDATE publication_tasks SET status = $1, attempts = attempts + 1, last_error = $2, updated_at = now() \
             WHERE id = $3",
        )
        .bind(status)
        .bind(&error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn reset_to_pending(&self, id: Uuid) -> Result<(), DomainError> {
        sqlx::query("UPDATE publication_tasks SET status = 'pending', last_error = NULL, updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn cancel(&self, id: Uuid) -> Result<(), DomainError> {
        sqlx::query("UPDATE publication_tasks SET status = 'cancelled', updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
