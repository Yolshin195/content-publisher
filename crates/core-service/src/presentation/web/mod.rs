pub mod article_pages;
pub mod calendar_pages;

use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(calendar_pages::index))
        .route("/calendar/month/{year}/{month}", get(calendar_pages::month_page))
        .route("/calendar/week/{year}/{week}", get(calendar_pages::week_page))
        .route("/calendar/list", get(calendar_pages::list_index))
        .route("/calendar/list/month/{year}/{month}", get(calendar_pages::list_month_page))
        .route("/calendar/list/week/{year}/{week}", get(calendar_pages::list_week_page))
        .route("/articles/new", get(article_pages::new_article_page))
        .route("/articles/{id}/edit", get(article_pages::edit_article_page))
        .route("/articles/{id}", get(article_pages::view_article_page))
}
