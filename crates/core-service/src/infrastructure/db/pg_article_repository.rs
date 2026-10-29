use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::{ArticleFilter, ArticleRepository};
use crate::domain::{
    Article, ArticleOrigin, ArticleState, ArticleUpdate, DomainError, NewArticle, PromptRef, PublicationSummary,
    SourceLink, VideoLink, VideoPlatform,
};

pub struct PgArticleRepository {
    pool: PgPool,
}

impl PgArticleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn row_to_article(row: &sqlx::postgres::PgRow) -> Result<Article, DomainError> {
    let state_str: String = row.try_get("state")?;
    let origin_str: String = row.try_get("origin")?;
    let sources: Json<Vec<SourceLink>> = row.try_get("sources")?;
    let prompt_refs: Json<Vec<PromptRef>> = row.try_get("prompt_refs")?;
    Ok(Article {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        slug: row.try_get("slug")?,
        content_html: row.try_get("content_html")?,
        excerpt: row.try_get("excerpt")?,
        cover_media_id: row.try_get("cover_media_id")?,
        cover_image_url: row.try_get("cover_image_url")?,
        state: ArticleState::parse(&state_str)?,
        scheduled_at: row.try_get("scheduled_at")?,
        origin: ArticleOrigin::parse(&origin_str)?,
        tags: row.try_get("tags")?,
        prompt_key: row.try_get("prompt_key")?,
        prompt_version: row.try_get("prompt_version")?,
        prompt_refs: prompt_refs.0,
        rationale: row.try_get("rationale")?,
        sources: sources.0,
        feedback: row.try_get("feedback")?,
        reviewed_at: row.try_get("reviewed_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        deleted_at: row.try_get("deleted_at")?,
        publication: PublicationSummary {
            total: row.try_get("pub_total")?,
            published: row.try_get("pub_published")?,
            failed: row.try_get("pub_failed")?,
            published_at: row.try_get("pub_published_at")?,
            permalink: row.try_get("pub_permalink")?,
        },
    })
}

fn row_to_video_link(row: &sqlx::postgres::PgRow) -> Result<VideoLink, DomainError> {
    let platform_str: String = row.try_get("platform")?;
    Ok(VideoLink {
        id: row.try_get("id")?,
        article_id: row.try_get("article_id")?,
        platform: VideoPlatform::parse(&platform_str)?,
        url: row.try_get("url")?,
        is_primary: row.try_get("is_primary")?,
    })
}

/// Колонки статьи + агрегированный статус публикации по её задачам.
const ARTICLE_SELECT: &str = "\
    SELECT a.id, a.title, a.slug, a.content_html, a.excerpt, a.cover_media_id, a.cover_image_url, a.state, \
           a.scheduled_at, a.origin, a.tags, a.prompt_key, a.prompt_version, a.prompt_refs, a.rationale, a.sources, \
           a.feedback, a.reviewed_at, a.created_at, a.updated_at, a.deleted_at, \
           ps.total AS pub_total, ps.published AS pub_published, ps.failed AS pub_failed, \
           ps.published_at AS pub_published_at, ps.permalink AS pub_permalink \
    FROM articles a \
    LEFT JOIN LATERAL ( \
        SELECT COUNT(*) FILTER (WHERE t.status <> 'cancelled') AS total, \
               COUNT(*) FILTER (WHERE t.status = 'published') AS published, \
               COUNT(*) FILTER (WHERE t.status = 'failed') AS failed, \
               MAX(t.published_at) AS published_at, \
               (ARRAY_AGG(t.permalink ORDER BY t.published_at) FILTER (WHERE t.permalink IS NOT NULL))[1] AS permalink \
        FROM publication_tasks t WHERE t.article_id = a.id \
    ) ps ON true";

const LIST_WHERE: &str = "\
    WHERE a.deleted_at IS NULL \
      AND ($1::text[] IS NULL OR a.state = ANY($1)) \
      AND ($2::text IS NULL OR a.origin = $2) \
      AND ($3::text IS NULL OR $3 = ANY(a.tags)) \
      AND ($4::text IS NULL OR lower(a.prompt_key) = lower($4)) \
      AND ($5::text IS NULL OR a.title ILIKE $5 OR a.content_html ILIKE $5) \
      AND ($6::bool IS NULL OR ($6 AND a.scheduled_at IS NOT NULL) OR (NOT $6 AND a.scheduled_at IS NULL)) \
      AND ($7::timestamptz IS NULL OR a.scheduled_at >= $7) \
      AND ($8::timestamptz IS NULL OR a.scheduled_at <= $8)";

fn order_by(sort: Option<&str>) -> &'static str {
    match sort {
        Some("created") => "ORDER BY a.created_at DESC",
        Some("updated") => "ORDER BY a.updated_at DESC",
        Some("scheduled") => "ORDER BY a.scheduled_at ASC NULLS LAST, a.created_at DESC",
        Some("scheduled_desc") => "ORDER BY a.scheduled_at DESC NULLS LAST, a.created_at DESC",
        _ => "ORDER BY COALESCE(a.scheduled_at, a.created_at) DESC",
    }
}

fn like_pattern(q: &str) -> String {
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

#[async_trait]
impl ArticleRepository for PgArticleRepository {
    async fn create(&self, a: NewArticle) -> Result<Article, DomainError> {
        let row = sqlx::query(
            "INSERT INTO articles (title, slug, content_html, excerpt, cover_media_id, cover_image_url, state, origin, \
                 tags, prompt_key, prompt_version, prompt_refs, rationale, sources) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING id",
        )
        .bind(&a.title)
        .bind(&a.slug)
        .bind(&a.content_html)
        .bind(&a.excerpt)
        .bind(a.cover_media_id)
        .bind(&a.cover_image_url)
        .bind(a.state.as_str())
        .bind(a.origin.as_str())
        .bind(&a.tags)
        .bind(&a.prompt_key)
        .bind(a.prompt_version)
        .bind(Json(&a.prompt_refs))
        .bind(&a.rationale)
        .bind(Json(&a.sources))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                DomainError::Conflict(format!("slug '{}' already exists", a.slug))
            }
            _ => DomainError::Database(e),
        })?;

        self.get(row.try_get("id")?).await
    }

    async fn get(&self, id: Uuid) -> Result<Article, DomainError> {
        let sql = format!("{ARTICLE_SELECT} WHERE a.id = $1 AND a.deleted_at IS NULL");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::ArticleNotFound)?;

        row_to_article(&row)
    }

    async fn list(&self, filter: ArticleFilter) -> Result<(Vec<Article>, i64), DomainError> {
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 200);
        let offset = (page - 1) * per_page;

        let states: Option<Vec<String>> = filter.state.as_deref().filter(|s| !s.is_empty()).map(|s| {
            s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
        });
        let q = filter.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(like_pattern);
        let tag = filter.tag.as_deref().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty());

        let sql = format!("{ARTICLE_SELECT} {LIST_WHERE} {} LIMIT $9 OFFSET $10", order_by(filter.sort.as_deref()));
        let rows = sqlx::query(&sql)
            .bind(&states)
            .bind(&filter.origin)
            .bind(&tag)
            .bind(&filter.prompt_key)
            .bind(&q)
            .bind(filter.has_date)
            .bind(filter.from)
            .bind(filter.to)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count_sql = format!("SELECT COUNT(*) AS total FROM articles a {LIST_WHERE}");
        let total_row = sqlx::query(&count_sql)
            .bind(&states)
            .bind(&filter.origin)
            .bind(&tag)
            .bind(&filter.prompt_key)
            .bind(&q)
            .bind(filter.has_date)
            .bind(filter.from)
            .bind(filter.to)
            .fetch_one(&self.pool)
            .await?;

        let total: i64 = total_row.try_get("total")?;
        let articles = rows.iter().map(row_to_article).collect::<Result<Vec<_>, _>>()?;
        Ok((articles, total))
    }

    async fn update(&self, id: Uuid, update: ArticleUpdate) -> Result<Article, DomainError> {
        let current = self.get(id).await?;

        let title = update.title.unwrap_or(current.title);
        let content_html = update.content_html.unwrap_or(current.content_html);
        let excerpt = update.excerpt.unwrap_or(current.excerpt);
        let cover_media_id = update.cover_media_id.unwrap_or(current.cover_media_id);
        let cover_image_url = update.cover_image_url.unwrap_or(current.cover_image_url);
        let tags = update.tags.unwrap_or(current.tags);
        let rationale = update.rationale.unwrap_or(current.rationale);
        let sources = update.sources.unwrap_or(current.sources);

        let result = sqlx::query(
            "UPDATE articles SET title = $1, content_html = $2, excerpt = $3, cover_media_id = $4, \
                 cover_image_url = $5, tags = $6, rationale = $7, sources = $8, updated_at = now() \
             WHERE id = $9 AND deleted_at IS NULL",
        )
        .bind(&title)
        .bind(&content_html)
        .bind(&excerpt)
        .bind(cover_media_id)
        .bind(&cover_image_url)
        .bind(&tags)
        .bind(&rationale)
        .bind(Json(&sources))
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::ArticleNotFound);
        }

        self.get(id).await
    }

    async fn set_state(
        &self,
        id: Uuid,
        state: ArticleState,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Article, DomainError> {
        let result = sqlx::query(
            "UPDATE articles SET state = $1, scheduled_at = COALESCE($2, scheduled_at), updated_at = now() \
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(state.as_str())
        .bind(scheduled_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::ArticleNotFound);
        }

        self.get(id).await
    }

    async fn set_review(&self, id: Uuid, state: ArticleState, feedback: Option<String>) -> Result<Article, DomainError> {
        let result = sqlx::query(
            "UPDATE articles SET state = $1, feedback = COALESCE($2, feedback), reviewed_at = now(), updated_at = now() \
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(state.as_str())
        .bind(&feedback)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::ArticleNotFound);
        }

        self.get(id).await
    }

    async fn set_scheduled_at(&self, id: Uuid, scheduled_at: Option<DateTime<Utc>>) -> Result<Article, DomainError> {
        let result = sqlx::query(
            "UPDATE articles SET scheduled_at = $1, updated_at = now() WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(scheduled_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::ArticleNotFound);
        }

        self.get(id).await
    }

    async fn soft_delete(&self, id: Uuid) -> Result<(), DomainError> {
        let result = sqlx::query("UPDATE articles SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::ArticleNotFound);
        }
        Ok(())
    }

    async fn list_by_date_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Article>, DomainError> {
        let sql = format!(
            "{ARTICLE_SELECT} \
             WHERE a.deleted_at IS NULL AND a.state NOT IN ('rejected', 'archived') \
               AND a.scheduled_at >= $1 AND a.scheduled_at < $2 \
             ORDER BY a.scheduled_at ASC"
        );
        let rows = sqlx::query(&sql).bind(from).bind(to).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_article).collect()
    }

    async fn add_video_link(
        &self,
        article_id: Uuid,
        platform: VideoPlatform,
        url: String,
        is_primary: bool,
    ) -> Result<VideoLink, DomainError> {
        let row = sqlx::query(
            "INSERT INTO article_video_links (article_id, platform, url, is_primary) \
             VALUES ($1, $2, $3, $4) RETURNING id, article_id, platform, url, is_primary",
        )
        .bind(article_id)
        .bind(platform.as_str())
        .bind(&url)
        .bind(is_primary)
        .fetch_one(&self.pool)
        .await?;

        row_to_video_link(&row)
    }

    async fn remove_video_link(&self, article_id: Uuid, link_id: Uuid) -> Result<(), DomainError> {
        let result = sqlx::query("DELETE FROM article_video_links WHERE id = $1 AND article_id = $2")
            .bind(link_id)
            .bind(article_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DomainError::Validation("video link not found".into()));
        }
        Ok(())
    }

    async fn list_video_links(&self, article_id: Uuid) -> Result<Vec<VideoLink>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, article_id, platform, url, is_primary FROM article_video_links \
             WHERE article_id = $1 ORDER BY is_primary DESC, created_at ASC",
        )
        .bind(article_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_video_link).collect()
    }

    async fn tag_counts(&self) -> Result<Vec<(String, i64)>, DomainError> {
        let rows = sqlx::query(
            "SELECT tag, COUNT(*) AS cnt FROM articles, unnest(tags) AS tag \
             WHERE deleted_at IS NULL AND state <> 'rejected' \
             GROUP BY tag ORDER BY cnt DESC, tag ASC LIMIT 200",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(|r| Ok((r.try_get("tag")?, r.try_get("cnt")?))).collect()
    }
}
