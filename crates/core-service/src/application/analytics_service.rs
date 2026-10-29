use std::sync::Arc;

use super::ports::{AnalyticsOverview, AnalyticsRepository};
use crate::domain::DomainError;

pub struct AnalyticsService {
    repo: Arc<dyn AnalyticsRepository>,
    tz: String,
}

impl AnalyticsService {
    pub fn new(repo: Arc<dyn AnalyticsRepository>, tz: String) -> Self {
        Self { repo, tz }
    }

    pub async fn overview(&self, weeks: i32) -> Result<AnalyticsOverview, DomainError> {
        self.repo.overview(&self.tz, weeks).await
    }
}
