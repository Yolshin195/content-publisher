use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::PromptRepository;
use crate::domain::{DomainError, NewPromptNode, PromptKind, PromptNode, PromptNodeUpdate, PromptVersion};

pub struct PgPromptRepository {
    pool: PgPool,
}

impl PgPromptRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const NODE_COLUMNS: &str =
    "id, parent_id, kind, key, title, description, body, version, position, created_at, updated_at";
const VERSION_COLUMNS: &str = "id, prompt_id, version, title, description, body, change_note, author, created_at";

fn row_to_node(row: &sqlx::postgres::PgRow) -> Result<PromptNode, DomainError> {
    let kind: String = row.try_get("kind")?;
    Ok(PromptNode {
        id: row.try_get("id")?,
        parent_id: row.try_get("parent_id")?,
        kind: PromptKind::parse(&kind)?,
        key: row.try_get("key")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        body: row.try_get("body")?,
        version: row.try_get("version")?,
        position: row.try_get("position")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn row_to_version(row: &sqlx::postgres::PgRow) -> Result<PromptVersion, DomainError> {
    Ok(PromptVersion {
        id: row.try_get("id")?,
        prompt_id: row.try_get("prompt_id")?,
        version: row.try_get("version")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        body: row.try_get("body")?,
        change_note: row.try_get("change_note")?,
        author: row.try_get("author")?,
        created_at: row.try_get("created_at")?,
    })
}

fn map_unique(e: sqlx::Error, key: Option<&str>) -> DomainError {
    match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Conflict(format!("prompt key '{}' is already used", key.unwrap_or_default()))
        }
        _ => DomainError::Database(e),
    }
}

#[async_trait]
impl PromptRepository for PgPromptRepository {
    async fn list_all(&self) -> Result<Vec<PromptNode>, DomainError> {
        let sql = format!("SELECT {NODE_COLUMNS} FROM prompt_nodes ORDER BY position ASC, lower(title) ASC");
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_node).collect()
    }

    async fn get(&self, id: Uuid) -> Result<PromptNode, DomainError> {
        let sql = format!("SELECT {NODE_COLUMNS} FROM prompt_nodes WHERE id = $1");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| DomainError::PromptNotFound(id.to_string()))?;
        row_to_node(&row)
    }

    async fn get_by_key(&self, key: &str) -> Result<PromptNode, DomainError> {
        let sql = format!("SELECT {NODE_COLUMNS} FROM prompt_nodes WHERE lower(key) = lower($1)");
        let row = sqlx::query(&sql)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| DomainError::PromptNotFound(key.to_string()))?;
        row_to_node(&row)
    }

    async fn create(&self, node: NewPromptNode) -> Result<PromptNode, DomainError> {
        let mut tx = self.pool.begin().await?;

        let position: i32 = sqlx::query(
            "SELECT COALESCE(MAX(position) + 1, 0) AS next FROM prompt_nodes WHERE parent_id IS NOT DISTINCT FROM $1",
        )
        .bind(node.parent_id)
        .fetch_one(&mut *tx)
        .await?
        .try_get("next")?;

        let sql = format!(
            "INSERT INTO prompt_nodes (parent_id, kind, key, title, description, body, position) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING {NODE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(node.parent_id)
            .bind(node.kind.as_str())
            .bind(&node.key)
            .bind(&node.title)
            .bind(&node.description)
            .bind(&node.body)
            .bind(position)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| map_unique(e, node.key.as_deref()))?;
        let created = row_to_node(&row)?;

        if created.kind == PromptKind::Prompt {
            sqlx::query(
                "INSERT INTO prompt_versions (prompt_id, version, title, description, body, change_note, author) \
                 VALUES ($1, 1, $2, $3, $4, $5, $6)",
            )
            .bind(created.id)
            .bind(&created.title)
            .bind(&created.description)
            .bind(&created.body)
            .bind(&node.change_note)
            .bind(&node.author)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(created)
    }

    async fn update(&self, id: Uuid, update: PromptNodeUpdate) -> Result<PromptNode, DomainError> {
        let mut tx = self.pool.begin().await?;

        let sql = format!("SELECT {NODE_COLUMNS} FROM prompt_nodes WHERE id = $1 FOR UPDATE");
        let current = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| DomainError::PromptNotFound(id.to_string()))?;
        let current = row_to_node(&current)?;

        // Пустая строка очищает ключ (допустимо только для папок — проверяется в сервисе).
        let key = match update.key {
            None => current.key.clone(),
            Some(k) if k.is_empty() => None,
            Some(k) => Some(k),
        };
        let title = update.title.unwrap_or_else(|| current.title.clone());
        let description = update.description.unwrap_or_else(|| current.description.clone());
        let body = update.body.unwrap_or_else(|| current.body.clone());

        let content_changed = title != current.title || description != current.description || body != current.body;
        let new_version =
            if current.kind == PromptKind::Prompt && content_changed { current.version + 1 } else { current.version };

        let sql = format!(
            "UPDATE prompt_nodes SET key = $1, title = $2, description = $3, body = $4, version = $5, updated_at = now() \
             WHERE id = $6 RETURNING {NODE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&key)
            .bind(&title)
            .bind(&description)
            .bind(&body)
            .bind(new_version)
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| map_unique(e, key.as_deref()))?;
        let updated = row_to_node(&row)?;

        if new_version != current.version {
            sqlx::query(
                "INSERT INTO prompt_versions (prompt_id, version, title, description, body, change_note, author) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(id)
            .bind(new_version)
            .bind(&title)
            .bind(&description)
            .bind(&body)
            .bind(&update.change_note)
            .bind(update.author.as_deref().unwrap_or("user"))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(updated)
    }

    async fn move_node(&self, id: Uuid, parent_id: Option<Uuid>, position: i32) -> Result<PromptNode, DomainError> {
        let mut tx = self.pool.begin().await?;

        // Освобождаем место: сдвигаем соседей в новой папке.
        sqlx::query(
            "UPDATE prompt_nodes SET position = position + 1 \
             WHERE parent_id IS NOT DISTINCT FROM $1 AND position >= $2 AND id <> $3",
        )
        .bind(parent_id)
        .bind(position)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        let sql = format!(
            "UPDATE prompt_nodes SET parent_id = $1, position = $2, updated_at = now() WHERE id = $3 RETURNING {NODE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(parent_id)
            .bind(position)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| DomainError::PromptNotFound(id.to_string()))?;
        let node = row_to_node(&row)?;

        tx.commit().await?;
        Ok(node)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        let result = sqlx::query("DELETE FROM prompt_nodes WHERE id = $1").bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::PromptNotFound(id.to_string()));
        }
        Ok(())
    }

    async fn list_versions(&self, prompt_id: Uuid) -> Result<Vec<PromptVersion>, DomainError> {
        let sql = format!("SELECT {VERSION_COLUMNS} FROM prompt_versions WHERE prompt_id = $1 ORDER BY version DESC");
        let rows = sqlx::query(&sql).bind(prompt_id).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_version).collect()
    }

    async fn get_version(&self, prompt_id: Uuid, version: i32) -> Result<PromptVersion, DomainError> {
        let sql = format!("SELECT {VERSION_COLUMNS} FROM prompt_versions WHERE prompt_id = $1 AND version = $2");
        let row = sqlx::query(&sql)
            .bind(prompt_id)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| DomainError::PromptNotFound(format!("{prompt_id} v{version}")))?;
        row_to_version(&row)
    }
}
