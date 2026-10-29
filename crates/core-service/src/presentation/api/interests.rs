use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::domain::{InterestTopic, NewInterestSnapshot};
use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct InterestsQuery {
    pub limit: Option<i64>,
}

#[utoipa::path(
    get, path = "/api/interests",
    params(InterestsQuery),
    responses((status = 200, description = "Последние снимки интересов", body = [InterestSnapshotResponse])),
    tag = "interests"
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<InterestsQuery>,
) -> AppResult<Json<Vec<InterestSnapshotResponse>>> {
    let snaps = state.interest_service.recent(q.limit.unwrap_or(20)).await.map_err(AppError::from)?;
    Ok(Json(snaps.into_iter().map(InterestSnapshotResponse::from).collect()))
}

#[utoipa::path(
    post, path = "/api/interests",
    request_body = CreateInterestSnapshotRequest,
    responses((status = 201, description = "Снимок интересов сохранён", body = InterestSnapshotResponse)),
    tag = "interests"
)]
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateInterestSnapshotRequest>,
) -> AppResult<(StatusCode, Json<InterestSnapshotResponse>)> {
    let snap = state
        .interest_service
        .record(NewInterestSnapshot {
            source: body.source.unwrap_or_default(),
            summary: body.summary,
            topics: body
                .topics
                .into_iter()
                .map(|t| InterestTopic { name: t.name, weight: t.weight, note: t.note })
                .collect(),
            period_from: body.period_from,
            period_to: body.period_to,
        })
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(snap.into())))
}
