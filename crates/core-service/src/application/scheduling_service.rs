use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::ports::{ArticleRepository, TargetRepository, TaskRepository};
use super::PublicationOrchestrator;
use crate::domain::{telegram_html, Article, ArticleState, DomainError, PublicationTask, TaskStatus};

/// Связывает статью с целевыми площадками публикации: создание задач `PublicationTask`
/// при планировании, перенос, снятие с публикации, повторные попытки и отмену.
pub struct SchedulingService {
    articles: Arc<dyn ArticleRepository>,
    targets: Arc<dyn TargetRepository>,
    tasks: Arc<dyn TaskRepository>,
    orchestrator: Arc<PublicationOrchestrator>,
}

impl SchedulingService {
    pub fn new(
        articles: Arc<dyn ArticleRepository>,
        targets: Arc<dyn TargetRepository>,
        tasks: Arc<dyn TaskRepository>,
        orchestrator: Arc<PublicationOrchestrator>,
    ) -> Self {
        Self { articles, targets, tasks, orchestrator }
    }

    /// Пустой `target_ids` — все активные площадки (типичный случай: один канал).
    async fn resolve_targets(&self, target_ids: Vec<Uuid>) -> Result<Vec<Uuid>, DomainError> {
        if target_ids.is_empty() {
            let active: Vec<Uuid> = self.targets.list().await?.into_iter().filter(|t| t.is_active).map(|t| t.id).collect();
            if active.is_empty() {
                return Err(DomainError::Validation(
                    "нет активных площадок публикации — добавьте канал в Настройках".into(),
                ));
            }
            return Ok(active);
        }
        for target_id in &target_ids {
            self.targets.get(*target_id).await?;
        }
        Ok(target_ids)
    }

    fn ensure_publishable(article: &Article) -> Result<(), DomainError> {
        let has_cover = article.cover_image_url.is_some();
        let problems = telegram_html::check_post(&article.title, &article.content_html, has_cover);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(DomainError::Validation(format!("пост не пройдёт в Telegram: {}", problems.join("; "))))
        }
    }

    pub async fn schedule(
        &self,
        article_id: Uuid,
        scheduled_at: DateTime<Utc>,
        target_ids: Vec<Uuid>,
    ) -> Result<Vec<PublicationTask>, DomainError> {
        let article = self.articles.get(article_id).await?;
        Self::ensure_publishable(&article)?;
        let target_ids = self.resolve_targets(target_ids).await?;

        self.articles.set_state(article_id, ArticleState::Scheduled, Some(scheduled_at)).await?;
        // Площадки, снятые при перепланировании, больше не публикуем.
        self.tasks.cancel_open_for_article(article_id, &target_ids).await?;

        let mut created = Vec::with_capacity(target_ids.len());
        for target_id in target_ids {
            created.push(self.tasks.create(article_id, target_id).await?);
        }
        Ok(created)
    }

    /// Снять с публикации: статья возвращается в черновики (дата сохраняется как
    /// плановая), незавершённые задачи отменяются.
    pub async fn unschedule(&self, article_id: Uuid) -> Result<Article, DomainError> {
        let article = self.articles.get(article_id).await?;
        if article.state != ArticleState::Scheduled {
            return Err(DomainError::InvalidStateTransition("article is not scheduled".into()));
        }
        self.tasks.cancel_open_for_article(article_id, &[]).await?;
        self.articles.set_state(article_id, ArticleState::Draft, None).await
    }

    /// Перенос даты (или очистка плановой даты черновика). Задачи запланированной
    /// статьи подхватят новое время автоматически — оно читается из статьи.
    pub async fn reschedule(&self, article_id: Uuid, at: Option<DateTime<Utc>>) -> Result<Article, DomainError> {
        let article = self.articles.get(article_id).await?;
        if at.is_none() && article.state == ArticleState::Scheduled {
            return Err(DomainError::Validation(
                "у запланированного поста нельзя убрать дату — сначала снимите его с публикации".into(),
            ));
        }
        self.articles.set_scheduled_at(article_id, at).await
    }

    /// Опубликовать немедленно. Если указаны площадки (или задач ещё нет) —
    /// сначала планирует на «сейчас», затем сразу отправляет.
    pub async fn publish_now(&self, article_id: Uuid, target_ids: Option<Vec<Uuid>>) -> Result<Vec<PublicationTask>, DomainError> {
        let open_tasks = self
            .tasks
            .list_by_article(article_id)
            .await?
            .into_iter()
            .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Failed))
            .count();
        if target_ids.is_some() || open_tasks == 0 {
            self.schedule(article_id, Utc::now(), target_ids.unwrap_or_default()).await?;
        } else {
            let article = self.articles.get(article_id).await?;
            Self::ensure_publishable(&article)?;
            self.articles.set_state(article_id, ArticleState::Scheduled, Some(Utc::now())).await?;
        }
        self.orchestrator.publish_article_now(article_id).await?;
        self.tasks.list_by_article(article_id).await
    }

    pub async fn retry_by_target(&self, article_id: Uuid, target_id: Uuid) -> Result<(), DomainError> {
        let task = self.tasks.get_by_article_and_target(article_id, target_id).await?;
        if task.status != TaskStatus::Failed {
            return Err(DomainError::InvalidStateTransition("only failed tasks can be retried".into()));
        }
        self.tasks.reset_to_pending(task.id).await
    }

    pub async fn cancel_by_target(&self, article_id: Uuid, target_id: Uuid) -> Result<(), DomainError> {
        let task = self.tasks.get_by_article_and_target(article_id, target_id).await?;
        if task.status == TaskStatus::Published {
            return Err(DomainError::InvalidStateTransition("task is already published".into()));
        }
        self.tasks.cancel(task.id).await
    }

    pub async fn list_tasks(&self, article_id: Uuid) -> Result<Vec<PublicationTask>, DomainError> {
        self.tasks.list_by_article(article_id).await
    }
}
