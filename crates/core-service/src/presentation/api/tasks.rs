use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[utoipa::path(
    post, path = "/api/articles/{id}/schedule",
    params(("id" = Uuid, Path, description = "ID статьи")),
    request_body = ScheduleArticleRequest,
    responses((status = 201, description = "Задачи публикации созданы по одной на каждую цель", body = [TaskResponse])),
    tag = "publication"
)]
pub async fn schedule_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ScheduleArticleRequest>,
) -> AppResult<(StatusCode, Json<Vec<TaskResponse>>)> {
    let tasks =
        state.scheduling_service.schedule(id, body.scheduled_at, body.target_ids).await.map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(tasks.into_iter().map(TaskResponse::from).collect())))
}

#[utoipa::path(
    post, path = "/api/articles/{id}/publish-now",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses((status = 202, description = "Немедленная публикация запущена во все привязанные цели")),
    tag = "publication"
)]
pub async fn publish_now(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    state.orchestrator.publish_article_now(id).await.map_err(AppError::from)?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post, path = "/api/articles/{id}/targets/{target_id}/retry",
    params(
        ("id" = Uuid, Path, description = "ID статьи"),
        ("target_id" = Uuid, Path, description = "ID целевой площадки")
    ),
    responses((status = 202, description = "Повторная публикация в эту цель запланирована")),
    tag = "publication"
)]
pub async fn retry_target(
    State(state): State<AppState>,
    Path((id, target_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    state.scheduling_service.retry_by_target(id, target_id).await.map_err(AppError::from)?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    delete, path = "/api/articles/{id}/targets/{target_id}",
    params(
        ("id" = Uuid, Path, description = "ID статьи"),
        ("target_id" = Uuid, Path, description = "ID целевой площадки")
    ),
    responses((status = 204, description = "Публикация в эту цель отменена (если ещё не выполнена)")),
    tag = "publication"
)]
pub async fn cancel_target(
    State(state): State<AppState>,
    Path((id, target_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    state.scheduling_service.cancel_by_target(id, target_id).await.map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/api/articles/{id}/tasks",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses((status = 200, description = "Статусы публикации по всем целевым площадкам", body = [TaskResponse])),
    tag = "publication"
)]
pub async fn list_tasks(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Vec<TaskResponse>>> {
    let tasks = state.scheduling_service.list_tasks(id).await.map_err(AppError::from)?;
    Ok(Json(tasks.into_iter().map(TaskResponse::from).collect()))
}

#[utoipa::path(
    get, path = "/api/tasks/{task_id}/logs",
    params(("task_id" = Uuid, Path, description = "ID задачи публикации")),
    responses((status = 200, description = "Логи попыток публикации задачи", body = [LogResponse])),
    tag = "publication"
)]
pub async fn task_logs(State(state): State<AppState>, Path(task_id): Path<Uuid>) -> AppResult<Json<Vec<LogResponse>>> {
    let logs = state.log_repo.list_by_task(task_id).await.map_err(AppError::from)?;
    Ok(Json(logs.into_iter().map(LogResponse::from).collect()))
}
