use std::sync::Arc;

use super::ports::InterestRepository;
use crate::domain::{build_trends, DomainError, InterestSnapshot, InterestTrend, NewInterestSnapshot};

pub struct InterestService {
    repo: Arc<dyn InterestRepository>,
}

impl InterestService {
    pub fn new(repo: Arc<dyn InterestRepository>) -> Self {
        Self { repo }
    }

    pub async fn record(&self, mut snapshot: NewInterestSnapshot) -> Result<InterestSnapshot, DomainError> {
        snapshot.topics.retain(|t| !t.name.trim().is_empty());
        if snapshot.topics.is_empty() {
            return Err(DomainError::Validation("at least one topic is required".into()));
        }
        for t in snapshot.topics.iter_mut() {
            t.name = t.name.trim().to_string();
            t.weight = t.weight.clamp(1, 10);
        }
        snapshot.topics.sort_by_key(|t| std::cmp::Reverse(t.weight));
        if snapshot.source.trim().is_empty() {
            snapshot.source = "chats".into();
        }
        self.repo.create(snapshot).await
    }

    pub async fn recent(&self, limit: i64) -> Result<Vec<InterestSnapshot>, DomainError> {
        self.repo.list_recent(limit.clamp(1, 100)).await
    }

    /// Последний снимок + тренды по последним `window` снимкам.
    pub async fn overview(&self, window: i64) -> Result<(Option<InterestSnapshot>, Vec<InterestTrend>), DomainError> {
        let snaps = self.recent(window).await?;
        let trends = build_trends(&snaps);
        Ok((snaps.into_iter().next(), trends))
    }
}
