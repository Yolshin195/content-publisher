use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DomainError;

/// Редакторский статус статьи. Не путать со статусом публикации в конкретную
/// площадку — тот живёт отдельно, в `PublicationTask`, т.к. одна статья может
/// быть опубликована в Telegram и одновременно провалиться в VK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleState {
    Draft,
    Scheduled,
    Archived,
}

impl ArticleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArticleState::Draft => "draft",
            ArticleState::Scheduled => "scheduled",
            ArticleState::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "draft" => Ok(ArticleState::Draft),
            "scheduled" => Ok(ArticleState::Scheduled),
            "archived" => Ok(ArticleState::Archived),
            other => Err(DomainError::Validation(format!("unknown article state: {other}"))),
        }
    }
}

/// Платформа хостинга видео. Намеренно список, а не одно поле — статья может
/// хранить несколько зеркал одного и того же ролика (например, YouTube +
/// VK Video), т.к. YouTube работает не везде.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoPlatform {
    Youtube,
    VkVideo,
    Rutube,
    Vimeo,
    Other,
}

impl VideoPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            VideoPlatform::Youtube => "youtube",
            VideoPlatform::VkVideo => "vk_video",
            VideoPlatform::Rutube => "rutube",
            VideoPlatform::Vimeo => "vimeo",
            VideoPlatform::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "youtube" => Ok(VideoPlatform::Youtube),
            "vk_video" => Ok(VideoPlatform::VkVideo),
            "rutube" => Ok(VideoPlatform::Rutube),
            "vimeo" => Ok(VideoPlatform::Vimeo),
            "other" => Ok(VideoPlatform::Other),
            other => Err(DomainError::Validation(format!("unknown video platform: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoLink {
    pub id: Uuid,
    pub article_id: Uuid,
    pub platform: VideoPlatform,
    pub url: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    /// Ссылка на MediaAsset в Media Service. Не обязательна — статья может быть без картинки.
    pub cover_media_id: Option<Uuid>,
    pub state: ArticleState,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
}

/// `Option<Option<T>>` для полей, которые можно явно очистить (например, убрать
/// картинку): внешний `Option` — «поле передано в запросе», внутренний — новое значение.
#[derive(Debug, Clone, Default)]
pub struct ArticleUpdate {
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub excerpt: Option<Option<String>>,
    pub cover_media_id: Option<Option<Uuid>>,
}
