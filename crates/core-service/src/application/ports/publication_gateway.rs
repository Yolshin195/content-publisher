use async_trait::async_trait;
use shared_contracts::{PublishRequest, PublishResult};

use crate::domain::DomainError;

/// Единый порт для обращения к любому Publisher-микросервису (Telegram, VK, ...).
/// Реализация в infrastructure — простой HTTP-вызов по реестру `platform -> base_url`.
#[async_trait]
pub trait PublicationGateway: Send + Sync {
    async fn publish(&self, platform: &str, request: PublishRequest) -> Result<PublishResult, DomainError>;
}
