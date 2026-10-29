use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect};
use serde::Deserialize;
use uuid::Uuid;

use super::format::{plural_ru, PostView};
use super::shell::{render, Shell};
use crate::application::ports::ArticleFilter;
use crate::state::AppState;

const PER_PAGE: i64 = 25;

#[derive(Deserialize, Default)]
pub struct PostsQuery {
    q: Option<String>,
    state: Option<String>,
    origin: Option<String>,
    tag: Option<String>,
    sort: Option<String>,
    page: Option<i64>,
}

#[derive(Template)]
#[template(path = "posts.html")]
struct PostsTemplate {
    shell: Shell,
    rows: Vec<PostView>,
    total_label: String,
    page: i64,
    pages: i64,
    q: String,
    state: String,
    origin: String,
    tag: String,
    sort: String,
    tags: Vec<(String, i64)>,
    prev_url: Option<String>,
    next_url: Option<String>,
}

fn page_url(q: &PostsQuery, page: i64) -> String {
    let mut parts = Vec::new();
    for (k, v) in [("q", &q.q), ("state", &q.state), ("origin", &q.origin), ("tag", &q.tag), ("sort", &q.sort)] {
        if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
            parts.push(format!("{k}={}", urlencode(v)));
        }
    }
    parts.push(format!("page={page}"));
    format!("/posts?{}", parts.join("&"))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub async fn posts_page(State(state): State<AppState>, Query(q): Query<PostsQuery>) -> impl IntoResponse {
    let page = q.page.unwrap_or(1).max(1);
    let non_empty = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
    let filter = ArticleFilter {
        state: non_empty(&q.state),
        origin: non_empty(&q.origin),
        tag: non_empty(&q.tag),
        q: non_empty(&q.q),
        sort: non_empty(&q.sort).or(Some("updated".into())),
        page,
        per_page: PER_PAGE,
        ..Default::default()
    };
    let (articles, total) = state.article_service.list(filter).await.unwrap_or_default();
    let pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let tz = state.config.timezone;

    render(&PostsTemplate {
        shell: Shell::load(&state, "posts").await,
        rows: articles.iter().map(|a| PostView::new(a, tz, 140)).collect(),
        total_label: plural_ru(total, "пост", "поста", "постов"),
        page,
        pages,
        q: q.q.clone().unwrap_or_default(),
        state: q.state.clone().unwrap_or_default(),
        origin: q.origin.clone().unwrap_or_default(),
        tag: q.tag.clone().unwrap_or_default(),
        sort: q.sort.clone().unwrap_or_else(|| "updated".into()),
        tags: state.article_service.tag_counts().await.unwrap_or_default(),
        prev_url: (page > 1).then(|| page_url(&q, page - 1)),
        next_url: (page < pages).then(|| page_url(&q, page + 1)),
    })
}

#[derive(Template)]
#[template(path = "article.html")]
struct ArticlePageTemplate {
    shell: Shell,
    article_id: String,
    date: String,
}

#[derive(Deserialize)]
pub struct NewArticleQuery {
    date: Option<String>,
}

pub async fn new_article_page(State(state): State<AppState>, Query(q): Query<NewArticleQuery>) -> impl IntoResponse {
    let date = q.date.filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()).unwrap_or_default();
    render(&ArticlePageTemplate { shell: Shell::load(&state, "posts").await, article_id: String::new(), date })
}

pub async fn article_page(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    render(&ArticlePageTemplate { shell: Shell::load(&state, "posts").await, article_id: id.to_string(), date: String::new() })
}

pub async fn edit_redirect(Path(id): Path<Uuid>) -> Redirect {
    Redirect::permanent(&format!("/articles/{id}"))
}
