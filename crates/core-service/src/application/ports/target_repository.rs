use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::{DomainError, NewPublicationTarget, PublicationTarget};

#[async_trait]
pub trait TargetRepository: Send + Sync {
    async fn create(&self, new_target: NewPublicationTarget) -> Result<PublicationTarget, DomainError>;
    async fn get(&self, id: Uuid) -> Result<PublicationTarget, DomainError>;
    async fn list(&self) -> Result<Vec<PublicationTarget>, DomainError>;
    async fn update(
        &self,
        id: Uuid,
        display_name: Option<String>,
        is_active: Option<bool>,
        config: Option<Value>,
    ) -> Result<PublicationTarget, DomainError>;
    async fn delete(&self, id: Uuid) -> Result<(), DomainError>;
}
