use std::collections::HashMap;

use async_trait::async_trait;
use shared_contracts::{PublishRequest, PublishResult};

use crate::application::ports::PublicationGateway;
use crate::domain::DomainError;

/// Реализация порта `PublicationGateway` поверх простого HTTP-вызова.
/// Реестр `platform -> base_url` заполняется из конфигурации при старте.
/// Это самый простой транспорт для старта; при необходимости заменяется на
/// адаптер поверх очереди сообщений (NATS/RabbitMQ) без изменений в domain/application.
pub struct HttpPublicationGateway {
    client: reqwest::Client,
    registry: HashMap<String, String>,
}

impl HttpPublicationGateway {
    pub fn new(registry: HashMap<String, String>) -> Self {
        Self { client: reqwest::Client::new(), registry }
    }
}

#[async_trait]
impl PublicationGateway for HttpPublicationGateway {
    async fn publish(&self, platform: &str, request: PublishRequest) -> Result<PublishResult, DomainError> {
        let base_url = self
            .registry
            .get(platform)
            .ok_or_else(|| DomainError::Gateway(format!("no publisher registered for platform '{platform}'")))?;

        let response = self
            .client
            .post(format!("{base_url}/publish"))
            .json(&request)
            .send()
            .await
            .map_err(|e| DomainError::Gateway(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DomainError::Gateway(format!("publisher responded with status {}", response.status())));
        }

        response.json::<PublishResult>().await.map_err(|e| DomainError::Gateway(e.to_string()))
    }
}
