use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::ports::{ArticleRepository, TargetRepository, TaskRepository};
use crate::domain::{ArticleState, DomainError, PublicationTask, TaskStatus};

/// Связывает статью с целевыми площадками публикации: создание задач `PublicationTask`
/// при планировании, повторные попытки и отмену публикации в конкретную цель.
pub struct SchedulingService {
    articles: Arc<dyn ArticleRepository>,
    targets: Arc<dyn TargetRepository>,
    tasks: Arc<dyn TaskRepository>,
}

impl SchedulingService {
    pub fn new(
        articles: Arc<dyn ArticleRepository>,
        targets: Arc<dyn TargetRepository>,
        tasks: Arc<dyn TaskRepository>,
    ) -> Self {
        Self { articles, targets, tasks }
    }

    pub async fn schedule(
        &self,
        article_id: Uuid,
        scheduled_at: DateTime<Utc>,
        target_ids: Vec<Uuid>,
    ) -> Result<Vec<PublicationTask>, DomainError> {
        if target_ids.is_empty() {
            return Err(DomainError::Validation("at least one target_id is required".into()));
        }

        for target_id in &target_ids {
            self.targets.get(*target_id).await?;
        }

        self.articles
            .set_state(article_id, ArticleState::Scheduled, Some(scheduled_at))
            .await?;

        let mut created = Vec::with_capacity(target_ids.len());
        for target_id in target_ids {
            created.push(self.tasks.create(article_id, target_id).await?);
        }
        Ok(created)
    }

    pub async fn retry_by_target(&self, article_id: Uuid, target_id: Uuid) -> Result<(), DomainError> {
        let task = self.tasks.get_by_article_and_target(article_id, target_id).await?;
        if task.status != TaskStatus::Failed {
            return Err(DomainError::InvalidStateTransition(
                "only failed tasks can be retried".into(),
            ));
        }
        self.tasks.reset_to_pending(task.id).await
    }

    pub async fn cancel_by_target(&self, article_id: Uuid, target_id: Uuid) -> Result<(), DomainError> {
        let task = self.tasks.get_by_article_and_target(article_id, target_id).await?;
        self.tasks.cancel(task.id).await
    }

    pub async fn list_tasks(&self, article_id: Uuid) -> Result<Vec<PublicationTask>, DomainError> {
        self.tasks.list_by_article(article_id).await
    }
}
