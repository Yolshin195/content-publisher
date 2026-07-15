use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use super::ports::TargetRepository;
use crate::domain::{DomainError, NewPublicationTarget, PublicationTarget};

pub struct TargetService {
    repo: Arc<dyn TargetRepository>,
}

impl TargetService {
    pub fn new(repo: Arc<dyn TargetRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, new_target: NewPublicationTarget) -> Result<PublicationTarget, DomainError> {
        if new_target.external_id.trim().is_empty() {
            return Err(DomainError::Validation("external_id must not be empty".into()));
        }
        self.repo.create(new_target).await
    }

    pub async fn get(&self, id: Uuid) -> Result<PublicationTarget, DomainError> {
        self.repo.get(id).await
    }

    pub async fn list(&self) -> Result<Vec<PublicationTarget>, DomainError> {
        self.repo.list().await
    }

    pub async fn update(
        &self,
        id: Uuid,
        display_name: Option<String>,
        is_active: Option<bool>,
        config: Option<Value>,
    ) -> Result<PublicationTarget, DomainError> {
        self.repo.update(id, display_name, is_active, config).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.repo.delete(id).await
    }
}
