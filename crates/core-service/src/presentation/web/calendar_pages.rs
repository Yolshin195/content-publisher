use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect};
use askama::Template;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::collections::HashMap;

use crate::application::DaySummary;
use crate::state::AppState;

pub struct DayArticleView {
    pub id: String,
    pub title: String,
    pub state: String,
}

pub struct DayCell {
    pub day: u32,
    pub date: String,
    pub in_month: bool,
    pub articles: Vec<DayArticleView>,
}

#[derive(Template)]
#[template(path = "calendar_month.html")]
struct CalendarMonthTemplate {
    year: i32,
    month: u32,
    prev_year: i32,
    prev_month: u32,
    next_year: i32,
    next_month: u32,
    weeks: Vec<Vec<DayCell>>,
}

#[derive(Template)]
#[template(path = "calendar_week.html")]
struct CalendarWeekTemplate {
    year: i32,
    week: u32,
    prev_year: i32,
    prev_week: u32,
    next_year: i32,
    next_week: u32,
    days: Vec<DayCell>,
}

pub async fn index() -> Redirect {
    let now = Utc::now();
    Redirect::to(&format!("/calendar/month/{}/{}", now.year(), now.month()))
}

fn build_month_grid(year: i32, month: u32, days: &[DaySummary]) -> Vec<Vec<DayCell>> {
    let day_map: HashMap<NaiveDate, &DaySummary> = days.iter().map(|d| (d.date, d)).collect();

    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let weekday_from_monday = first_of_month.weekday().num_days_from_monday();
    let grid_start = first_of_month - Duration::days(weekday_from_monday as i64);

    let mut weeks = Vec::new();
    let mut cursor = grid_start;
    for _ in 0..6 {
        let mut week = Vec::new();
        for _ in 0..7 {
            let articles = day_map
                .get(&cursor)
                .map(|s| {
                    s.articles
                        .iter()
                        .map(|a| DayArticleView { id: a.id.to_string(), title: a.title.clone(), state: a.state.clone() })
                        .collect()
                })
                .unwrap_or_default();

            week.push(DayCell {
                day: cursor.day(),
                date: cursor.format("%Y-%m-%d").to_string(),
                in_month: cursor.month() == month,
                articles,
            });
            cursor += Duration::days(1);
        }
        weeks.push(week);
    }
    weeks
}

pub async fn month_page(State(state): State<AppState>, Path((year, month)): Path<(i32, u32)>) -> impl IntoResponse {
    let days = match state.calendar_service.month_view(year, month).await {
        Ok(d) => d,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };

    let weeks = build_month_grid(year, month, &days);
    let (prev_year, prev_month) = if month == 1 { (year - 1, 12) } else { (year, month - 1) };
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };

    let tpl = CalendarMonthTemplate { year, month, prev_year, prev_month, next_year, next_month, weeks };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

pub async fn week_page(State(state): State<AppState>, Path((year, week)): Path<(i32, u32)>) -> impl IntoResponse {
    let summaries = match state.calendar_service.week_view(year, week).await {
        Ok(d) => d,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };

    let days = summaries
        .into_iter()
        .map(|s| DayCell {
            day: s.date.day(),
            date: s.date.format("%Y-%m-%d").to_string(),
            in_month: true,
            articles: s
                .articles
                .into_iter()
                .map(|a| DayArticleView { id: a.id.to_string(), title: a.title, state: a.state })
                .collect(),
        })
        .collect();

    let (prev_year, prev_week) = if week <= 1 { (year - 1, 52) } else { (year, week - 1) };
    let (next_year, next_week) = if week >= 52 { (year + 1, 1) } else { (year, week + 1) };

    let tpl = CalendarWeekTemplate { year, week, prev_year, prev_week, next_year, next_week, days };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}
