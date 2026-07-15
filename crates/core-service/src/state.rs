use std::sync::Arc;

use crate::application::ports::{LogRepository, MediaClient};
use crate::application::{ArticleService, CalendarService, PublicationOrchestrator, SchedulingService, TargetService};
use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub article_service: Arc<ArticleService>,
    pub target_service: Arc<TargetService>,
    pub calendar_service: Arc<CalendarService>,
    pub scheduling_service: Arc<SchedulingService>,
    pub orchestrator: Arc<PublicationOrchestrator>,
    pub media_client: Arc<dyn MediaClient>,
    pub log_repo: Arc<dyn LogRepository>,
}
