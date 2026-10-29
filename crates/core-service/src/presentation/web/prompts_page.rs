use askama::Template;
use axum::extract::State;
use axum::response::IntoResponse;

use super::shell::{render, Shell};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "prompts.html")]
struct PromptsTemplate {
    shell: Shell,
}

pub async fn prompts_page(State(state): State<AppState>) -> impl IntoResponse {
    render(&PromptsTemplate { shell: Shell::load(&state, "prompts").await })
}
