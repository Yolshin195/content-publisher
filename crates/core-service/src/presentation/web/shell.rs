use crate::application::ports::ArticleFilter;
use crate::state::AppState;

/// Общие данные каркаса страницы (навигация, бейджи, настройки клиента).
pub struct Shell {
    pub active: &'static str,
    pub inbox_count: i64,
    pub tz: String,
    pub auth_enabled: bool,
    pub media_enabled: bool,
    pub asset_version: String,
}

impl Shell {
    pub async fn load(state: &AppState, active: &'static str) -> Self {
        let inbox_count = state
            .article_service
            .list(ArticleFilter { state: Some("suggested".into()), page: 1, per_page: 1, ..Default::default() })
            .await
            .map(|(_, total)| total)
            .unwrap_or(0);
        Self {
            active,
            inbox_count,
            tz: state.config.timezone.to_string(),
            auth_enabled: state.config.auth_enabled(),
            media_enabled: state.config.media_service_url.is_some(),
            asset_version: state.config.asset_version.clone(),
        }
    }
}

/// Отрисовка шаблона с понятной страницей ошибки вместо паники.
pub fn render<T: askama::Template>(tpl: &T) -> axum::response::Html<String> {
    axum::response::Html(tpl.render().unwrap_or_else(|e| {
        tracing::error!(error = %e, "template render failed");
        format!("<h1>Ошибка шаблона</h1><pre>{e}</pre>")
    }))
}

pub fn error_page(message: impl std::fmt::Display) -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::response::Html(format!(
            "<!doctype html><meta charset=utf-8><link rel=stylesheet href=/static/css/app.css>\
             <div style='font-family:system-ui;padding:3rem;text-align:center'><h1>Не найдено</h1><p>{message}</p>\
             <p><a href='/'>На главную</a></p></div>"
        )),
    )
        .into_response()
}
