use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::domain::{NewPublicationTarget, Platform};
use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[utoipa::path(
    get, path = "/api/targets",
    responses((status = 200, description = "Список целевых площадок публикации", body = [TargetResponse])),
    tag = "targets"
)]
pub async fn list_targets(State(state): State<AppState>) -> AppResult<Json<Vec<TargetResponse>>> {
    let targets = state.target_service.list().await.map_err(AppError::from)?;
    Ok(Json(targets.into_iter().map(TargetResponse::from).collect()))
}

#[utoipa::path(
    post, path = "/api/targets",
    request_body = CreateTargetRequest,
    responses((status = 201, description = "Целевая площадка создана", body = TargetResponse)),
    tag = "targets"
)]
pub async fn create_target(
    State(state): State<AppState>,
    Json(body): Json<CreateTargetRequest>,
) -> AppResult<(StatusCode, Json<TargetResponse>)> {
    let platform = Platform::parse(&body.platform).map_err(AppError::from)?;
    let target = state
        .target_service
        .create(NewPublicationTarget {
            platform,
            external_id: body.external_id,
            display_name: body.display_name,
            config: body.config,
        })
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(TargetResponse::from(target))))
}

#[utoipa::path(
    get, path = "/api/targets/{id}",
    params(("id" = Uuid, Path, description = "ID целевой площадки")),
    responses((status = 200, description = "Целевая площадка", body = TargetResponse)),
    tag = "targets"
)]
pub async fn get_target(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<TargetResponse>> {
    let target = state.target_service.get(id).await.map_err(AppError::from)?;
    Ok(Json(TargetResponse::from(target)))
}

#[utoipa::path(
    put, path = "/api/targets/{id}",
    params(("id" = Uuid, Path, description = "ID целевой площадки")),
    request_body = UpdateTargetRequest,
    responses((status = 200, description = "Целевая площадка обновлена", body = TargetResponse)),
    tag = "targets"
)]
pub async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTargetRequest>,
) -> AppResult<Json<TargetResponse>> {
    let target =
        state.target_service.update(id, body.display_name, body.is_active, body.config).await.map_err(AppError::from)?;
    Ok(Json(TargetResponse::from(target)))
}

#[utoipa::path(
    delete, path = "/api/targets/{id}",
    params(("id" = Uuid, Path, description = "ID целевой площадки")),
    responses((status = 204, description = "Целевая площадка удалена")),
    tag = "targets"
)]
pub async fn delete_target(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    state.target_service.delete(id).await.map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
