use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::ports::{ArticleFilter, ArticleRepository};
use crate::domain::{Article, ArticleState, ArticleUpdate, DomainError, NewArticle, VideoLink, VideoPlatform};

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
    Ok(Article {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        slug: row.try_get("slug")?,
        content_html: row.try_get("content_html")?,
        excerpt: row.try_get("excerpt")?,
        cover_media_id: row.try_get("cover_media_id")?,
        state: ArticleState::parse(&state_str)?,
        scheduled_at: row.try_get("scheduled_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        deleted_at: row.try_get("deleted_at")?,
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

const ARTICLE_COLUMNS: &str =
    "id, title, slug, content_html, excerpt, cover_media_id, state, scheduled_at, created_at, updated_at, deleted_at";

#[async_trait]
impl ArticleRepository for PgArticleRepository {
    async fn create(&self, new_article: NewArticle) -> Result<Article, DomainError> {
        let sql = format!(
            "INSERT INTO articles (title, slug, content_html, excerpt, cover_media_id) \
             VALUES ($1, $2, $3, $4, $5) RETURNING {ARTICLE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&new_article.title)
            .bind(&new_article.slug)
            .bind(&new_article.content_html)
            .bind(&new_article.excerpt)
            .bind(new_article.cover_media_id)
            .fetch_one(&self.pool)
            .await?;

        row_to_article(&row)
    }

    async fn get(&self, id: Uuid) -> Result<Article, DomainError> {
        let sql = format!("SELECT {ARTICLE_COLUMNS} FROM articles WHERE id = $1 AND deleted_at IS NULL");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::ArticleNotFound)?;

        row_to_article(&row)
    }

    async fn list(&self, filter: ArticleFilter) -> Result<(Vec<Article>, i64), DomainError> {
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;

        let sql = format!(
            "SELECT {ARTICLE_COLUMNS} FROM articles \
             WHERE deleted_at IS NULL \
               AND ($1::text IS NULL OR state = $1) \
               AND ($2::timestamptz IS NULL OR scheduled_at >= $2) \
               AND ($3::timestamptz IS NULL OR scheduled_at <= $3) \
             ORDER BY COALESCE(scheduled_at, created_at) DESC \
             LIMIT $4 OFFSET $5"
        );
        let rows = sqlx::query(&sql)
            .bind(&filter.state)
            .bind(filter.from)
            .bind(filter.to)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let total_row = sqlx::query(
            "SELECT COUNT(*) as total FROM articles \
             WHERE deleted_at IS NULL \
               AND ($1::text IS NULL OR state = $1) \
               AND ($2::timestamptz IS NULL OR scheduled_at >= $2) \
               AND ($3::timestamptz IS NULL OR scheduled_at <= $3)",
        )
        .bind(&filter.state)
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

        let sql = format!(
            "UPDATE articles SET title = $1, content_html = $2, excerpt = $3, cover_media_id = $4, updated_at = now() \
             WHERE id = $5 AND deleted_at IS NULL RETURNING {ARTICLE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&title)
            .bind(&content_html)
            .bind(&excerpt)
            .bind(cover_media_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::ArticleNotFound)?;

        row_to_article(&row)
    }

    async fn set_state(
        &self,
        id: Uuid,
        state: ArticleState,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Article, DomainError> {
        let sql = format!(
            "UPDATE articles SET state = $1, scheduled_at = COALESCE($2, scheduled_at), updated_at = now() \
             WHERE id = $3 AND deleted_at IS NULL RETURNING {ARTICLE_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(state.as_str())
            .bind(scheduled_at)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DomainError::ArticleNotFound)?;

        row_to_article(&row)
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
            "SELECT {ARTICLE_COLUMNS} FROM articles \
             WHERE deleted_at IS NULL AND scheduled_at >= $1 AND scheduled_at < $2 \
             ORDER BY scheduled_at ASC"
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
             WHERE article_id = $1 ORDER BY is_primary DESC",
        )
        .bind(article_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_video_link).collect()
    }
}
