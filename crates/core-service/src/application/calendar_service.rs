use std::sync::Arc;

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

use super::ports::ArticleRepository;
use crate::domain::{Article, DomainError};

pub struct DaySummary {
    pub date: NaiveDate,
    pub count: usize,
    /// Статьи дня, отсортированные по времени публикации.
    pub articles: Vec<Article>,
}

/// Строит данные для календарного представления (месяц/неделя/день) поверх
/// статей со `scheduled_at`. Границы дней считаются в часовом поясе автора
/// (`APP_TIMEZONE`), иначе пост на 6 утра по местному времени мог бы оказаться
/// в предыдущем дне.
pub struct CalendarService {
    articles: Arc<dyn ArticleRepository>,
    tz: Tz,
}

impl CalendarService {
    pub fn new(articles: Arc<dyn ArticleRepository>, tz: Tz) -> Self {
        Self { articles, tz }
    }

    pub fn tz(&self) -> Tz {
        self.tz
    }

    /// Полночь локальной даты в UTC.
    pub fn day_start_utc(&self, date: NaiveDate) -> DateTime<Utc> {
        let naive = date.and_hms_opt(0, 0, 0).expect("midnight is valid");
        self.tz
            .from_local_datetime(&naive)
            .earliest()
            .unwrap_or_else(|| self.tz.from_utc_datetime(&naive))
            .with_timezone(&Utc)
    }

    pub fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.tz).date_naive()
    }

    pub async fn range_view(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<DaySummary>, DomainError> {
        let articles = self.articles.list_by_date_range(self.day_start_utc(start), self.day_start_utc(end)).await?;
        Ok(self.group_by_day(articles, start, end))
    }

    pub async fn month_view(&self, year: i32, month: u32) -> Result<Vec<DaySummary>, DomainError> {
        let (start, end) = month_bounds(year, month)?;
        self.range_view(start, end).await
    }

    pub async fn week_view(&self, year: i32, week: u32) -> Result<Vec<DaySummary>, DomainError> {
        let start = iso_week_start(year, week)?;
        self.range_view(start, start + Duration::days(7)).await
    }

    pub async fn day_view(&self, date: NaiveDate) -> Result<DaySummary, DomainError> {
        let mut days = self.range_view(date, date + Duration::days(1)).await?;
        Ok(days.remove(0))
    }

    fn group_by_day(&self, articles: Vec<Article>, start: NaiveDate, end: NaiveDate) -> Vec<DaySummary> {
        let mut days: Vec<DaySummary> = Vec::new();
        let mut cur = start;
        while cur < end {
            days.push(DaySummary { date: cur, count: 0, articles: Vec::new() });
            cur += Duration::days(1);
        }
        for article in articles {
            let Some(at) = article.scheduled_at else { continue };
            let local = at.with_timezone(&self.tz).date_naive();
            if let Some(day) = days.iter_mut().find(|d| d.date == local) {
                day.articles.push(article);
            }
        }
        for day in days.iter_mut() {
            day.articles.sort_by_key(|a| a.scheduled_at);
            day.count = day.articles.len();
        }
        days
    }
}

pub fn month_bounds(year: i32, month: u32) -> Result<(NaiveDate, NaiveDate), DomainError> {
    let start =
        NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| DomainError::Validation("invalid year/month".into()))?;
    let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }
        .ok_or_else(|| DomainError::Validation("invalid year/month".into()))?;
    Ok((start, next))
}

pub fn iso_week_start(year: i32, week: u32) -> Result<NaiveDate, DomainError> {
    NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
        .ok_or_else(|| DomainError::Validation("invalid year/week".into()))
}
