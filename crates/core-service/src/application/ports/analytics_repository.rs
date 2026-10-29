use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::DomainError;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WeekBucket {
    pub week_start: NaiveDate,
    pub published: i64,
    pub failed: i64,
    pub created_ai: i64,
    pub created_manual: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PromptStat {
    pub key: String,
    pub version: Option<i32>,
    pub total: i64,
    pub pending: i64,
    pub accepted: i64,
    pub rejected: i64,
    pub published: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FailureRow {
    pub article_id: Uuid,
    pub title: String,
    pub target_name: String,
    pub error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HourBucket {
    /// 0 = понедельник
    pub weekday: i32,
    pub hour: i32,
    pub count: i64,
}

/// Сводка для страницы аналитики. Все даты/часы — в часовом поясе приложения.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsOverview {
    pub state_counts: Vec<LabelCount>,
    pub total_posts: i64,
    pub published_total: i64,
    pub published_last_30d: i64,
    pub scheduled_upcoming: i64,
    pub suggestions_pending: i64,
    pub suggestions_accepted: i64,
    pub suggestions_rejected: i64,
    pub failed_tasks: i64,
    pub weeks: Vec<WeekBucket>,
    pub prompts: Vec<PromptStat>,
    pub tags: Vec<LabelCount>,
    pub publish_hours: Vec<HourBucket>,
    pub recent_failures: Vec<FailureRow>,
}

#[async_trait]
pub trait AnalyticsRepository: Send + Sync {
    async fn overview(&self, tz: &str, weeks: i32) -> Result<AnalyticsOverview, DomainError>;
}
