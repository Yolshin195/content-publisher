use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect};
use askama::Template;
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use std::collections::HashMap;

use crate::application::DaySummary;
use crate::state::AppState;

pub struct DayArticleView {
    pub id: String,
    pub title: String,
    pub state: String,
    pub platform: Option<String>,
    pub scheduled_at: Option<String>,
}

pub struct DayCell {
    pub day: u32,
    pub date: String,
    pub in_month: bool,
    pub is_today: bool,
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

pub struct ListArticleView {
    pub id: String,
    pub title: String,
    pub state: String,
    pub time: String,
}

pub struct ListDayGroup {
    pub day: u32,
    pub month_caption: String,
    pub weekday: String,
    pub is_today: bool,
    pub articles: Vec<ListArticleView>,
}

#[derive(Template)]
#[template(path = "calendar_list.html")]
struct CalendarListTemplate {
    title: String,
    prev_url: String,
    next_url: String,
    grid_url: String,
    list_month_url: String,
    list_week_url: String,
    granularity: String,
    groups: Vec<ListDayGroup>,
}

pub async fn index() -> Redirect {
    let now = Utc::now();
    Redirect::to(&format!("/calendar/month/{}/{}", now.year(), now.month()))
}

pub async fn list_index() -> Redirect {
    let now = Utc::now();
    Redirect::to(&format!("/calendar/list/month/{}/{}", now.year(), now.month()))
}

fn build_month_grid(year: i32, month: u32, days: &[DaySummary]) -> Vec<Vec<DayCell>> {
    let day_map: HashMap<NaiveDate, &DaySummary> = days.iter().map(|d| (d.date, d)).collect();

    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let weekday_from_monday = first_of_month.weekday().num_days_from_monday();
    let grid_start = first_of_month - Duration::days(weekday_from_monday as i64);
    
    let today = Utc::now().date_naive();

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
                        .map(|a| DayArticleView { 
                            id: a.id.to_string(), 
                            title: a.title.clone(), 
                            state: a.state.clone(),
                            platform: a.target_platforms.first().cloned().map(|p| p.to_lowercase()),
                            scheduled_at: a.scheduled_at.map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default();

            week.push(DayCell {
                day: cursor.day(),
                date: cursor.format("%Y-%m-%d").to_string(),
                in_month: cursor.month() == month,
                is_today: cursor == today,
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

fn weekday_short_ru(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "пн",
        Weekday::Tue => "вт",
        Weekday::Wed => "ср",
        Weekday::Thu => "чт",
        Weekday::Fri => "пт",
        Weekday::Sat => "сб",
        Weekday::Sun => "вс",
    }
}

fn month_genitive_ru(month: u32) -> &'static str {
    match month {
        1 => "января",
        2 => "февраля",
        3 => "марта",
        4 => "апреля",
        5 => "мая",
        6 => "июня",
        7 => "июля",
        8 => "августа",
        9 => "сентября",
        10 => "октября",
        11 => "ноября",
        12 => "декабря",
        _ => "",
    }
}

fn month_nominative_ru(month: u32) -> &'static str {
    match month {
        1 => "Январь",
        2 => "Февраль",
        3 => "Март",
        4 => "Апрель",
        5 => "Май",
        6 => "Июнь",
        7 => "Июль",
        8 => "Август",
        9 => "Сентябрь",
        10 => "Октябрь",
        11 => "Ноябрь",
        12 => "Декабрь",
        _ => "",
    }
}

fn build_list_group(summary: DaySummary, today: NaiveDate) -> ListDayGroup {
    let mut articles = summary.articles;
    articles.sort_by_key(|a| a.scheduled_at);

    let items = articles
        .into_iter()
        .map(|a| ListArticleView {
            id: a.id.to_string(),
            title: a.title,
            state: a.state,
            time: a
                .scheduled_at
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_else(|| "—".to_string()),
        })
        .collect();

    ListDayGroup {
        day: summary.date.day(),
        month_caption: month_genitive_ru(summary.date.month()).to_string(),
        weekday: weekday_short_ru(summary.date.weekday()).to_string(),
        is_today: summary.date == today,
        articles: items,
    }
}

/// Agenda-представление (список статей по дням, аналог Google Calendar "Schedule"),
/// разбитое по месяцу — показывает только дни, где есть запланированные статьи.
pub async fn list_month_page(State(state): State<AppState>, Path((year, month)): Path<(i32, u32)>) -> impl IntoResponse {
    let days = match state.calendar_service.month_view(year, month).await {
        Ok(d) => d,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };

    let today = Utc::now().date_naive();
    let groups: Vec<ListDayGroup> =
        days.into_iter().filter(|d| d.count > 0).map(|d| build_list_group(d, today)).collect();

    let (prev_year, prev_month) = if month == 1 { (year - 1, 12) } else { (year, month - 1) };
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };

    let first_of_month = match NaiveDate::from_ymd_opt(year, month, 1) {
        Some(d) => d,
        None => return Html("<h1>Ошибка</h1><p>invalid year/month</p>".to_string()),
    };
    let iso = first_of_month.iso_week();

    let tpl = CalendarListTemplate {
        title: format!("{} {}", month_nominative_ru(month), year),
        prev_url: format!("/calendar/list/month/{prev_year}/{prev_month}"),
        next_url: format!("/calendar/list/month/{next_year}/{next_month}"),
        grid_url: format!("/calendar/month/{year}/{month}"),
        list_month_url: format!("/calendar/list/month/{year}/{month}"),
        list_week_url: format!("/calendar/list/week/{}/{}", iso.year(), iso.week()),
        granularity: "month".to_string(),
        groups,
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

/// Agenda-представление, разбитое по неделе.
pub async fn list_week_page(State(state): State<AppState>, Path((year, week)): Path<(i32, u32)>) -> impl IntoResponse {
    let days = match state.calendar_service.week_view(year, week).await {
        Ok(d) => d,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };

    let today = Utc::now().date_naive();
    let groups: Vec<ListDayGroup> =
        days.into_iter().filter(|d| d.count > 0).map(|d| build_list_group(d, today)).collect();

    let (prev_year, prev_week) = if week <= 1 { (year - 1, 52) } else { (year, week - 1) };
    let (next_year, next_week) = if week >= 52 { (year + 1, 1) } else { (year, week + 1) };

    let jan4 = match NaiveDate::from_ymd_opt(year, 1, 4) {
        Some(d) => d,
        None => return Html("<h1>Ошибка</h1><p>invalid year</p>".to_string()),
    };
    let week1_monday = jan4 - Duration::days(jan4.weekday().num_days_from_monday() as i64);
    let week_start = week1_monday + Duration::weeks(week as i64 - 1);

    let tpl = CalendarListTemplate {
        title: format!("Неделя {week}, {year}"),
        prev_url: format!("/calendar/list/week/{prev_year}/{prev_week}"),
        next_url: format!("/calendar/list/week/{next_year}/{next_week}"),
        grid_url: format!("/calendar/week/{year}/{week}"),
        list_month_url: format!("/calendar/list/month/{}/{}", week_start.year(), week_start.month()),
        list_week_url: format!("/calendar/list/week/{year}/{week}"),
        granularity: "week".to_string(),
        groups,
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}
