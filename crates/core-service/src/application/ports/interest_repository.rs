use async_trait::async_trait;

use crate::domain::{DomainError, InterestSnapshot, NewInterestSnapshot};

#[async_trait]
pub trait InterestRepository: Send + Sync {
    async fn create(&self, snapshot: NewInterestSnapshot) -> Result<InterestSnapshot, DomainError>;
    /// Последние снимки, от новых к старым.
    async fn list_recent(&self, limit: i64) -> Result<Vec<InterestSnapshot>, DomainError>;
}
