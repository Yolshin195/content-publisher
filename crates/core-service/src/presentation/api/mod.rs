pub mod analytics;
pub mod articles;
pub mod calendar;
pub mod interests;
pub mod media;
pub mod openapi;
pub mod prompts;
pub mod targets;
pub mod tasks;

use axum::routing::{delete, get, post};
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
        .route("/api/articles/{id}/video-links/{link_id}", delete(articles::remove_video_link))
        .route("/api/articles/{id}/accept", post(articles::accept_article))
        .route("/api/articles/{id}/reject", post(articles::reject_article))
        .route("/api/articles/{id}/restore", post(articles::restore_article))
        .route("/api/articles/{id}/archive", post(articles::archive_article))
        .route("/api/articles/{id}/schedule", post(tasks::schedule_article))
        .route("/api/articles/{id}/unschedule", post(tasks::unschedule))
        .route("/api/articles/{id}/publish-now", post(tasks::publish_now))
        .route("/api/articles/{id}/tasks", get(tasks::list_tasks))
        .route("/api/articles/{id}/targets/{target_id}", delete(tasks::cancel_target))
        .route("/api/articles/{id}/targets/{target_id}/retry", post(tasks::retry_target))
        .route("/api/suggestions", post(articles::create_suggestion))
        .route("/api/tags", get(articles::list_tags))
        .route("/api/tasks/{task_id}/logs", get(tasks::task_logs))
        .route("/api/targets", get(targets::list_targets).post(targets::create_target))
        .route(
            "/api/targets/{id}",
            get(targets::get_target).put(targets::update_target).delete(targets::delete_target),
        )
        .route("/api/prompts", get(prompts::tree).post(prompts::create))
        .route("/api/prompts/export", get(prompts::export_all))
        .route("/api/prompts/import", post(prompts::import))
        .route("/api/prompts/by-key/{key}", get(prompts::get_by_key).put(prompts::save_by_key))
        .route("/api/prompts/{id}", get(prompts::get_prompt).patch(prompts::update).delete(prompts::delete))
        .route("/api/prompts/{id}/move", post(prompts::move_node))
        .route("/api/prompts/{id}/versions", get(prompts::versions))
        .route("/api/prompts/{id}/diff", get(prompts::diff))
        .route("/api/prompts/{id}/restore/{version}", post(prompts::restore))
        .route("/api/prompts/{id}/markdown", get(prompts::export_one))
        .route("/api/interests", get(interests::list).post(interests::create))
        .route("/api/analytics", get(analytics::overview))
        .route("/api/calendar/month", get(calendar::month_view))
        .route("/api/calendar/week", get(calendar::week_view))
        .route("/api/calendar/day", get(calendar::day_view))
        .route("/api/media/upload-url", post(media::create_upload_url))
}
