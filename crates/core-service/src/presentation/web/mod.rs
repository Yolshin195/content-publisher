pub mod analytics_page;
pub mod calendar_pages;
pub mod format;
pub mod inbox;
pub mod posts;
pub mod prompts_page;
pub mod settings_page;
pub mod shell;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { Redirect::to("/inbox") }))
        .route("/health", get(health))
        .route("/login", get(super::auth::login_page).post(super::auth::login_submit))
        .route("/logout", post(super::auth::logout))
        .route("/inbox", get(inbox::inbox_page))
        .route("/calendar", get(calendar_pages::index))
        .route("/calendar/month/{year}/{month}", get(calendar_pages::month_page))
        .route("/calendar/week/{year}/{week}", get(calendar_pages::week_page))
        .route("/calendar/list", get(calendar_pages::list_index))
        .route("/calendar/list/month/{year}/{month}", get(calendar_pages::list_month_page))
        .route("/calendar/list/week/{year}/{week}", get(calendar_pages::list_week_page))
        .route("/posts", get(posts::posts_page))
        .route("/articles/new", get(posts::new_article_page))
        .route("/articles/{id}", get(posts::article_page))
        .route("/articles/{id}/edit", get(posts::edit_redirect))
        .route("/prompts", get(prompts_page::prompts_page))
        .route("/analytics", get(analytics_page::analytics_page))
        .route("/settings", get(settings_page::settings_page))
}

/// Проверка живости для Docker healthcheck: сервис отвечает и БД доступна.
async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "status": "error", "error": e.to_string() }))),
    }
}
