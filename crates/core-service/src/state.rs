use std::sync::Arc;

use crate::application::ports::{LogRepository, MediaClient};
use crate::application::{
    AnalyticsService, ArticleService, CalendarService, InterestService, PromptService, PublicationOrchestrator,
    SchedulingService, SuggestionService, TargetService,
};
use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: sqlx::PgPool,
    pub article_service: Arc<ArticleService>,
    pub target_service: Arc<TargetService>,
    pub calendar_service: Arc<CalendarService>,
    pub scheduling_service: Arc<SchedulingService>,
    pub suggestion_service: Arc<SuggestionService>,
    pub prompt_service: Arc<PromptService>,
    pub interest_service: Arc<InterestService>,
    pub analytics_service: Arc<AnalyticsService>,
    pub orchestrator: Arc<PublicationOrchestrator>,
    pub media_client: Arc<dyn MediaClient>,
    pub log_repo: Arc<dyn LogRepository>,
}
