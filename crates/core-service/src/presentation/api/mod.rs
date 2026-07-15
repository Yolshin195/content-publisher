pub mod articles;
pub mod calendar;
pub mod media;
pub mod openapi;
pub mod targets;
pub mod tasks;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/articles", get(articles::list_articles).post(articles::create_article))
        .route(
            "/api/articles/{id}",
            get(articles::get_article).patch(articles::update_article).delete(articles::delete_article),
        )
        .route("/api/articles/{id}/video-links", post(articles::add_video_link))
        .route("/api/articles/{id}/video-links/{link_id}", axum::routing::delete(articles::remove_video_link))
        .route("/api/articles/{id}/schedule", post(tasks::schedule_article))
        .route("/api/articles/{id}/publish-now", post(tasks::publish_now))
        .route("/api/articles/{id}/tasks", get(tasks::list_tasks))
        .route(
            "/api/articles/{id}/targets/{target_id}",
            axum::routing::delete(tasks::cancel_target),
        )
        .route("/api/articles/{id}/targets/{target_id}/retry", post(tasks::retry_target))
        .route("/api/tasks/{task_id}/logs", get(tasks::task_logs))
        .route("/api/targets", get(targets::list_targets).post(targets::create_target))
        .route(
            "/api/targets/{id}",
            get(targets::get_target).put(targets::update_target).delete(targets::delete_target),
        )
        .route("/api/calendar/month", get(calendar::month_view))
        .route("/api/calendar/week", get(calendar::week_view))
        .route("/api/calendar/day", get(calendar::day_view))
        .route("/api/media/upload-url", post(media::create_upload_url))
}
