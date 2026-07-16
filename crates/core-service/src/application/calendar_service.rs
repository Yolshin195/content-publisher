use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use uuid::Uuid;

use super::ports::{ArticleRepository, TargetRepository, TaskRepository};
use crate::domain::{Article, DomainError};

pub struct DayArticleSummary {
    pub id: Uuid,
    pub title: String,
    pub state: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub target_platforms: Vec<String>,
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
    tasks: Arc<dyn TaskRepository>,
    targets: Arc<dyn TargetRepository>,
}

impl CalendarService {
    pub fn new(articles: Arc<dyn ArticleRepository>, tasks: Arc<dyn TaskRepository>, targets: Arc<dyn TargetRepository>) -> Self {
        Self { articles, tasks, targets }
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
        
        // Fetch all targets once for platform lookup
        let targets_map: std::collections::HashMap<Uuid, String> = self.targets.list().await?
            .into_iter()
            .map(|t| (t.id, t.platform.as_str().to_string()))
            .collect();
        
        let mut days = group_by_day(&articles, start, next_month);
        
        // Enrich with target platform info from publication tasks
        for day in &mut days {
            for article in &mut day.articles {
                if let Ok(tasks) = self.tasks.list_by_article(article.id).await {
                    article.target_platforms = tasks
                        .iter()
                        .filter_map(|task| targets_map.get(&task.target_id).cloned())
                        .collect();
                }
            }
        }
        
        Ok(days)
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
        
        // Fetch all targets once for platform lookup
        let targets_map: std::collections::HashMap<Uuid, String> = self.targets.list().await?
            .into_iter()
            .map(|t| (t.id, t.platform.as_str().to_string()))
            .collect();
        
        let mut days = group_by_day(&articles, start, end);
        
        // Enrich with target platform info from publication tasks
        for day in &mut days {
            for article in &mut day.articles {
                if let Ok(tasks) = self.tasks.list_by_article(article.id).await {
                    article.target_platforms = tasks
                        .iter()
                        .filter_map(|task| targets_map.get(&task.target_id).cloned())
                        .collect();
                }
            }
        }
        
        Ok(days)
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
            target_platforms: vec![], // placeholder, will be filled by caller with task data
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
