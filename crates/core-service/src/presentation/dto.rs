use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::domain;

// ---------- Запросы ----------

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateArticleRequest {
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateArticleRequest {
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddVideoLinkRequest {
    /// youtube | vk_video | rutube | vimeo | other
    pub platform: String,
    pub url: String,
    #[serde(default)]
    pub is_primary: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ScheduleArticleRequest {
    pub scheduled_at: DateTime<Utc>,
    pub target_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTargetRequest {
    /// telegram | vk | x
    pub platform: String,
    pub external_id: String,
    pub display_name: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTargetRequest {
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
    pub config: Option<Value>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListArticlesQuery {
    pub state: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct MonthQuery {
    pub year: i32,
    pub month: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct WeekQuery {
    pub year: i32,
    pub week: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DayQuery {
    pub date: NaiveDate,
}

// ---------- Ответы ----------

#[derive(Debug, Serialize, ToSchema)]
pub struct VideoLinkResponse {
    pub id: Uuid,
    pub platform: String,
    pub url: String,
    pub is_primary: bool,
}

impl From<domain::VideoLink> for VideoLinkResponse {
    fn from(v: domain::VideoLink) -> Self {
        Self { id: v.id, platform: v.platform.as_str().to_string(), url: v.url, is_primary: v.is_primary }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArticleResponse {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
    pub state: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_links: Option<Vec<VideoLinkResponse>>,
}

impl From<domain::Article> for ArticleResponse {
    fn from(a: domain::Article) -> Self {
        Self {
            id: a.id,
            title: a.title,
            slug: a.slug,
            content_html: a.content_html,
            excerpt: a.excerpt,
            cover_media_id: a.cover_media_id,
            state: a.state.as_str().to_string(),
            scheduled_at: a.scheduled_at,
            created_at: a.created_at,
            updated_at: a.updated_at,
            video_links: None,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TargetResponse {
    pub id: Uuid,
    pub platform: String,
    pub external_id: String,
    pub display_name: String,
    pub is_active: bool,
    pub config: Value,
}

impl From<domain::PublicationTarget> for TargetResponse {
    fn from(t: domain::PublicationTarget) -> Self {
        Self {
            id: t.id,
            platform: t.platform.as_str().to_string(),
            external_id: t.external_id,
            display_name: t.display_name,
            is_active: t.is_active,
            config: t.config,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: Uuid,
    pub article_id: Uuid,
    pub target_id: Uuid,
    pub status: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub external_post_id: Option<String>,
    pub permalink: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

impl From<domain::PublicationTask> for TaskResponse {
    fn from(t: domain::PublicationTask) -> Self {
        Self {
            id: t.id,
            article_id: t.article_id,
            target_id: t.target_id,
            status: t.status.as_str().to_string(),
            attempts: t.attempts,
            last_error: t.last_error,
            external_post_id: t.external_post_id,
            permalink: t.permalink,
            published_at: t.published_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub attempted_at: DateTime<Utc>,
}

impl From<domain::PublicationLog> for LogResponse {
    fn from(l: domain::PublicationLog) -> Self {
        Self {
            id: l.id,
            task_id: l.task_id,
            attempt_no: l.attempt_no,
            status: l.status.as_str().to_string(),
            error_message: l.error_message,
            attempted_at: l.attempted_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DayArticleResponse {
    pub id: Uuid,
    pub title: String,
    pub state: String,
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DaySummaryResponse {
    pub date: NaiveDate,
    pub count: usize,
    pub articles: Vec<DayArticleResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Meta {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListArticlesResponse {
    pub data: Vec<ArticleResponse>,
    pub meta: Meta,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}
