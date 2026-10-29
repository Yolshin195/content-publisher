use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::application::ports::AnalyticsOverview;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct AnalyticsQuery {
    /// Сколько последних недель показать в динамике (по умолчанию 12).
    pub weeks: Option<i32>,
}

#[utoipa::path(
    get, path = "/api/analytics",
    params(AnalyticsQuery),
    responses((status = 200, description = "Сводка: статусы, публикации по неделям, эффективность промптов, теги", body = AnalyticsOverview)),
    tag = "analytics"
)]
pub async fn overview(
    State(state): State<AppState>,
    Query(q): Query<AnalyticsQuery>,
) -> AppResult<Json<AnalyticsOverview>> {
    let data = state.analytics_service.overview(q.weeks.unwrap_or(12)).await.map_err(AppError::from)?;
    Ok(Json(data))
}
