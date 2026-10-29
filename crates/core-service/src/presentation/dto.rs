use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::application::prompt_service::{DiffLine, PromptFile, PromptTreeNode};
use crate::domain::{self, telegram_html};

/// Различает «поле не передано» (`None`) и «передан null» (`Some(None)`) —
/// нужно, чтобы PATCH мог явно очистить поле.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

// ---------- Запросы ----------

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateArticleRequest {
    pub title: String,
    /// Необязателен — сгенерируется из заголовка.
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Плановая дата (статья остаётся черновиком, пока её не запланируют).
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateArticleRequest {
    pub title: Option<String>,
    pub content_html: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub excerpt: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<Uuid>)]
    pub cover_media_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub cover_image_url: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub rationale: Option<Option<String>>,
    pub sources: Option<Vec<SourceLinkDto>>,
    /// Перенос даты публикации; `null` — убрать плановую дату у черновика.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<DateTime<Utc>>)]
    pub scheduled_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SourceLinkDto {
    #[serde(default)]
    pub title: String,
    pub url: String,
}

impl From<SourceLinkDto> for domain::SourceLink {
    fn from(s: SourceLinkDto) -> Self {
        Self { title: s.title, url: s.url }
    }
}

impl From<domain::SourceLink> for SourceLinkDto {
    fn from(s: domain::SourceLink) -> Self {
        Self { title: s.title, url: s.url }
    }
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
    /// Пустой список — все активные площадки.
    #[serde(default)]
    pub target_ids: Vec<Uuid>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PublishNowRequest {
    /// Площадки для немедленной публикации; если не указаны — уже назначенные
    /// (или все активные, если назначенных нет).
    pub target_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct ReviewRequest {
    /// Комментарий автора: почему принято/отклонено. Claude учитывает его в следующих предложениях.
    pub feedback: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSuggestionRequest {
    pub title: String,
    /// Текст в HTML-разметке Telegram (b, i, u, s, a, code, pre, blockquote, tg-spoiler; переносы — `\n`).
    pub content_html: String,
    pub excerpt: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub cover_image_url: Option<String>,
    /// Ключ промпта-шаблона, по которому написан пост (версия фиксируется автоматически).
    pub prompt_key: Option<String>,
    /// Другие использованные промпты (например, промпт поиска новостей).
    #[serde(default)]
    pub extra_prompt_keys: Vec<String>,
    /// Почему стоит опубликовать этот пост.
    pub rationale: Option<String>,
    #[serde(default)]
    pub sources: Vec<SourceLinkDto>,
    /// Предлагаемое время публикации.
    pub publish_at: Option<DateTime<Utc>>,
    /// Сразу запланировать на `publish_at` без ревью автора.
    #[serde(default)]
    pub auto_publish: bool,
    #[serde(default)]
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
    /// Статус или несколько через запятую: suggested, draft, scheduled, archived, rejected.
    pub state: Option<String>,
    /// manual | ai
    pub origin: Option<String>,
    pub tag: Option<String>,
    pub prompt_key: Option<String>,
    /// Поиск по заголовку и тексту.
    pub q: Option<String>,
    /// true — только с датой, false — только без даты.
    pub has_date: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// created | updated | scheduled | scheduled_desc
    pub sort: Option<String>,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePromptRequest {
    /// prompt | folder
    pub kind: String,
    pub parent_id: Option<Uuid>,
    /// Обязателен для промптов: латиница, цифры, `_ - .`
    pub key: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    pub change_note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePromptRequest {
    pub key: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub body: Option<String>,
    /// Что изменилось — попадёт в историю версий.
    pub change_note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SavePromptByKeyRequest {
    pub body: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Путь папок, например `Новости/Rust` (создаётся при необходимости).
    pub folder_path: Option<String>,
    pub change_note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MovePromptRequest {
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub position: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportPromptsRequest {
    /// Содержимое .md файлов с front matter (`key`, `title`, `path`, `description`).
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DiffQuery {
    pub from: i32,
    /// Не указан — сравнение с текущей версией.
    pub to: Option<i32>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ResolveQuery {
    /// Раскрыть включения `{{> key}}` (по умолчанию true).
    pub resolve: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InterestTopicDto {
    pub name: String,
    /// 1–10
    pub weight: i32,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInterestSnapshotRequest {
    /// Откуда собраны интересы: chats (по умолчанию), news, ...
    pub source: Option<String>,
    /// Короткое резюме: чем автор интересуется сейчас.
    #[serde(default)]
    pub summary: String,
    pub topics: Vec<InterestTopicDto>,
    pub period_from: Option<DateTime<Utc>>,
    pub period_to: Option<DateTime<Utc>>,
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
pub struct PromptRefDto {
    pub key: String,
    pub version: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicationSummaryDto {
    /// none | pending | published | partial | failed
    pub status: String,
    pub total: i64,
    pub published: i64,
    pub failed: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub permalink: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArticleResponse {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
    pub cover_image_url: Option<String>,
    pub state: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    /// manual | ai
    pub origin: String,
    pub tags: Vec<String>,
    pub prompt_key: Option<String>,
    pub prompt_version: Option<i32>,
    pub prompt_refs: Vec<PromptRefDto>,
    pub rationale: Option<String>,
    pub sources: Vec<SourceLinkDto>,
    pub feedback: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub publication: PublicationSummaryDto,
    /// Видимая длина текста (как считает Telegram).
    pub text_length: usize,
    /// Проблемы разметки/длины, из-за которых Telegram не примет пост.
    pub telegram_problems: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_links: Option<Vec<VideoLinkResponse>>,
}

impl From<domain::Article> for ArticleResponse {
    fn from(a: domain::Article) -> Self {
        let telegram_problems = telegram_html::check_post(&a.title, &a.content_html, a.cover_image_url.is_some());
        let text_length = telegram_html::visible_len(&a.content_html);
        let status = a.publication.status().to_string();
        Self {
            id: a.id,
            title: a.title,
            slug: a.slug,
            content_html: a.content_html,
            excerpt: a.excerpt,
            cover_media_id: a.cover_media_id,
            cover_image_url: a.cover_image_url,
            state: a.state.as_str().to_string(),
            scheduled_at: a.scheduled_at,
            origin: a.origin.as_str().to_string(),
            tags: a.tags,
            prompt_key: a.prompt_key,
            prompt_version: a.prompt_version,
            prompt_refs: a.prompt_refs.into_iter().map(|r| PromptRefDto { key: r.key, version: r.version }).collect(),
            rationale: a.rationale,
            sources: a.sources.into_iter().map(SourceLinkDto::from).collect(),
            feedback: a.feedback,
            reviewed_at: a.reviewed_at,
            created_at: a.created_at,
            updated_at: a.updated_at,
            publication: PublicationSummaryDto {
                status,
                total: a.publication.total,
                published: a.publication.published,
                failed: a.publication.failed,
                published_at: a.publication.published_at,
                permalink: a.publication.permalink,
            },
            text_length,
            telegram_problems,
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
pub struct TagCountResponse {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PromptResponse {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub key: Option<String>,
    pub title: String,
    pub description: String,
    pub body: String,
    pub version: i32,
    pub path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PromptResponse {
    pub fn new(n: domain::PromptNode, path: String) -> Self {
        Self {
            id: n.id,
            parent_id: n.parent_id,
            kind: n.kind.as_str().to_string(),
            key: n.key,
            title: n.title,
            description: n.description,
            body: n.body,
            version: n.version,
            path,
            created_at: n.created_at,
            updated_at: n.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PromptTreeResponse {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub key: Option<String>,
    pub title: String,
    pub description: String,
    pub version: i32,
    pub path: String,
    #[schema(no_recursion)]
    pub children: Vec<PromptTreeResponse>,
}

impl From<PromptTreeNode> for PromptTreeResponse {
    fn from(n: PromptTreeNode) -> Self {
        Self {
            id: n.id,
            parent_id: n.parent_id,
            kind: n.kind.as_str().to_string(),
            key: n.key,
            title: n.title,
            description: n.description,
            version: n.version,
            path: n.path,
            children: n.children.into_iter().map(PromptTreeResponse::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResolvedPromptResponse {
    pub key: String,
    pub title: String,
    pub version: i32,
    /// Текст промпта (с раскрытыми включениями, если resolve=true).
    pub body: String,
    pub refs: Vec<PromptRefDto>,
    pub warnings: Vec<String>,
}

impl From<domain::ResolvedPrompt> for ResolvedPromptResponse {
    fn from(r: domain::ResolvedPrompt) -> Self {
        Self {
            key: r.key,
            title: r.title,
            version: r.version,
            body: r.body,
            refs: r.refs.into_iter().map(|r| PromptRefDto { key: r.key, version: r.version }).collect(),
            warnings: r.warnings,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PromptVersionResponse {
    pub version: i32,
    pub title: String,
    pub description: String,
    pub body: String,
    pub change_note: Option<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

impl From<domain::PromptVersion> for PromptVersionResponse {
    fn from(v: domain::PromptVersion) -> Self {
        Self {
            version: v.version,
            title: v.title,
            description: v.description,
            body: v.body,
            change_note: v.change_note,
            author: v.author,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DiffLineResponse {
    /// equal | insert | delete
    pub tag: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

impl From<DiffLine> for DiffLineResponse {
    fn from(d: DiffLine) -> Self {
        Self { tag: d.tag.to_string(), old_line: d.old_line, new_line: d.new_line, text: d.text }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PromptFileResponse {
    pub path: String,
    pub content: String,
}

impl From<PromptFile> for PromptFileResponse {
    fn from(f: PromptFile) -> Self {
        Self { path: f.path, content: f.content }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InterestTopicResponse {
    pub name: String,
    pub weight: i32,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InterestSnapshotResponse {
    pub id: Uuid,
    pub source: String,
    pub summary: String,
    pub topics: Vec<InterestTopicResponse>,
    pub period_from: Option<DateTime<Utc>>,
    pub period_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<domain::InterestSnapshot> for InterestSnapshotResponse {
    fn from(s: domain::InterestSnapshot) -> Self {
        Self {
            id: s.id,
            source: s.source,
            summary: s.summary,
            topics: s
                .topics
                .into_iter()
                .map(|t| InterestTopicResponse { name: t.name, weight: t.weight, note: t.note })
                .collect(),
            period_from: s.period_from,
            period_to: s.period_to,
            created_at: s.created_at,
        }
    }
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
