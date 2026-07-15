use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};
use uuid::Uuid;

use crate::state::AppState;

pub struct TargetRow {
    pub id: String,
    pub platform: String,
    pub display_name: String,
    pub external_id: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Template)]
#[template(path = "target_list.html")]
struct TargetListTemplate {
    targets: Vec<TargetRow>,
}

pub async fn list_targets_page(State(state): State<AppState>) -> impl IntoResponse {
    let targets = state
        .target_service
        .list()
        .await
        .unwrap_or_default();

    let target_rows: Vec<TargetRow> = targets
        .into_iter()
        .map(|t| TargetRow {
            id: t.id.to_string(),
            platform: t.platform.as_str().to_string(),
            display_name: t.display_name,
            external_id: t.external_id,
            is_active: t.is_active,
            created_at: t.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    let tpl = TargetListTemplate { targets: target_rows };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

#[derive(Template)]
#[template(path = "target_form.html")]
struct TargetFormTemplate {
    is_edit: bool,
    target_id: String,
    platform: String,
    external_id: String,
    display_name: String,
    is_active: bool,
    config_json: String,
}

pub async fn new_target_page() -> impl IntoResponse {
    let tpl = TargetFormTemplate {
        is_edit: false,
        target_id: String::new(),
        platform: String::new(),
        external_id: String::new(),
        display_name: String::new(),
        is_active: true,
        config_json: "{}".to_string(),
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

pub async fn edit_target_page(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let target = match state.target_service.get(id).await {
        Ok(t) => t,
        Err(e) => return Html(format!("<h1>Ошибка</h1><p>{e}</p>")),
    };

    let config_json = serde_json::to_string_pretty(&target.config).unwrap_or_else(|_| "{}".to_string());

    let tpl = TargetFormTemplate {
        is_edit: true,
        target_id: target.id.to_string(),
        platform: target.platform.as_str().to_string(),
        external_id: target.external_id,
        display_name: target.display_name,
        is_active: target.is_active,
        config_json,
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}
