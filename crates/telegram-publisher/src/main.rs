mod config;
mod handlers;
mod telegram;

use axum::routing::{get, post};
use axum::Router;
use teloxide::Bot;
use tower_http::trace::TraceLayer;

use config::Config;

/// `Bot` внутри teloxide уже дёшево клонируется (обёртка над Arc), поэтому
/// хранить его напрямую в состоянии, без дополнительного Arc, вполне достаточно.
#[derive(Clone)]
pub struct AppState {
    pub bot: Bot,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env();

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let bot = Bot::new(config.bot_token.clone());
    let state = AppState { bot };

    let app = Router::new()
        .route("/publish", post(handlers::publish))
        .route("/health", get(handlers::health))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!(%addr, "starting telegram-publisher");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
