use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use chrono::Datelike;
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;

pub struct TargetOption {
    pub id: String,
    pub name: String,
}

pub struct VideoLinkOption {
    pub platform: String,
    pub url: String,
}

pub struct TaskRow {
    pub target_name: String,
    pub status: String,
}

#[derive(Deserialize)]
pub struct NewArticleQuery {
    pub date: Option<String>,
}

#[derive(Template)]
#[template(path = "article_form.html")]
struct ArticleFormTemplate {
    is_edit: bool,
    article_id: String,
    title: String,
    content_html: String,
    excerpt: String,
    default_date: String,
    targets: Vec<TargetOption>,
    video_links: Vec<VideoLinkOption>,
    month: u32,
    year: i32,
}

pub async fn new_article_page(State(state): State<AppState>, Query(q): Query<NewArticleQuery>) -> impl IntoResponse {
    let targets = state
        .target_service
        .list()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| TargetOption { id: t.id.to_string(), name: t.display_name })
        .collect();

    let now = Utc::now();
    let tpl = ArticleFormTemplate {
        is_edit: false,
        article_id: String::new(),
        title: String::new(),
        content_html: String::new(),
        excerpt: String::new(),
        default_date: q.date.unwrap_or_default(),
        targets,
        video_links: Vec::new(),
        month: now.month(),
        year: now.year(),
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

pub async fn edit_article_page(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let article = match state.article_service.get(id).await {
        Ok(a) => a,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };
    let links = state.article_service.list_video_links(id).await.unwrap_or_default();
    let targets = state
        .target_service
        .list()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| TargetOption { id: t.id.to_string(), name: t.display_name })
        .collect();

    let now = Utc::now();
    let tpl = ArticleFormTemplate {
        is_edit: true,
        article_id: article.id.to_string(),
        title: article.title,
        content_html: article.content_html,
        excerpt: article.excerpt.unwrap_or_default(),
        default_date: article.scheduled_at.map(|d| d.format("%Y-%m-%dT%H:%M").to_string()).unwrap_or_default(),
        targets,
        video_links: links
            .into_iter()
            .map(|v| VideoLinkOption { platform: v.platform.as_str().to_string(), url: v.url })
            .collect(),
        month: now.month(),
        year: now.year(),
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

#[derive(Template)]
#[template(path = "article_view.html")]
struct ArticleViewTemplate {
    article_id: String,
    title: String,
    content_html: String,
    state: String,
    tasks: Vec<TaskRow>,
    month: u32,
    year: i32,
}

pub async fn view_article_page(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let article = match state.article_service.get(id).await {
        Ok(a) => a,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };
    let tasks = state.scheduling_service.list_tasks(id).await.unwrap_or_default();

    let mut task_rows = Vec::new();
    for t in tasks {
        let target_name = state
            .target_service
            .get(t.target_id)
            .await
            .map(|tg| tg.display_name)
            .unwrap_or_else(|_| t.target_id.to_string());
        task_rows.push(TaskRow { target_name, status: t.status.as_str().to_string() });
    }

    let now = Utc::now();
    let tpl = ArticleViewTemplate {
        article_id: article.id.to_string(),
        title: article.title,
        content_html: article.content_html,
        state: article.state.as_str().to_string(),
        tasks: task_rows,
        month: now.month(),
        year: now.year(),
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}
