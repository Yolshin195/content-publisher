use async_trait::async_trait;
use shared_contracts::MediaUploadUrlResponse;

use crate::domain::DomainError;

/// Порт для обращения к Media Service. Core Service рассчитан на его отсутствие:
/// без настроенного MEDIA_SERVICE_URL статьи просто создаются без картинки.
#[async_trait]
pub trait MediaClient: Send + Sync {
    async fn create_upload_url(&self) -> Result<MediaUploadUrlResponse, DomainError>;
}
