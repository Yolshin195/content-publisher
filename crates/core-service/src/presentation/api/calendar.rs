use axum::extract::{Query, State};
use axum::Json;

use crate::application::DaySummary;
use crate::presentation::dto::{DayArticleResponse, DayQuery, DaySummaryResponse, MonthQuery, WeekQuery};
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[utoipa::path(
    get, path = "/api/calendar/month",
    params(MonthQuery),
    responses((status = 200, description = "Статьи, сгруппированные по дням месяца", body = [DaySummaryResponse])),
    tag = "calendar"
)]
pub async fn month_view(
    State(state): State<AppState>,
    Query(q): Query<MonthQuery>,
) -> AppResult<Json<Vec<DaySummaryResponse>>> {
    let days = state.calendar_service.month_view(q.year, q.month).await.map_err(AppError::from)?;
    Ok(Json(to_response(days)))
}

#[utoipa::path(
    get, path = "/api/calendar/week",
    params(WeekQuery),
    responses((status = 200, description = "Статьи, сгруппированные по дням недели", body = [DaySummaryResponse])),
    tag = "calendar"
)]
pub async fn week_view(
    State(state): State<AppState>,
    Query(q): Query<WeekQuery>,
) -> AppResult<Json<Vec<DaySummaryResponse>>> {
    let days = state.calendar_service.week_view(q.year, q.week).await.map_err(AppError::from)?;
    Ok(Json(to_response(days)))
}

#[utoipa::path(
    get, path = "/api/calendar/day",
    params(DayQuery),
    responses((status = 200, description = "Статьи на конкретный день", body = DaySummaryResponse)),
    tag = "calendar"
)]
pub async fn day_view(State(state): State<AppState>, Query(q): Query<DayQuery>) -> AppResult<Json<DaySummaryResponse>> {
    let day = state.calendar_service.day_view(q.date).await.map_err(AppError::from)?;
    Ok(Json(to_response(vec![day]).remove(0)))
}

fn to_response(days: Vec<DaySummary>) -> Vec<DaySummaryResponse> {
    days.into_iter()
        .map(|d| DaySummaryResponse {
            date: d.date,
            count: d.count,
            articles: d
                .articles
                .into_iter()
                .map(|a| DayArticleResponse { id: a.id, title: a.title, state: a.state, scheduled_at: a.scheduled_at })
                .collect(),
        })
        .collect()
}
