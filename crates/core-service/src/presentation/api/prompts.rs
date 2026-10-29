use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::application::prompt_service::PromptDraft;
use crate::domain::{NewPromptNode, PromptKind, PromptNodeUpdate};
use crate::presentation::auth::Actor;
use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

async fn prompt_response(state: &AppState, node: crate::domain::PromptNode) -> AppResult<PromptResponse> {
    let path = state.prompt_service.path_of(&node).await.map_err(AppError::from)?;
    Ok(PromptResponse::new(node, path))
}

#[utoipa::path(
    get, path = "/api/prompts",
    responses((status = 200, description = "Дерево промптов (папки и промпты, без текста)", body = [PromptTreeResponse])),
    tag = "prompts"
)]
pub async fn tree(State(state): State<AppState>) -> AppResult<Json<Vec<PromptTreeResponse>>> {
    let tree = state.prompt_service.tree().await.map_err(AppError::from)?;
    Ok(Json(tree.into_iter().map(PromptTreeResponse::from).collect()))
}

#[utoipa::path(
    get, path = "/api/prompts/{id}",
    params(("id" = Uuid, Path, description = "ID узла")),
    responses((status = 200, description = "Узел справочника с текстом", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn get_prompt(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<PromptResponse>> {
    let node = state.prompt_service.get(id).await.map_err(AppError::from)?;
    Ok(Json(prompt_response(&state, node).await?))
}

#[utoipa::path(
    get, path = "/api/prompts/by-key/{key}",
    params(("key" = String, Path, description = "Ключ промпта (регистр не важен)"), ResolveQuery),
    responses(
        (status = 200, description = "Текст промпта; по умолчанию с раскрытыми {{> key}}", body = ResolvedPromptResponse),
        (status = 404, description = "Промпт не найден", body = ErrorResponse)
    ),
    tag = "prompts"
)]
pub async fn get_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<ResolveQuery>,
) -> AppResult<Json<ResolvedPromptResponse>> {
    if q.resolve.unwrap_or(true) {
        let resolved = state.prompt_service.resolve(&key).await.map_err(AppError::from)?;
        return Ok(Json(resolved.into()));
    }
    let node = state.prompt_service.get_by_key(&key).await.map_err(AppError::from)?;
    Ok(Json(ResolvedPromptResponse {
        key: node.key.clone().unwrap_or_default(),
        title: node.title,
        version: node.version,
        body: node.body,
        refs: vec![PromptRefDto { key: node.key.unwrap_or_default(), version: node.version }],
        warnings: Vec::new(),
    }))
}

#[utoipa::path(
    put, path = "/api/prompts/by-key/{key}",
    params(("key" = String, Path, description = "Ключ промпта")),
    request_body = SavePromptByKeyRequest,
    responses((status = 200, description = "Промпт создан или обновлён (новая версия)", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn save_by_key(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(key): Path<String>,
    Json(body): Json<SavePromptByKeyRequest>,
) -> AppResult<Json<PromptResponse>> {
    let draft = PromptDraft {
        key,
        body: body.body,
        title: body.title,
        description: body.description,
        folder_path: body.folder_path,
        change_note: body.change_note,
    };
    let node = state.prompt_service.save_by_key(draft, actor.as_str()).await.map_err(AppError::from)?;
    Ok(Json(prompt_response(&state, node).await?))
}

#[utoipa::path(
    post, path = "/api/prompts",
    request_body = CreatePromptRequest,
    responses((status = 201, description = "Папка или промпт создан", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Json(body): Json<CreatePromptRequest>,
) -> AppResult<(StatusCode, Json<PromptResponse>)> {
    let kind = PromptKind::parse(&body.kind).map_err(AppError::from)?;
    let node = state
        .prompt_service
        .create(NewPromptNode {
            parent_id: body.parent_id,
            kind,
            key: body.key,
            title: body.title,
            description: body.description,
            body: body.body,
            change_note: body.change_note,
            author: actor.as_str().to_string(),
        })
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(prompt_response(&state, node).await?)))
}

#[utoipa::path(
    patch, path = "/api/prompts/{id}",
    params(("id" = Uuid, Path, description = "ID узла")),
    request_body = UpdatePromptRequest,
    responses((status = 200, description = "Обновлено; при изменении текста создана новая версия", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePromptRequest>,
) -> AppResult<Json<PromptResponse>> {
    let node = state
        .prompt_service
        .update(
            id,
            PromptNodeUpdate {
                key: body.key,
                title: body.title,
                description: body.description,
                body: body.body,
                change_note: body.change_note,
                author: Some(actor.as_str().to_string()),
            },
        )
        .await
        .map_err(AppError::from)?;
    Ok(Json(prompt_response(&state, node).await?))
}

#[utoipa::path(
    post, path = "/api/prompts/{id}/move",
    params(("id" = Uuid, Path, description = "ID узла")),
    request_body = MovePromptRequest,
    responses((status = 200, description = "Узел перемещён", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn move_node(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<MovePromptRequest>,
) -> AppResult<Json<PromptResponse>> {
    let node = state.prompt_service.move_node(id, body.parent_id, body.position).await.map_err(AppError::from)?;
    Ok(Json(prompt_response(&state, node).await?))
}

#[utoipa::path(
    delete, path = "/api/prompts/{id}",
    params(("id" = Uuid, Path, description = "ID узла")),
    responses((status = 204, description = "Удалено вместе с вложенными узлами и историей")),
    tag = "prompts"
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    state.prompt_service.delete(id).await.map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/api/prompts/{id}/versions",
    params(("id" = Uuid, Path, description = "ID промпта")),
    responses((status = 200, description = "История версий, от новых к старым", body = [PromptVersionResponse])),
    tag = "prompts"
)]
pub async fn versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<PromptVersionResponse>>> {
    let versions = state.prompt_service.versions(id).await.map_err(AppError::from)?;
    Ok(Json(versions.into_iter().map(PromptVersionResponse::from).collect()))
}

#[utoipa::path(
    get, path = "/api/prompts/{id}/diff",
    params(("id" = Uuid, Path, description = "ID промпта"), DiffQuery),
    responses((status = 200, description = "Построчный diff между версиями", body = [DiffLineResponse])),
    tag = "prompts"
)]
pub async fn diff(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<DiffQuery>,
) -> AppResult<Json<Vec<DiffLineResponse>>> {
    let lines = state.prompt_service.diff(id, q.from, q.to).await.map_err(AppError::from)?;
    Ok(Json(lines.into_iter().map(DiffLineResponse::from).collect()))
}

#[utoipa::path(
    post, path = "/api/prompts/{id}/restore/{version}",
    params(("id" = Uuid, Path, description = "ID промпта"), ("version" = i32, Path, description = "Номер версии")),
    responses((status = 200, description = "Текст версии восстановлен как новая версия", body = PromptResponse)),
    tag = "prompts"
)]
pub async fn restore(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path((id, version)): Path<(Uuid, i32)>,
) -> AppResult<Json<PromptResponse>> {
    let node = state.prompt_service.restore(id, version, actor.as_str()).await.map_err(AppError::from)?;
    Ok(Json(prompt_response(&state, node).await?))
}

#[utoipa::path(
    get, path = "/api/prompts/{id}/markdown",
    params(("id" = Uuid, Path, description = "ID промпта")),
    responses((status = 200, description = "Промпт как .md файл с front matter", content_type = "text/markdown")),
    tag = "prompts"
)]
pub async fn export_one(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<impl IntoResponse> {
    let file = state.prompt_service.export_markdown(id).await.map_err(AppError::from)?;
    let filename = file.path.rsplit('/').next().unwrap_or("prompt.md").to_string();
    let ascii_name: String = filename.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    Ok((
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{ascii_name}\"")),
        ],
        file.content,
    ))
}

#[utoipa::path(
    get, path = "/api/prompts/export",
    responses((status = 200, description = "Все промпты как .md файлы (для git)", body = [PromptFileResponse])),
    tag = "prompts"
)]
pub async fn export_all(State(state): State<AppState>) -> AppResult<Json<Vec<PromptFileResponse>>> {
    let files = state.prompt_service.export_all().await.map_err(AppError::from)?;
    Ok(Json(files.into_iter().map(PromptFileResponse::from).collect()))
}

#[utoipa::path(
    post, path = "/api/prompts/import",
    request_body = ImportPromptsRequest,
    responses((status = 200, description = "Импортированные/обновлённые промпты", body = [PromptResponse])),
    tag = "prompts"
)]
pub async fn import(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Json(body): Json<ImportPromptsRequest>,
) -> AppResult<Json<Vec<PromptResponse>>> {
    let mut out = Vec::new();
    for content in body.files {
        let node = state.prompt_service.import_markdown(&content, actor.as_str()).await.map_err(AppError::from)?;
        out.push(prompt_response(&state, node).await?);
    }
    Ok(Json(out))
}
