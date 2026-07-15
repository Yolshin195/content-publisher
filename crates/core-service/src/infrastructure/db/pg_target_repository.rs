use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::TargetRepository;
use crate::domain::{DomainError, NewPublicationTarget, Platform, PublicationTarget};

pub struct PgTargetRepository {
    pool: PgPool,
}

impl PgTargetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const TARGET_COLUMNS: &str = "id, platform, external_id, display_name, is_active, config, created_at, updated_at";

fn row_to_target(row: &sqlx::postgres::PgRow) -> Result<PublicationTarget, DomainError> {
    let platform_str: String = row.try_get("platform")?;
    Ok(PublicationTarget {
        id: row.try_get("id")?,
        platform: Platform::parse(&platform_str)?,
        external_id: row.try_get("external_id")?,
        display_name: row.try_get("display_name")?,
        is_active: row.try_get("is_active")?,
        config: row.try_get("config")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[async_trait]
impl TargetRepository for PgTargetRepository {
    async fn create(&self, new_target: NewPublicationTarget) -> Result<PublicationTarget, DomainError> {
        let sql = format!(
            "INSERT INTO publication_targets (platform, external_id, display_name, config) \
             VALUES ($1, $2, $3, $4) RETURNING {TARGET_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(new_target.platform.as_str())
            .bind(&new_target.external_id)
            .bind(&new_target.display_name)
            .bind(&new_target.config)
            .fetch_one(&self.pool)
            .await?;
        row_to_target(&row)
    }

    async fn get(&self, id: Uuid) -> Result<PublicationTarget, DomainError> {
        let sql = format!("SELECT {TARGET_COLUMNS} FROM publication_targets WHERE id = $1");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::TargetNotFound)?;
        row_to_target(&row)
    }

    async fn list(&self) -> Result<Vec<PublicationTarget>, DomainError> {
        let sql = format!("SELECT {TARGET_COLUMNS} FROM publication_targets ORDER BY created_at DESC");
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_target).collect()
    }

    async fn update(
        &self,
        id: Uuid,
        display_name: Option<String>,
        is_active: Option<bool>,
        config: Option<Value>,
    ) -> Result<PublicationTarget, DomainError> {
        let current = self.get(id).await?;
        let display_name = display_name.unwrap_or(current.display_name);
        let is_active = is_active.unwrap_or(current.is_active);
        let config = config.unwrap_or(current.config);

        let sql = format!(
            "UPDATE publication_targets SET display_name = $1, is_active = $2, config = $3, updated_at = now() \
             WHERE id = $4 RETURNING {TARGET_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&display_name)
            .bind(is_active)
            .bind(&config)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::TargetNotFound)?;
        row_to_target(&row)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        let result = sqlx::query("DELETE FROM publication_targets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::TargetNotFound);
        }
        Ok(())
    }
}
