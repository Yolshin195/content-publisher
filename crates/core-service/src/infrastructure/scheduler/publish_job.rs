use std::sync::Arc;
use std::time::Duration;

use tracing::info;

use crate::application::PublicationOrchestrator;

/// Простой интервальный планировщик поверх `tokio::time::interval`.
/// Для полноценных cron-выражений (не только фиксированного интервала)
/// можно заменить на `tokio-cron-scheduler`, не меняя `PublicationOrchestrator`.
pub fn spawn_scheduler(orchestrator: Arc<PublicationOrchestrator>, tick_interval: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_interval);
        info!(interval_secs = tick_interval.as_secs(), "publication scheduler started");
        loop {
            interval.tick().await;
            orchestrator.tick().await;
        }
    });
}
