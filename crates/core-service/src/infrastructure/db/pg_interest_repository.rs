use async_trait::async_trait;
use sqlx::types::Json;
use sqlx::{PgPool, Row};

use crate::application::ports::InterestRepository;
use crate::domain::{DomainError, InterestSnapshot, InterestTopic, NewInterestSnapshot};

pub struct PgInterestRepository {
    pool: PgPool,
}

impl PgInterestRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const COLUMNS: &str = "id, source, summary, topics, period_from, period_to, created_at";

fn row_to_snapshot(row: &sqlx::postgres::PgRow) -> Result<InterestSnapshot, DomainError> {
    let topics: Json<Vec<InterestTopic>> = row.try_get("topics")?;
    Ok(InterestSnapshot {
        id: row.try_get("id")?,
        source: row.try_get("source")?,
        summary: row.try_get("summary")?,
        topics: topics.0,
        period_from: row.try_get("period_from")?,
        period_to: row.try_get("period_to")?,
        created_at: row.try_get("created_at")?,
    })
}

#[async_trait]
impl InterestRepository for PgInterestRepository {
    async fn create(&self, s: NewInterestSnapshot) -> Result<InterestSnapshot, DomainError> {
        let sql = format!(
            "INSERT INTO interest_snapshots (source, summary, topics, period_from, period_to) \
             VALUES ($1, $2, $3, $4, $5) RETURNING {COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&s.source)
            .bind(&s.summary)
            .bind(Json(&s.topics))
            .bind(s.period_from)
            .bind(s.period_to)
            .fetch_one(&self.pool)
            .await?;
        row_to_snapshot(&row)
    }

    async fn list_recent(&self, limit: i64) -> Result<Vec<InterestSnapshot>, DomainError> {
        let sql = format!("SELECT {COLUMNS} FROM interest_snapshots ORDER BY created_at DESC LIMIT $1");
        let rows = sqlx::query(&sql).bind(limit).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_snapshot).collect()
    }
}
