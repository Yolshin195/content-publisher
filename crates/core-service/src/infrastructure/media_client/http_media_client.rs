use async_trait::async_trait;
use shared_contracts::MediaUploadUrlResponse;

use crate::application::ports::MediaClient;
use crate::domain::DomainError;

/// Реализация порта `MediaClient` поверх HTTP-вызова к Media Service.
/// Если `base_url` не задан (сервис ещё не поднят), возвращает понятную ошибку,
/// а не паникует — Core Service работает и без Media Service, просто без картинок.
pub struct HttpMediaClient {
    client: reqwest::Client,
    base_url: Option<String>,
}

impl HttpMediaClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[async_trait]
impl MediaClient for HttpMediaClient {
    async fn create_upload_url(&self) -> Result<MediaUploadUrlResponse, DomainError> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| DomainError::Gateway("MEDIA_SERVICE_URL is not configured".into()))?;

        let response = self
            .client
            .post(format!("{base_url}/media/upload-url"))
            .send()
            .await
            .map_err(|e| DomainError::Gateway(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DomainError::Gateway(format!("media service responded with status {}", response.status())));
        }

        response.json::<MediaUploadUrlResponse>().await.map_err(|e| DomainError::Gateway(e.to_string()))
    }
}
