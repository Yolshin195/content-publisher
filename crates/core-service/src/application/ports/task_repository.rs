use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{DomainError, PublicationTask};

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, article_id: Uuid, target_id: Uuid) -> Result<PublicationTask, DomainError>;
    async fn get(&self, id: Uuid) -> Result<PublicationTask, DomainError>;
    async fn get_by_article_and_target(&self, article_id: Uuid, target_id: Uuid) -> Result<PublicationTask, DomainError>;
    async fn list_by_article(&self, article_id: Uuid) -> Result<Vec<PublicationTask>, DomainError>;
    /// Возвращает задачи, готовые к публикации (статья запланирована и время наступило),
    /// с блокировкой строк `FOR UPDATE SKIP LOCKED` — защита от двойной публикации
    /// при нескольких инстансах Core Service.
    async fn fetch_due(&self, limit: i64) -> Result<Vec<(PublicationTask, DateTime<Utc>)>, DomainError>;
    async fn mark_publishing(&self, id: Uuid) -> Result<(), DomainError>;
    async fn mark_published(
        &self,
        id: Uuid,
        external_post_id: Option<String>,
        permalink: Option<String>,
    ) -> Result<(), DomainError>;
    async fn mark_failed(&self, id: Uuid, error: String, terminal: bool) -> Result<(), DomainError>;
    async fn reset_to_pending(&self, id: Uuid) -> Result<(), DomainError>;
    async fn cancel(&self, id: Uuid) -> Result<(), DomainError>;
}
