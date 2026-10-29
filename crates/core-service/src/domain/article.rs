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
    /// Предложение от Claude — ждёт решения автора.
    Suggested,
    Draft,
    Scheduled,
    Archived,
    /// Предложение отклонено автором (причина — в `feedback`).
    Rejected,
}

impl ArticleState {
    pub const ALL: [ArticleState; 5] = [
        ArticleState::Suggested,
        ArticleState::Draft,
        ArticleState::Scheduled,
        ArticleState::Archived,
        ArticleState::Rejected,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ArticleState::Suggested => "suggested",
            ArticleState::Draft => "draft",
            ArticleState::Scheduled => "scheduled",
            ArticleState::Archived => "archived",
            ArticleState::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "suggested" => Ok(ArticleState::Suggested),
            "draft" => Ok(ArticleState::Draft),
            "scheduled" => Ok(ArticleState::Scheduled),
            "archived" => Ok(ArticleState::Archived),
            "rejected" => Ok(ArticleState::Rejected),
            other => Err(DomainError::Validation(format!("unknown article state: {other}"))),
        }
    }

    pub fn label_ru(&self) -> &'static str {
        match self {
            ArticleState::Suggested => "Предложение",
            ArticleState::Draft => "Черновик",
            ArticleState::Scheduled => "Запланирован",
            ArticleState::Archived => "В архиве",
            ArticleState::Rejected => "Отклонён",
        }
    }
}

/// Кто создал статью: автор вручную или Claude (предложение/автопубликация).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleOrigin {
    Manual,
    Ai,
}

impl ArticleOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArticleOrigin::Manual => "manual",
            ArticleOrigin::Ai => "ai",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "manual" => Ok(ArticleOrigin::Manual),
            "ai" => Ok(ArticleOrigin::Ai),
            other => Err(DomainError::Validation(format!("unknown article origin: {other}"))),
        }
    }
}

/// Ссылка на источник, на основе которого написан пост (новость, документация, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLink {
    #[serde(default)]
    pub title: String,
    pub url: String,
}

/// Версия промпта, использованная при генерации поста.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptRef {
    pub key: String,
    pub version: i32,
}

/// Агрегированный статус публикации статьи по всем её целевым площадкам
/// (без отменённых задач). Вычисляется в запросе, а не хранится.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PublicationSummary {
    pub total: i64,
    pub published: i64,
    pub failed: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub permalink: Option<String>,
}

impl PublicationSummary {
    /// none | pending | published | partial | failed
    pub fn status(&self) -> &'static str {
        if self.total == 0 {
            "none"
        } else if self.published == self.total {
            "published"
        } else if self.failed > 0 && self.published > 0 {
            "partial"
        } else if self.failed > 0 {
            "failed"
        } else {
            "pending"
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
    pub cover_image_url: Option<String>,
    pub state: ArticleState,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub origin: ArticleOrigin,
    pub tags: Vec<String>,
    pub prompt_key: Option<String>,
    pub prompt_version: Option<i32>,
    pub prompt_refs: Vec<PromptRef>,
    pub rationale: Option<String>,
    pub sources: Vec<SourceLink>,
    pub feedback: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub publication: PublicationSummary,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub cover_media_id: Option<Uuid>,
    pub cover_image_url: Option<String>,
    pub state: ArticleState,
    pub origin: ArticleOrigin,
    pub tags: Vec<String>,
    pub prompt_key: Option<String>,
    pub prompt_version: Option<i32>,
    pub prompt_refs: Vec<PromptRef>,
    pub rationale: Option<String>,
    pub sources: Vec<SourceLink>,
}

impl NewArticle {
    /// Ручной черновик с минимальным набором полей.
    pub fn draft(title: String, slug: String, content_html: String) -> Self {
        Self {
            title,
            slug,
            content_html,
            excerpt: None,
            cover_media_id: None,
            cover_image_url: None,
            state: ArticleState::Draft,
            origin: ArticleOrigin::Manual,
            tags: Vec::new(),
            prompt_key: None,
            prompt_version: None,
            prompt_refs: Vec::new(),
            rationale: None,
            sources: Vec::new(),
        }
    }
}

/// `Option<Option<T>>` для полей, которые можно явно очистить (например, убрать
/// картинку): внешний `Option` — «поле передано в запросе», внутренний — новое значение.
#[derive(Debug, Clone, Default)]
pub struct ArticleUpdate {
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub excerpt: Option<Option<String>>,
    pub cover_media_id: Option<Option<Uuid>>,
    pub cover_image_url: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub rationale: Option<Option<String>>,
    pub sources: Option<Vec<SourceLink>>,
}

/// Генерирует slug из заголовка: транслитерация кириллицы, латиница/цифры/дефисы
/// и короткий случайный суффикс для уникальности.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    for ch in title.to_lowercase().chars() {
        let mapped: &str = match ch {
            'а' => "a", 'б' => "b", 'в' => "v", 'г' => "g", 'д' => "d", 'е' | 'ё' => "e",
            'ж' => "zh", 'з' => "z", 'и' | 'й' => "i", 'к' => "k", 'л' => "l", 'м' => "m",
            'н' => "n", 'о' => "o", 'п' => "p", 'р' => "r", 'с' => "s", 'т' => "t", 'у' => "u",
            'ф' => "f", 'х' => "h", 'ц' => "ts", 'ч' => "ch", 'ш' => "sh", 'щ' => "sch",
            'ы' => "y", 'э' => "e", 'ю' => "yu", 'я' => "ya", 'ъ' | 'ь' => "",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => "-",
        };
        out.push_str(mapped);
    }
    let mut slug = String::new();
    for part in out.split('-').filter(|p| !p.is_empty()) {
        if slug.len() + part.len() > 60 {
            break;
        }
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(part);
    }
    if slug.is_empty() {
        slug.push_str("post");
    }
    let suffix = Uuid::new_v4().simple().to_string();
    format!("{slug}-{}", &suffix[..6])
}

/// Ссылки (источники, обложка) — только http(s): они попадают в `href`/`src` интерфейса.
pub fn validate_url(url: &str, what: &str) -> Result<(), DomainError> {
    let u = url.trim();
    let ok = (u.starts_with("https://") || u.starts_with("http://")) && u.len() > 8 && !u.chars().any(char::is_whitespace);
    if ok {
        Ok(())
    } else {
        Err(DomainError::Validation(format!("{what} must be an http(s) URL: {url}")))
    }
}

pub fn validate_sources(sources: &[SourceLink]) -> Result<(), DomainError> {
    sources.iter().try_for_each(|s| validate_url(&s.url, "source url"))
}

/// Нормализует теги: обрезка, нижний регистр, без `#`, без дублей и пустых.
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let t = tag.trim().trim_start_matches('#').trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_transliterates_and_appends_suffix() {
        let slug = slugify("Новости Rust 1.90: что нового?");
        assert!(slug.starts_with("novosti-rust-1-90-chto-novogo-"), "{slug}");
    }

    #[test]
    fn urls_must_be_http() {
        assert!(validate_url("https://blog.rust-lang.org", "x").is_ok());
        assert!(validate_url("javascript:alert(1)", "x").is_err());
        assert!(validate_url("https://a b", "x").is_err());
    }

    #[test]
    fn normalize_tags_dedupes() {
        assert_eq!(normalize_tags(vec!["#Rust".into(), "rust ".into(), "".into(), "News".into()]), vec!["rust", "news"]);
    }

    #[test]
    fn publication_status() {
        let mut s = PublicationSummary::default();
        assert_eq!(s.status(), "none");
        s.total = 2;
        assert_eq!(s.status(), "pending");
        s.published = 1;
        s.failed = 1;
        assert_eq!(s.status(), "partial");
        s.published = 2;
        s.failed = 0;
        assert_eq!(s.status(), "published");
    }
}
