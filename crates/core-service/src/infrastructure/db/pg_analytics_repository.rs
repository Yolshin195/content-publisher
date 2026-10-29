use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::application::ports::{
    AnalyticsOverview, AnalyticsRepository, FailureRow, HourBucket, LabelCount, PromptStat, WeekBucket,
};
use crate::domain::DomainError;

/// Read-модель для страницы аналитики: только агрегирующие SELECT-ы.
pub struct PgAnalyticsRepository {
    pool: PgPool,
}

impl PgAnalyticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnalyticsRepository for PgAnalyticsRepository {
    async fn overview(&self, tz: &str, weeks: i32) -> Result<AnalyticsOverview, DomainError> {
        let state_rows =
            sqlx::query("SELECT state, COUNT(*) AS cnt FROM articles WHERE deleted_at IS NULL GROUP BY state")
                .fetch_all(&self.pool)
                .await?;
        let state_counts: Vec<LabelCount> = state_rows
            .iter()
            .map(|r| Ok(LabelCount { label: r.try_get("state")?, count: r.try_get("cnt")? }))
            .collect::<Result<_, DomainError>>()?;
        let count_of = |s: &str| state_counts.iter().find(|c| c.label == s).map(|c| c.count).unwrap_or(0);
        let total_posts: i64 = state_counts.iter().map(|c| c.count).sum();

        let totals = sqlx::query(
            "SELECT \
               (SELECT COUNT(DISTINCT t.article_id) FROM publication_tasks t JOIN articles a ON a.id = t.article_id \
                  WHERE a.deleted_at IS NULL AND t.status = 'published') AS published_total, \
               (SELECT COUNT(DISTINCT t.article_id) FROM publication_tasks t JOIN articles a ON a.id = t.article_id \
                  WHERE a.deleted_at IS NULL AND t.status = 'published' AND t.published_at >= now() - interval '30 days') \
                  AS published_30d, \
               (SELECT COUNT(*) FROM articles WHERE deleted_at IS NULL AND state = 'scheduled' AND scheduled_at > now()) \
                  AS upcoming, \
               (SELECT COUNT(*) FROM articles WHERE deleted_at IS NULL AND origin = 'ai' AND reviewed_at IS NOT NULL \
                  AND state IN ('draft', 'scheduled', 'archived')) AS ai_accepted, \
               (SELECT COUNT(*) FROM publication_tasks t JOIN articles a ON a.id = t.article_id \
                  WHERE a.deleted_at IS NULL AND t.status = 'failed') AS failed_tasks",
        )
        .fetch_one(&self.pool)
        .await?;

        let week_rows = sqlx::query(
            "WITH weeks AS ( \
                SELECT generate_series( \
                    date_trunc('week', now() AT TIME ZONE $1) - ($2::int - 1) * interval '1 week', \
                    date_trunc('week', now() AT TIME ZONE $1), \
                    interval '1 week') AS wk \
             ) \
             SELECT wk::date AS week_start, \
               (SELECT COUNT(DISTINCT t.article_id) FROM publication_tasks t JOIN articles a ON a.id = t.article_id \
                  WHERE a.deleted_at IS NULL AND t.status = 'published' \
                    AND date_trunc('week', t.published_at AT TIME ZONE $1) = wk) AS published, \
               (SELECT COUNT(*) FROM publication_logs l \
                  WHERE l.status = 'failure' AND date_trunc('week', l.attempted_at AT TIME ZONE $1) = wk) AS failed, \
               (SELECT COUNT(*) FROM articles a WHERE a.deleted_at IS NULL AND a.origin = 'ai' \
                  AND date_trunc('week', a.created_at AT TIME ZONE $1) = wk) AS created_ai, \
               (SELECT COUNT(*) FROM articles a WHERE a.deleted_at IS NULL AND a.origin = 'manual' \
                  AND date_trunc('week', a.created_at AT TIME ZONE $1) = wk) AS created_manual \
             FROM weeks ORDER BY wk",
        )
        .bind(tz)
        .bind(weeks.clamp(1, 104))
        .fetch_all(&self.pool)
        .await?;
        let weeks = week_rows
            .iter()
            .map(|r| {
                Ok(WeekBucket {
                    week_start: r.try_get("week_start")?,
                    published: r.try_get("published")?,
                    failed: r.try_get("failed")?,
                    created_ai: r.try_get("created_ai")?,
                    created_manual: r.try_get("created_manual")?,
                })
            })
            .collect::<Result<_, DomainError>>()?;

        let prompt_rows = sqlx::query(
            "SELECT MIN(a.prompt_key) AS key, a.prompt_version AS version, COUNT(*) AS total, \
                    COUNT(*) FILTER (WHERE a.state = 'suggested') AS pending, \
                    COUNT(*) FILTER (WHERE a.reviewed_at IS NOT NULL AND a.state IN ('draft', 'scheduled', 'archived')) \
                        AS accepted, \
                    COUNT(*) FILTER (WHERE a.state = 'rejected') AS rejected, \
                    COUNT(*) FILTER (WHERE p.published) AS published \
             FROM articles a \
             LEFT JOIN LATERAL (SELECT EXISTS (SELECT 1 FROM publication_tasks t \
                 WHERE t.article_id = a.id AND t.status = 'published') AS published) p ON true \
             WHERE a.deleted_at IS NULL AND a.prompt_key IS NOT NULL \
             GROUP BY lower(a.prompt_key), a.prompt_version \
             ORDER BY lower(a.prompt_key), a.prompt_version DESC NULLS LAST",
        )
        .fetch_all(&self.pool)
        .await?;
        let prompts = prompt_rows
            .iter()
            .map(|r| {
                Ok(PromptStat {
                    key: r.try_get("key")?,
                    version: r.try_get("version")?,
                    total: r.try_get("total")?,
                    pending: r.try_get("pending")?,
                    accepted: r.try_get("accepted")?,
                    rejected: r.try_get("rejected")?,
                    published: r.try_get("published")?,
                })
            })
            .collect::<Result<_, DomainError>>()?;

        let tag_rows = sqlx::query(
            "SELECT tag, COUNT(*) AS cnt FROM articles, unnest(tags) AS tag \
             WHERE deleted_at IS NULL AND state <> 'rejected' GROUP BY tag ORDER BY cnt DESC, tag ASC LIMIT 15",
        )
        .fetch_all(&self.pool)
        .await?;
        let tags = tag_rows
            .iter()
            .map(|r| Ok(LabelCount { label: r.try_get("tag")?, count: r.try_get("cnt")? }))
            .collect::<Result<_, DomainError>>()?;

        let hour_rows = sqlx::query(
            "SELECT (EXTRACT(ISODOW FROM t.published_at AT TIME ZONE $1)::int - 1) AS weekday, \
                    EXTRACT(HOUR FROM t.published_at AT TIME ZONE $1)::int AS hour, COUNT(*) AS cnt \
             FROM publication_tasks t JOIN articles a ON a.id = t.article_id \
             WHERE a.deleted_at IS NULL AND t.status = 'published' AND t.published_at IS NOT NULL \
             GROUP BY 1, 2",
        )
        .bind(tz)
        .fetch_all(&self.pool)
        .await?;
        let publish_hours = hour_rows
            .iter()
            .map(|r| Ok(HourBucket { weekday: r.try_get("weekday")?, hour: r.try_get("hour")?, count: r.try_get("cnt")? }))
            .collect::<Result<_, DomainError>>()?;

        let failure_rows = sqlx::query(
            "SELECT t.article_id, a.title, pt.display_name AS target_name, t.last_error AS error, t.updated_at \
             FROM publication_tasks t \
             JOIN articles a ON a.id = t.article_id \
             JOIN publication_targets pt ON pt.id = t.target_id \
             WHERE t.status = 'failed' AND a.deleted_at IS NULL \
             ORDER BY t.updated_at DESC LIMIT 10",
        )
        .fetch_all(&self.pool)
        .await?;
        let recent_failures = failure_rows
            .iter()
            .map(|r| {
                Ok(FailureRow {
                    article_id: r.try_get("article_id")?,
                    title: r.try_get("title")?,
                    target_name: r.try_get("target_name")?,
                    error: r.try_get("error")?,
                    updated_at: r.try_get("updated_at")?,
                })
            })
            .collect::<Result<_, DomainError>>()?;

        Ok(AnalyticsOverview {
            total_posts,
            published_total: totals.try_get("published_total")?,
            published_last_30d: totals.try_get("published_30d")?,
            scheduled_upcoming: totals.try_get("upcoming")?,
            suggestions_pending: count_of("suggested"),
            suggestions_accepted: totals.try_get("ai_accepted")?,
            suggestions_rejected: count_of("rejected"),
            failed_tasks: totals.try_get("failed_tasks")?,
            state_counts,
            weeks,
            prompts,
            tags,
            publish_hours,
            recent_failures,
        })
    }
}
