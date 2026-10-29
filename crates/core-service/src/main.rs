use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use core_service::config::AppConfig;
use core_service::infrastructure::db::{
    PgAnalyticsRepository, PgArticleRepository, PgInterestRepository, PgLogRepository, PgPromptRepository,
    PgTargetRepository, PgTaskRepository,
};
use core_service::infrastructure::gateway::HttpPublicationGateway;
use core_service::infrastructure::media_client::HttpMediaClient;
use core_service::infrastructure::scheduler::spawn_scheduler;
use core_service::presentation::api::openapi::ApiDoc;
use core_service::state::AppState;
use core_service::{application, presentation, seed};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env();

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    if let Err(problem) = config.validate_auth() {
        // Ошибка настройки, а не сбой: короткое сообщение без backtrace.
        eprintln!("Ошибка конфигурации — небезопасная настройка доступа: {problem}");
        std::process::exit(2);
    }
    if !config.auth_enabled() {
        tracing::warn!(
            host = %config.server_host,
            "AUTH_DISABLED=true — авторизация ОТКЛЮЧЕНА, интерфейс, API и MCP открыты всем; только для локальной разработки"
        );
    }

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let article_repo: Arc<dyn application::ports::ArticleRepository> = Arc::new(PgArticleRepository::new(pool.clone()));
    let target_repo: Arc<dyn application::ports::TargetRepository> = Arc::new(PgTargetRepository::new(pool.clone()));
    let task_repo: Arc<dyn application::ports::TaskRepository> = Arc::new(PgTaskRepository::new(pool.clone()));
    let log_repo: Arc<dyn application::ports::LogRepository> = Arc::new(PgLogRepository::new(pool.clone()));
    let prompt_repo: Arc<dyn application::ports::PromptRepository> = Arc::new(PgPromptRepository::new(pool.clone()));
    let interest_repo: Arc<dyn application::ports::InterestRepository> =
        Arc::new(PgInterestRepository::new(pool.clone()));
    let analytics_repo: Arc<dyn application::ports::AnalyticsRepository> =
        Arc::new(PgAnalyticsRepository::new(pool.clone()));

    let gateway: Arc<dyn application::ports::PublicationGateway> =
        Arc::new(HttpPublicationGateway::new(config.publisher_registry.clone()));
    let media_client: Arc<dyn application::ports::MediaClient> =
        Arc::new(HttpMediaClient::new(config.media_service_url.clone()));

    let orchestrator = Arc::new(application::PublicationOrchestrator::new(
        article_repo.clone(),
        target_repo.clone(),
        task_repo.clone(),
        log_repo.clone(),
        gateway.clone(),
        config.publish_max_attempts,
    ));

    let article_service = Arc::new(application::ArticleService::new(article_repo.clone()));
    let target_service = Arc::new(application::TargetService::new(target_repo.clone()));
    let calendar_service = Arc::new(application::CalendarService::new(article_repo.clone(), config.timezone));
    let scheduling_service = Arc::new(application::SchedulingService::new(
        article_repo.clone(),
        target_repo.clone(),
        task_repo.clone(),
        orchestrator.clone(),
    ));
    let prompt_service = Arc::new(application::PromptService::new(prompt_repo.clone()));
    let suggestion_service = Arc::new(application::SuggestionService::new(
        article_repo.clone(),
        task_repo.clone(),
        article_service.clone(),
        prompt_service.clone(),
        scheduling_service.clone(),
    ));
    let interest_service = Arc::new(application::InterestService::new(interest_repo.clone()));
    let analytics_service =
        Arc::new(application::AnalyticsService::new(analytics_repo.clone(), config.timezone.name().to_string()));

    let seeded = prompt_service.seed_if_empty(seed::PROMPTS).await?;
    if seeded > 0 {
        tracing::info!(count = seeded, "seeded starter prompts");
    }

    spawn_scheduler(orchestrator.clone(), std::time::Duration::from_secs(config.scheduler_tick_interval_secs));

    let server_host = config.server_host.clone();
    let server_port = config.server_port;
    let static_dir = config.static_dir.clone();
    tracing::info!(static_dir = %static_dir.display(), timezone = %config.timezone, "configuration loaded");

    let state = AppState {
        config: Arc::new(config),
        db: pool.clone(),
        article_service,
        target_service,
        calendar_service,
        scheduling_service,
        suggestion_service,
        prompt_service,
        interest_service,
        analytics_service,
        orchestrator,
        media_client,
        log_repo,
    };

    let app = Router::new()
        .merge(presentation::api::router())
        .merge(presentation::web::router())
        .route("/mcp", get(presentation::mcp::handle_get).post(presentation::mcp::handle_post))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(axum::middleware::from_fn_with_state(state.clone(), presentation::auth::auth_middleware))
        .with_state(state)
        // В span пишем только путь: в query у MCP может быть токен.
        .layer(TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
            tracing::info_span!("http", method = %req.method(), path = %req.uri().path())
        }));

    let addr = format!("{server_host}:{server_port}");
    tracing::info!(%addr, "starting core-service");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
