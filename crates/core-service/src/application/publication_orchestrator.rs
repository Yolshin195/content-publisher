use std::sync::Arc;

use tracing::{error, info, warn};
use uuid::Uuid;

use super::ports::{ArticleRepository, LogRepository, PublicationGateway, TargetRepository, TaskRepository};
use crate::domain::{DomainError, LogStatus, TaskStatus, VideoPlatform};
use shared_contracts::{PublishContent, PublishRequest, PublishStatus, PublishTargetRef, VideoLinkDto};

/// Ядро логики публикации: и cron-джоба, и ручной `publish-now` вызывают один и тот же
/// метод `process_task`, так что логика повторных попыток/логирования не дублируется.
pub struct PublicationOrchestrator {
    articles: Arc<dyn ArticleRepository>,
    targets: Arc<dyn TargetRepository>,
    tasks: Arc<dyn TaskRepository>,
    logs: Arc<dyn LogRepository>,
    gateway: Arc<dyn PublicationGateway>,
    max_attempts: i32,
}

impl PublicationOrchestrator {
    pub fn new(
        articles: Arc<dyn ArticleRepository>,
        targets: Arc<dyn TargetRepository>,
        tasks: Arc<dyn TaskRepository>,
        logs: Arc<dyn LogRepository>,
        gateway: Arc<dyn PublicationGateway>,
        max_attempts: i32,
    ) -> Self {
        Self { articles, targets, tasks, logs, gateway, max_attempts }
    }

    /// Вызывается планировщиком по тику: выбирает due-задачи и публикует их.
    pub async fn tick(&self) {
        let due = match self.tasks.fetch_due(50).await {
            Ok(d) => d,
            Err(e) => {
                error!(error = %e, "failed to fetch due publication tasks");
                return;
            }
        };

        for (task, _scheduled_at) in due {
            if let Err(e) = self.process_task(task.id).await {
                warn!(task_id = %task.id, error = %e, "publication task failed");
            }
        }
    }

    /// Немедленная публикация статьи во все её незавершённые задачи, минуя расписание.
    pub async fn publish_article_now(&self, article_id: Uuid) -> Result<(), DomainError> {
        let tasks = self.tasks.list_by_article(article_id).await?;
        for task in tasks {
            if matches!(task.status, TaskStatus::Pending | TaskStatus::Failed) {
                self.process_task(task.id).await?;
            }
        }
        Ok(())
    }

    async fn process_task(&self, task_id: Uuid) -> Result<(), DomainError> {
        let task = self.tasks.get(task_id).await?;
        let article = self.articles.get(task.article_id).await?;
        let target = self.targets.get(task.target_id).await?;

        if !target.is_active {
            self.tasks.mark_failed(task.id, "target is inactive".into(), true).await?;
            return Ok(());
        }

        self.tasks.mark_publishing(task.id).await?;

        let video_links: Vec<VideoLinkDto> = self
            .articles
            .list_video_links(article.id)
            .await?
            .into_iter()
            .map(|v| VideoLinkDto { platform: map_video_platform(v.platform), url: v.url, is_primary: v.is_primary })
            .collect();

        let request = PublishRequest {
            task_id: task.id,
            target: PublishTargetRef { external_id: target.external_id.clone(), config: target.config.clone() },
            content: PublishContent {
                title: article.title.clone(),
                body_html: article.content_html.clone(),
                // TODO: резолвится через MediaClient по article.cover_media_id, когда Media Service подключён
                cover_image_url: None,
                video_links,
            },
        };

        let attempt_no = task.attempts + 1;

        match self.gateway.publish(target.platform.as_str(), request).await {
            Ok(result) if matches!(result.status, PublishStatus::Success) => {
                self.tasks
                    .mark_published(task.id, result.external_post_id.clone(), result.permalink.clone())
                    .await?;
                self.logs.record(task.id, attempt_no, LogStatus::Success, None, None).await?;
                info!(task_id = %task.id, "article published successfully");
            }
            Ok(result) => {
                let terminal = attempt_no >= self.max_attempts;
                let err = result.error.unwrap_or_else(|| "unknown gateway error".into());
                self.tasks.mark_failed(task.id, err.clone(), terminal).await?;
                self.logs.record(task.id, attempt_no, LogStatus::Failure, Some(err), None).await?;
            }
            Err(e) => {
                let terminal = attempt_no >= self.max_attempts;
                self.tasks.mark_failed(task.id, e.to_string(), terminal).await?;
                self.logs.record(task.id, attempt_no, LogStatus::Failure, Some(e.to_string()), None).await?;
            }
        }

        Ok(())
    }
}

fn map_video_platform(p: VideoPlatform) -> shared_contracts::VideoPlatform {
    match p {
        VideoPlatform::Youtube => shared_contracts::VideoPlatform::Youtube,
        VideoPlatform::VkVideo => shared_contracts::VideoPlatform::VkVideo,
        VideoPlatform::Rutube => shared_contracts::VideoPlatform::Rutube,
        VideoPlatform::Vimeo => shared_contracts::VideoPlatform::Vimeo,
        VideoPlatform::Other => shared_contracts::VideoPlatform::Other,
    }
}
