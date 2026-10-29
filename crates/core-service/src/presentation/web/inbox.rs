use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;

use super::format::{plural_ru, relative, PostView};
use super::shell::{render, Shell};
use crate::application::ports::ArticleFilter;
use crate::state::AppState;

pub struct TopicView {
    pub name: String,
    pub weight: i32,
    pub note: String,
    /// Ширина полосы в процентах (вес 10 = 100%).
    pub pct: i32,
}

pub struct InterestView {
    pub summary: String,
    pub updated: String,
    pub topics: Vec<TopicView>,
}

pub struct TrendView {
    pub name: String,
    pub mentions: usize,
    pub pct: i32,
    pub tip: String,
}

#[derive(Template)]
#[template(path = "inbox.html")]
struct InboxTemplate {
    shell: Shell,
    suggestions: Vec<PostView>,
    total: i64,
    total_label: String,
    upcoming: Vec<PostView>,
    rejected: Vec<PostView>,
    interests: Option<InterestView>,
    trends: Vec<TrendView>,
    tags: Vec<(String, usize)>,
    tag: String,
}

#[derive(Deserialize)]
pub struct InboxQuery {
    tag: Option<String>,
}

pub async fn inbox_page(State(state): State<AppState>, Query(q): Query<InboxQuery>) -> impl IntoResponse {
    let tz = state.config.timezone;
    let svc = &state.article_service;
    let tag = q.tag.clone().unwrap_or_default();

    let (all_suggested, total) = svc
        .list(ArticleFilter { state: Some("suggested".into()), sort: Some("created".into()), page: 1, per_page: 100, ..Default::default() })
        .await
        .unwrap_or_default();
    let mut tag_counts: Vec<(String, usize)> = Vec::new();
    for a in &all_suggested {
        for t in &a.tags {
            match tag_counts.iter_mut().find(|(k, _)| k == t) {
                Some((_, c)) => *c += 1,
                None => tag_counts.push((t.clone(), 1)),
            }
        }
    }
    tag_counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    tag_counts.truncate(12);

    let suggestions = all_suggested
        .iter()
        .filter(|a| tag.is_empty() || a.tags.contains(&tag))
        .map(|a| PostView::new(a, tz, 360))
        .collect();

    let (upcoming, _) = svc
        .list(ArticleFilter {
            state: Some("scheduled".into()),
            from: Some(chrono::Utc::now()),
            sort: Some("scheduled".into()),
            page: 1,
            per_page: 6,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
    let (rejected, _) = svc
        .list(ArticleFilter { state: Some("rejected".into()), sort: Some("updated".into()), page: 1, per_page: 5, ..Default::default() })
        .await
        .unwrap_or_default();

    let (latest, trends) = state.interest_service.overview(8).await.unwrap_or((None, Vec::new()));
    let interests = latest.map(|s| InterestView {
        summary: s.summary,
        updated: relative(s.created_at, tz),
        topics: s
            .topics
            .into_iter()
            .take(12)
            .map(|t| TopicView { pct: t.weight.clamp(1, 10) * 10, name: t.name, weight: t.weight, note: t.note.unwrap_or_default() })
            .collect(),
    });
    let max_trend = trends.iter().map(|t| t.total_weight).max().unwrap_or(1).max(1);
    let trends = trends
        .into_iter()
        .take(8)
        .map(|t| TrendView {
            pct: (t.total_weight * 100 / max_trend).max(4),
            tip: format!("{}: суммарный вес {}, упоминаний {}", t.name, t.total_weight, t.mentions),
            name: t.name,
            mentions: t.mentions,
        })
        .collect();

    render(&InboxTemplate {
        shell: Shell::load(&state, "inbox").await,
        suggestions,
        total,
        total_label: plural_ru(total, "пост", "поста", "постов"),
        upcoming: upcoming.iter().map(|a| PostView::new(a, tz, 120)).collect(),
        rejected: rejected.iter().map(|a| PostView::new(a, tz, 120)).collect(),
        interests,
        trends,
        tags: tag_counts,
        tag,
    })
}
