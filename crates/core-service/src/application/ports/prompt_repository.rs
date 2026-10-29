use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{DomainError, NewPromptNode, PromptNode, PromptNodeUpdate, PromptVersion};

#[async_trait]
pub trait PromptRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<PromptNode>, DomainError>;
    async fn get(&self, id: Uuid) -> Result<PromptNode, DomainError>;
    /// Поиск по ключу без учёта регистра.
    async fn get_by_key(&self, key: &str) -> Result<PromptNode, DomainError>;
    /// Создаёт узел; для промпта сразу пишет версию 1 в историю.
    async fn create(&self, node: NewPromptNode) -> Result<PromptNode, DomainError>;
    /// Обновляет узел; если у промпта изменился текст/заголовок/описание —
    /// увеличивает версию и пишет её в историю (в одной транзакции).
    async fn update(&self, id: Uuid, update: PromptNodeUpdate) -> Result<PromptNode, DomainError>;
    async fn move_node(&self, id: Uuid, parent_id: Option<Uuid>, position: i32) -> Result<PromptNode, DomainError>;
    async fn delete(&self, id: Uuid) -> Result<(), DomainError>;
    async fn list_versions(&self, prompt_id: Uuid) -> Result<Vec<PromptVersion>, DomainError>;
    async fn get_version(&self, prompt_id: Uuid, version: i32) -> Result<PromptVersion, DomainError>;
}
