//! Общие DTO, пересекающие границы микросервисов:
//! Core Service <-> Publisher-сервисы (Telegram, VK, ...) и Core Service <-> Media Service.
//! Ни один из типов здесь не должен зависеть от конкретной инфраструктуры (БД, HTTP-фреймворка).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Платформа размещения видео. Список специально не ограничен одной ссылкой,
/// т.к. YouTube работает не везде и нужна возможность держать зеркало ролика
/// на другом хостинге (VK Video, Rutube и т.д.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VideoPlatform {
    Youtube,
    VkVideo,
    Rutube,
    Vimeo,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VideoLinkDto {
    pub platform: VideoPlatform,
    pub url: String,
    pub is_primary: bool,
}

/// Ссылка на целевую площадку публикации во внешней системе
/// (например, `-100123456789` для Telegram-канала).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublishTargetRef {
    pub external_id: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublishContent {
    pub title: String,
    pub body_html: String,
    /// Не обязательное поле — статья может публиковаться без обложки.
    pub cover_image_url: Option<String>,
    /// Не обязательное поле, может содержать несколько зеркал одного и того же видео.
    pub video_links: Vec<VideoLinkDto>,
}

/// Единый контракт запроса на публикацию, который обязан принимать
/// любой Publisher-микросервис на `POST /publish`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublishRequest {
    pub task_id: Uuid,
    pub target: PublishTargetRef,
    pub content: PublishContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    Success,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublishResult {
    pub task_id: Uuid,
    pub status: PublishStatus,
    pub external_post_id: Option<String>,
    pub permalink: Option<String>,
    pub error: Option<String>,
}

/// Ответ Media Service на запрос presigned URL для прямой загрузки файла в объектное хранилище.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MediaUploadUrlResponse {
    pub media_id: Uuid,
    pub upload_url: String,
    pub expires_at: DateTime<Utc>,
}
