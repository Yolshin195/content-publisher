use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use uuid::Uuid;

use super::ports::ArticleRepository;
use crate::domain::{Article, DomainError};

pub struct DayArticleSummary {
    pub id: Uuid,
    pub title: String,
    pub state: String,
    pub scheduled_at: Option<DateTime<Utc>>,
}

pub struct DaySummary {
    pub date: NaiveDate,
    pub count: usize,
    pub articles: Vec<DayArticleSummary>,
}

/// Строит данные для календарного представления (месяц/неделя/день) поверх
/// статей со `scheduled_at`. UI-навигация (сетка 6x7 в стиле Google Calendar)
/// реализуется в presentation/web, здесь — только агрегация данных.
pub struct CalendarService {
    articles: Arc<dyn ArticleRepository>,
}

impl CalendarService {
    pub fn new(articles: Arc<dyn ArticleRepository>) -> Self {
        Self { articles }
    }

    pub async fn month_view(&self, year: i32, month: u32) -> Result<Vec<DaySummary>, DomainError> {
        let start = NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or_else(|| DomainError::Validation("invalid year/month".into()))?;
        let next_month = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        }
        .ok_or_else(|| DomainError::Validation("invalid year/month".into()))?;

        let from = Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap());
        let to = Utc.from_utc_datetime(&next_month.and_hms_opt(0, 0, 0).unwrap());

        let articles = self.articles.list_by_date_range(from, to).await?;
        Ok(group_by_day(&articles, start, next_month))
    }

    pub async fn week_view(&self, year: i32, week: u32) -> Result<Vec<DaySummary>, DomainError> {
        let jan4 =
            NaiveDate::from_ymd_opt(year, 1, 4).ok_or_else(|| DomainError::Validation("invalid year".into()))?;
        let week1_monday = jan4 - Duration::days(jan4.weekday().num_days_from_monday() as i64);
        let start = week1_monday + Duration::weeks(week as i64 - 1);
        let end = start + Duration::days(7);

        let from = Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap());
        let to = Utc.from_utc_datetime(&end.and_hms_opt(0, 0, 0).unwrap());

        let articles = self.articles.list_by_date_range(from, to).await?;
        Ok(group_by_day(&articles, start, end))
    }

    pub async fn day_view(&self, date: NaiveDate) -> Result<DaySummary, DomainError> {
        let from = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap());
        let to = from + Duration::days(1);
        let articles = self.articles.list_by_date_range(from, to).await?;
        let day_articles = to_day_articles(&articles, date);
        Ok(DaySummary { date, count: day_articles.len(), articles: day_articles })
    }
}

fn to_day_articles(articles: &[Article], day: NaiveDate) -> Vec<DayArticleSummary> {
    articles
        .iter()
        .filter(|a| a.scheduled_at.map(|d| d.date_naive() == day).unwrap_or(false))
        .map(|a| DayArticleSummary {
            id: a.id,
            title: a.title.clone(),
            state: a.state.as_str().to_string(),
            scheduled_at: a.scheduled_at,
        })
        .collect()
}

fn group_by_day(articles: &[Article], start: NaiveDate, end: NaiveDate) -> Vec<DaySummary> {
    let mut days = Vec::new();
    let mut cur = start;
    while cur < end {
        let day_articles = to_day_articles(articles, cur);
        days.push(DaySummary { date: cur, count: day_articles.len(), articles: day_articles });
        cur += Duration::days(1);
    }
    days
}
