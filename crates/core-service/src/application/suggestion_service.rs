use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::ports::{ArticleRepository, TaskRepository};
use super::{ArticleService, PromptService, SchedulingService};
use crate::domain::{
    telegram_html, Article, ArticleOrigin, ArticleState, DomainError, NewArticle, SourceLink,
};

/// Предложение поста от Claude.
#[derive(Debug, Clone, Default)]
pub struct SuggestionInput {
    pub title: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub tags: Vec<String>,
    pub cover_image_url: Option<String>,
    /// Основной промпт, по которому написан пост.
    pub prompt_key: Option<String>,
    /// Другие промпты, которые использовались (например, промпт поиска новостей).
    pub extra_prompt_keys: Vec<String>,
    pub rationale: Option<String>,
    pub sources: Vec<SourceLink>,
    /// Предлагаемое время публикации.
    pub publish_at: Option<DateTime<Utc>>,
    /// `true` — сразу запланировать публикацию на `publish_at` без ревью автора
    /// (утренние новости на автопилоте).
    pub auto_publish: bool,
    pub target_ids: Vec<Uuid>,
}

/// Цикл «Claude предлагает — автор решает»: создание предложений (с проверкой
/// разметки и фиксацией версий промптов), принятие, отклонение с причиной.
pub struct SuggestionService {
    articles: Arc<dyn ArticleRepository>,
    tasks: Arc<dyn TaskRepository>,
    article_service: Arc<ArticleService>,
    prompts: Arc<PromptService>,
    scheduling: Arc<SchedulingService>,
}

impl SuggestionService {
    pub fn new(
        articles: Arc<dyn ArticleRepository>,
        tasks: Arc<dyn TaskRepository>,
        article_service: Arc<ArticleService>,
        prompts: Arc<PromptService>,
        scheduling: Arc<SchedulingService>,
    ) -> Self {
        Self { articles, tasks, article_service, prompts, scheduling }
    }

    pub async fn create(&self, input: SuggestionInput) -> Result<Article, DomainError> {
        let cover = input.cover_image_url.clone().filter(|u| !u.trim().is_empty());
        // Ошибки разметки и длины возвращаем сразу — Claude исправит текст до того,
        // как пост окажется в очереди публикации.
        let problems = telegram_html::check_post(&input.title, &input.content_html, cover.is_some());
        if !problems.is_empty() {
            return Err(DomainError::Validation(format!("исправьте пост: {}", problems.join("; "))));
        }
        if input.auto_publish && input.publish_at.is_none() {
            return Err(DomainError::Validation("auto_publish requires publish_at".into()));
        }

        let mut keys: Vec<String> = input.prompt_key.iter().cloned().collect();
        keys.extend(input.extra_prompt_keys.iter().cloned());
        keys.retain(|k| !k.trim().is_empty());
        let refs = self.prompts.collect_refs(&keys).await?;
        let (prompt_key, prompt_version) = match input.prompt_key.as_deref().filter(|k| !k.trim().is_empty()) {
            Some(k) => {
                let r = refs.iter().find(|r| r.key.eq_ignore_ascii_case(k.trim()));
                (r.map(|r| r.key.clone()), r.map(|r| r.version))
            }
            None => (None, None),
        };

        let article = self
            .article_service
            .create(NewArticle {
                title: input.title,
                slug: String::new(),
                content_html: input.content_html,
                excerpt: input.excerpt.filter(|e| !e.trim().is_empty()),
                cover_media_id: None,
                cover_image_url: cover,
                state: ArticleState::Suggested,
                origin: ArticleOrigin::Ai,
                tags: input.tags,
                prompt_key,
                prompt_version,
                prompt_refs: refs,
                rationale: input.rationale.filter(|r| !r.trim().is_empty()),
                sources: input.sources,
            })
            .await?;

        match input.publish_at {
            Some(at) if input.auto_publish => {
                self.scheduling.schedule(article.id, at, input.target_ids).await?;
                self.articles.get(article.id).await
            }
            Some(at) => self.articles.set_scheduled_at(article.id, Some(at)).await,
            None => Ok(article),
        }
    }

    /// Принять предложение: переходит в черновики (дата, предложенная Claude, сохраняется).
    pub async fn accept(&self, id: Uuid, feedback: Option<String>) -> Result<Article, DomainError> {
        let article = self.articles.get(id).await?;
        if !matches!(article.state, ArticleState::Suggested | ArticleState::Rejected) {
            return Err(DomainError::InvalidStateTransition("only suggestions can be accepted".into()));
        }
        self.articles.set_review(id, ArticleState::Draft, feedback.filter(|f| !f.trim().is_empty())).await
    }

    /// Отклонить с причиной — Claude читает причины отказов, чтобы не повторяться.
    pub async fn reject(&self, id: Uuid, feedback: Option<String>) -> Result<Article, DomainError> {
        let article = self.articles.get(id).await?;
        if matches!(article.state, ArticleState::Archived) {
            return Err(DomainError::InvalidStateTransition("archived article cannot be rejected".into()));
        }
        if article.publication.published > 0 {
            return Err(DomainError::InvalidStateTransition("post is already published".into()));
        }
        self.tasks.cancel_open_for_article(id, &[]).await?;
        self.articles.set_review(id, ArticleState::Rejected, feedback.filter(|f| !f.trim().is_empty())).await
    }

    /// Вернуть отклонённое предложение на рассмотрение.
    pub async fn restore(&self, id: Uuid) -> Result<Article, DomainError> {
        let article = self.articles.get(id).await?;
        if article.state != ArticleState::Rejected {
            return Err(DomainError::InvalidStateTransition("only rejected suggestions can be restored".into()));
        }
        self.articles.set_state(id, ArticleState::Suggested, None).await
    }

    pub async fn archive(&self, id: Uuid) -> Result<Article, DomainError> {
        self.tasks.cancel_open_for_article(id, &[]).await?;
        self.articles.set_state(id, ArticleState::Archived, None).await
    }
}
