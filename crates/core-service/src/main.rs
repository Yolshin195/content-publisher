use std::sync::Arc;

use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use core_service::config::AppConfig;
use core_service::infrastructure::db::{PgArticleRepository, PgLogRepository, PgTargetRepository, PgTaskRepository};
use core_service::infrastructure::gateway::HttpPublicationGateway;
use core_service::infrastructure::media_client::HttpMediaClient;
use core_service::infrastructure::scheduler::spawn_scheduler;
use core_service::presentation::api::openapi::ApiDoc;
use core_service::state::AppState;
use core_service::{application, presentation};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env();

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let article_repo: Arc<dyn application::ports::ArticleRepository> = Arc::new(PgArticleRepository::new(pool.clone()));
    let target_repo: Arc<dyn application::ports::TargetRepository> = Arc::new(PgTargetRepository::new(pool.clone()));
    let task_repo: Arc<dyn application::ports::TaskRepository> = Arc::new(PgTaskRepository::new(pool.clone()));
    let log_repo: Arc<dyn application::ports::LogRepository> = Arc::new(PgLogRepository::new(pool.clone()));

    let gateway: Arc<dyn application::ports::PublicationGateway> =
        Arc::new(HttpPublicationGateway::new(config.publisher_registry.clone()));
    let media_client: Arc<dyn application::ports::MediaClient> =
        Arc::new(HttpMediaClient::new(config.media_service_url.clone()));

    let article_service = Arc::new(application::ArticleService::new(article_repo.clone()));
    let target_service = Arc::new(application::TargetService::new(target_repo.clone()));
    let calendar_service = Arc::new(application::CalendarService::new(article_repo.clone()));
    let scheduling_service =
        Arc::new(application::SchedulingService::new(article_repo.clone(), target_repo.clone(), task_repo.clone()));

    let orchestrator = Arc::new(application::PublicationOrchestrator::new(
        article_repo.clone(),
        target_repo.clone(),
        task_repo.clone(),
        log_repo.clone(),
        gateway.clone(),
        config.publish_max_attempts,
    ));

    spawn_scheduler(orchestrator.clone(), std::time::Duration::from_secs(config.scheduler_tick_interval_secs));

    let server_host = config.server_host.clone();
    let server_port = config.server_port;

    let state = AppState {
        config: Arc::new(config),
        article_service,
        target_service,
        calendar_service,
        scheduling_service,
        orchestrator,
        media_client,
        log_repo,
    };

    let app = Router::new()
        .merge(presentation::api::router())
        .merge(presentation::web::router())
        .merge(SwaggerUi::new("/swagger-ui").url("/swagger-ui/openapi.json", ApiDoc::openapi()))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("{server_host}:{server_port}");
    tracing::info!(%addr, "starting core-service");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
