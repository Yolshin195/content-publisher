use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{Datelike, Duration, NaiveDate};

use super::format::{day_caption, month_nominative, month_short, plural_ru, weekday_short, PostView};
use super::shell::{error_page, render, Shell};
use crate::application::calendar_service::{iso_week_start, month_bounds};
use crate::application::ports::ArticleFilter;
use crate::application::DaySummary;
use crate::state::AppState;

#[derive(Clone)]
pub struct DayCell {
    pub date: String,
    pub day: u32,
    /// «ср, 29 окт» — заголовок колонки недели
    pub caption: String,
    /// «29 октября, ср» — заголовок дня в списке
    pub long_caption: String,
    pub in_month: bool,
    pub is_today: bool,
    pub is_past: bool,
    pub posts: Vec<PostView>,
}

#[derive(Template)]
#[template(path = "calendar.html")]
struct CalendarTemplate {
    shell: Shell,
    /// month | week | list
    view: &'static str,
    title: String,
    prev_url: String,
    next_url: String,
    today_url: String,
    month_url: String,
    week_url: String,
    list_url: String,
    weeks: Vec<Vec<DayCell>>,
    days: Vec<DayCell>,
    undated: Vec<PostView>,
    total_label: String,
}

fn cells(state: &AppState, summaries: Vec<DaySummary>, month: Option<u32>) -> Vec<DayCell> {
    let tz = state.config.timezone;
    let today = state.calendar_service.today();
    summaries
        .into_iter()
        .map(|s| DayCell {
            date: s.date.format("%Y-%m-%d").to_string(),
            day: s.date.day(),
            caption: format!("{}, {} {}", weekday_short(s.date.weekday()), s.date.day(), month_short(s.date.month())),
            long_caption: day_caption(s.date),
            in_month: month.map(|m| s.date.month() == m).unwrap_or(true),
            is_today: s.date == today,
            is_past: s.date < today,
            posts: s.articles.iter().map(|a| PostView::new(a, tz, 140)).collect(),
        })
        .collect()
}

async fn undated(state: &AppState) -> Vec<PostView> {
    let (list, _) = state
        .article_service
        .list(ArticleFilter {
            state: Some("draft,suggested".into()),
            has_date: Some(false),
            sort: Some("updated".into()),
            page: 1,
            per_page: 30,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
    list.iter().map(|a| PostView::new(a, state.config.timezone, 100)).collect()
}

fn week_url(prefix: &str, date: NaiveDate) -> String {
    let iso = date.iso_week();
    format!("/calendar/{prefix}week/{}/{}", iso.year(), iso.week())
}

pub async fn index(State(state): State<AppState>) -> Redirect {
    let today = state.calendar_service.today();
    Redirect::to(&format!("/calendar/month/{}/{}", today.year(), today.month()))
}

pub async fn list_index(State(state): State<AppState>) -> Redirect {
    let today = state.calendar_service.today();
    Redirect::to(&format!("/calendar/list/month/{}/{}", today.year(), today.month()))
}

async fn month_like(state: AppState, year: i32, month: u32, list: bool) -> Response {
    let Ok((start, end)) = month_bounds(year, month) else { return error_page("Неверный месяц") };
    let today = state.calendar_service.today();
    let (prev, next) = (start - Duration::days(1), end);
    let prefix = if list { "list/" } else { "" };

    // Сетка месяца: с понедельника недели, где 1-е число, 6 недель.
    let grid_start = start - Duration::days(start.weekday().num_days_from_monday() as i64);
    let grid_end = if list { end } else { grid_start + Duration::days(42) };
    let range_start = if list { start } else { grid_start };
    let summaries = match state.calendar_service.range_view(range_start, grid_end).await {
        Ok(s) => s,
        Err(e) => return error_page(e),
    };
    let total: usize = summaries.iter().filter(|d| d.date >= start && d.date < end).map(|d| d.count).sum();
    let all = cells(&state, summaries, Some(month));

    let (weeks, days) = if list {
        (Vec::new(), all.into_iter().filter(|d| !d.posts.is_empty()).collect())
    } else {
        (all.chunks(7).map(|w| w.to_vec()).collect(), Vec::new())
    };
    let reference = if today >= start && today < end { today } else { start };

    render(&CalendarTemplate {
        shell: Shell::load(&state, "calendar").await,
        view: if list { "list" } else { "month" },
        title: format!("{} {}", month_nominative(month), year),
        prev_url: format!("/calendar/{prefix}month/{}/{}", prev.year(), prev.month()),
        next_url: format!("/calendar/{prefix}month/{}/{}", next.year(), next.month()),
        today_url: format!("/calendar/{prefix}month/{}/{}", today.year(), today.month()),
        month_url: format!("/calendar/month/{year}/{month}"),
        week_url: week_url("", reference),
        list_url: format!("/calendar/list/month/{year}/{month}"),
        weeks,
        days,
        undated: undated(&state).await,
        total_label: plural_ru(total as i64, "пост", "поста", "постов"),
    })
    .into_response()
}

async fn week_like(state: AppState, year: i32, week: u32, list: bool) -> Response {
    let Ok(start) = iso_week_start(year, week) else { return error_page("Неверная неделя") };
    let end = start + Duration::days(7);
    let today = state.calendar_service.today();
    let prefix = if list { "list/" } else { "" };
    let summaries = match state.calendar_service.range_view(start, end).await {
        Ok(s) => s,
        Err(e) => return error_page(e),
    };
    let total: usize = summaries.iter().map(|d| d.count).sum();
    let mut days = cells(&state, summaries, None);
    if list {
        days.retain(|d| !d.posts.is_empty());
    }
    let last = end - Duration::days(1);
    let title = if start.month() == last.month() {
        format!("{}–{} {} {}", start.day(), last.day(), super::format::month_genitive(last.month()), last.year())
    } else {
        format!(
            "{} {} – {} {} {}",
            start.day(),
            month_short(start.month()),
            last.day(),
            month_short(last.month()),
            last.year()
        )
    };

    render(&CalendarTemplate {
        shell: Shell::load(&state, "calendar").await,
        view: if list { "list" } else { "week" },
        title,
        prev_url: week_url(prefix, start - Duration::days(7)),
        next_url: week_url(prefix, end),
        today_url: week_url(prefix, today),
        month_url: format!("/calendar/month/{}/{}", start.year(), start.month()),
        week_url: week_url("", start),
        list_url: week_url("list/", start),
        weeks: Vec::new(),
        days,
        undated: undated(&state).await,
        total_label: plural_ru(total as i64, "пост", "поста", "постов"),
    })
    .into_response()
}

pub async fn month_page(State(state): State<AppState>, Path((year, month)): Path<(i32, u32)>) -> Response {
    month_like(state, year, month, false).await
}

pub async fn week_page(State(state): State<AppState>, Path((year, week)): Path<(i32, u32)>) -> Response {
    week_like(state, year, week, false).await
}

pub async fn list_month_page(State(state): State<AppState>, Path((year, month)): Path<(i32, u32)>) -> Response {
    month_like(state, year, month, true).await
}

pub async fn list_week_page(State(state): State<AppState>, Path((year, week)): Path<(i32, u32)>) -> Response {
    week_like(state, year, week, true).await
}
