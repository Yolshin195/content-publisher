use askama::Template;
use axum::extract::State;
use axum::response::IntoResponse;
use serde_json::json;

use super::format::json_for_script;
use super::shell::{render, Shell};
use crate::state::AppState;

pub struct ExamplePrompt {
    pub title: &'static str,
    pub schedule: &'static str,
    pub text: String,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    shell: Shell,
    targets_json: String,
    base_url: String,
    mcp_url: String,
    token_configured: bool,
    password_configured: bool,
    publishers: Vec<String>,
    examples: Vec<ExamplePrompt>,
}

pub fn example_prompts() -> Vec<ExamplePrompt> {
    vec![
        ExamplePrompt {
            title: "Утренние новости Rust",
            schedule: "каждый день в 06:00",
            text: "Используй коннектор Content Publisher. Возьми промпт news_pipeline и выполни его для темы Rust: \
                   промпт поиска — rust_news_research, шаблон поста — rust_news_template. \
                   Опубликуй выпуск сегодня в 07:00 по моему времени."
                .into(),
        },
        ExamplePrompt {
            title: "Интересы и идеи постов из наших чатов",
            schedule: "каждый вечер в 21:00",
            text: "Используй коннектор Content Publisher. Возьми промпт chat_digest и выполни его: \
                   разбери мои чаты и сессии за последние сутки, сохрани интересы и предложи посты."
                .into(),
        },
        ExamplePrompt {
            title: "Новости по другой технологии",
            schedule: "по будням в 06:30",
            text: "Используй коннектор Content Publisher. Возьми промпт news_pipeline и выполни его для темы <Технология>: \
                   промпт поиска — <tech>_news_research (если его нет — создай по образцу rust_news_research через save_prompt), \
                   шаблон — rust_news_template. Только предложи пост, не публикуй."
                .into(),
        },
    ]
}

pub async fn settings_page(State(state): State<AppState>) -> impl IntoResponse {
    let targets = state.target_service.list().await.unwrap_or_default();
    let targets_json = json_for_script(
        &targets
            .iter()
            .map(|t| {
                json!({
                    "id": t.id, "platform": t.platform.as_str(), "external_id": t.external_id,
                    "display_name": t.display_name, "is_active": t.is_active, "config": t.config,
                })
            })
            .collect::<Vec<_>>(),
    );
    let mut publishers: Vec<String> = state.config.publisher_registry.keys().cloned().collect();
    publishers.sort();
    let base = state.config.public_base_url.clone();
    render(&SettingsTemplate {
        shell: Shell::load(&state, "settings").await,
        targets_json,
        mcp_url: format!("{base}/mcp"),
        base_url: base,
        token_configured: state.config.api_token.is_some(),
        password_configured: state.config.admin_password.is_some(),
        publishers,
        examples: example_prompts(),
    })
}
