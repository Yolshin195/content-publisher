use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::application::ports::ArticleFilter;
use crate::application::SuggestionInput;
use crate::domain::{ArticleUpdate, NewArticle, VideoPlatform};
use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

pub async fn full_article(state: &AppState, id: Uuid) -> AppResult<ArticleResponse> {
    let article = state.article_service.get(id).await.map_err(AppError::from)?;
    let links = state.article_service.list_video_links(id).await.map_err(AppError::from)?;
    let mut resp = ArticleResponse::from(article);
    resp.video_links = Some(links.into_iter().map(VideoLinkResponse::from).collect());
    Ok(resp)
}

#[utoipa::path(
    get, path = "/api/articles",
    params(ListArticlesQuery),
    responses((status = 200, description = "Список статей с фильтрами", body = ListArticlesResponse)),
    tag = "articles"
)]
pub async fn list_articles(
    State(state): State<AppState>,
    Query(q): Query<ListArticlesQuery>,
) -> AppResult<Json<ListArticlesResponse>> {
    let filter = ArticleFilter {
        state: q.state.clone(),
        origin: q.origin.clone(),
        tag: q.tag.clone(),
        prompt_key: q.prompt_key.clone(),
        q: q.q.clone(),
        has_date: q.has_date,
        from: q.from,
        to: q.to,
        sort: q.sort.clone(),
        page: q.page,
        per_page: q.per_page,
    };
    let (articles, total) = state.article_service.list(filter).await.map_err(AppError::from)?;
    Ok(Json(ListArticlesResponse {
        data: articles.into_iter().map(ArticleResponse::from).collect(),
        meta: Meta { page: q.page, per_page: q.per_page, total },
    }))
}

#[utoipa::path(
    get, path = "/api/articles/{id}",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses(
        (status = 200, description = "Статья вместе со списком видео-ссылок", body = ArticleResponse),
        (status = 404, description = "Не найдена", body = ErrorResponse)
    ),
    tag = "articles"
)]
pub async fn get_article(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<ArticleResponse>> {
    Ok(Json(full_article(&state, id).await?))
}

#[utoipa::path(
    post, path = "/api/articles",
    request_body = CreateArticleRequest,
    responses((status = 201, description = "Черновик создан", body = ArticleResponse)),
    tag = "articles"
)]
pub async fn create_article(
    State(state): State<AppState>,
    Json(body): Json<CreateArticleRequest>,
) -> AppResult<(StatusCode, Json<ArticleResponse>)> {
    let mut new_article = NewArticle::draft(body.title, body.slug.unwrap_or_default(), body.content_html);
    new_article.excerpt = body.excerpt.filter(|e| !e.trim().is_empty());
    new_article.cover_media_id = body.cover_media_id;
    new_article.cover_image_url = body.cover_image_url;
    new_article.tags = body.tags;
    let mut article = state.article_service.create(new_article).await.map_err(AppError::from)?;
    if let Some(at) = body.scheduled_at {
        article = state.scheduling_service.reschedule(article.id, Some(at)).await.map_err(AppError::from)?;
    }
    Ok((StatusCode::CREATED, Json(ArticleResponse::from(article))))
}

#[utoipa::path(
    patch, path = "/api/articles/{id}",
    params(("id" = Uuid, Path, description = "ID статьи")),
    request_body = UpdateArticleRequest,
    responses((status = 200, description = "Статья обновлена", body = ArticleResponse)),
    tag = "articles"
)]
pub async fn update_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateArticleRequest>,
) -> AppResult<Json<ArticleResponse>> {
    state
        .article_service
        .update(
            id,
            ArticleUpdate {
                title: body.title,
                content_html: body.content_html,
                excerpt: body.excerpt.map(|e| e.filter(|v| !v.trim().is_empty())),
                cover_media_id: body.cover_media_id,
                cover_image_url: body.cover_image_url,
                tags: body.tags,
                rationale: body.rationale,
                sources: body.sources.map(|s| s.into_iter().map(Into::into).collect()),
            },
        )
        .await
        .map_err(AppError::from)?;
    if let Some(at) = body.scheduled_at {
        state.scheduling_service.reschedule(id, at).await.map_err(AppError::from)?;
    }
    Ok(Json(full_article(&state, id).await?))
}

#[utoipa::path(
    delete, path = "/api/articles/{id}",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses((status = 204, description = "Статья удалена (soft delete)")),
    tag = "articles"
)]
pub async fn delete_article(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    state.article_service.delete(id).await.map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post, path = "/api/articles/{id}/video-links",
    params(("id" = Uuid, Path, description = "ID статьи")),
    request_body = AddVideoLinkRequest,
    responses((status = 201, description = "Ссылка на видео добавлена", body = VideoLinkResponse)),
    tag = "articles"
)]
pub async fn add_video_link(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddVideoLinkRequest>,
) -> AppResult<(StatusCode, Json<VideoLinkResponse>)> {
    let platform = VideoPlatform::parse(&body.platform).map_err(AppError::from)?;
    let link =
        state.article_service.add_video_link(id, platform, body.url, body.is_primary).await.map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(VideoLinkResponse::from(link))))
}

#[utoipa::path(
    delete, path = "/api/articles/{id}/video-links/{link_id}",
    params(
        ("id" = Uuid, Path, description = "ID статьи"),
        ("link_id" = Uuid, Path, description = "ID видео-ссылки")
    ),
    responses((status = 204, description = "Ссылка удалена")),
    tag = "articles"
)]
pub async fn remove_video_link(
    State(state): State<AppState>,
    Path((id, link_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    state.article_service.remove_video_link(id, link_id).await.map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/api/tags",
    responses((status = 200, description = "Все теги с количеством постов", body = [TagCountResponse])),
    tag = "articles"
)]
pub async fn list_tags(State(state): State<AppState>) -> AppResult<Json<Vec<TagCountResponse>>> {
    let tags = state.article_service.tag_counts().await.map_err(AppError::from)?;
    Ok(Json(tags.into_iter().map(|(tag, count)| TagCountResponse { tag, count }).collect()))
}

// ---------- Предложения от Claude ----------

pub fn suggestion_input(body: CreateSuggestionRequest) -> SuggestionInput {
    SuggestionInput {
        title: body.title,
        content_html: body.content_html,
        excerpt: body.excerpt,
        tags: body.tags,
        cover_image_url: body.cover_image_url,
        prompt_key: body.prompt_key,
        extra_prompt_keys: body.extra_prompt_keys,
        rationale: body.rationale,
        sources: body.sources.into_iter().map(Into::into).collect(),
        publish_at: body.publish_at,
        auto_publish: body.auto_publish,
        target_ids: body.target_ids,
    }
}

#[utoipa::path(
    post, path = "/api/suggestions",
    request_body = CreateSuggestionRequest,
    responses(
        (status = 201, description = "Предложение создано (или сразу запланировано при auto_publish)", body = ArticleResponse),
        (status = 422, description = "Разметка не подходит для Telegram или пост слишком длинный", body = ErrorResponse)
    ),
    tag = "suggestions"
)]
pub async fn create_suggestion(
    State(state): State<AppState>,
    Json(body): Json<CreateSuggestionRequest>,
) -> AppResult<(StatusCode, Json<ArticleResponse>)> {
    let article = state.suggestion_service.create(suggestion_input(body)).await.map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(ArticleResponse::from(article))))
}

#[utoipa::path(
    post, path = "/api/articles/{id}/accept",
    params(("id" = Uuid, Path, description = "ID статьи")),
    request_body = ReviewRequest,
    responses((status = 200, description = "Предложение принято — теперь это черновик", body = ArticleResponse)),
    tag = "suggestions"
)]
pub async fn accept_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Option<Json<ReviewRequest>>,
) -> AppResult<Json<ArticleResponse>> {
    let feedback = body.and_then(|b| b.0.feedback);
    state.suggestion_service.accept(id, feedback).await.map_err(AppError::from)?;
    Ok(Json(full_article(&state, id).await?))
}

#[utoipa::path(
    post, path = "/api/articles/{id}/reject",
    params(("id" = Uuid, Path, description = "ID статьи")),
    request_body = ReviewRequest,
    responses((status = 200, description = "Предложение отклонено", body = ArticleResponse)),
    tag = "suggestions"
)]
pub async fn reject_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Option<Json<ReviewRequest>>,
) -> AppResult<Json<ArticleResponse>> {
    let feedback = body.and_then(|b| b.0.feedback);
    state.suggestion_service.reject(id, feedback).await.map_err(AppError::from)?;
    Ok(Json(full_article(&state, id).await?))
}

#[utoipa::path(
    post, path = "/api/articles/{id}/restore",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses((status = 200, description = "Отклонённое предложение возвращено на рассмотрение", body = ArticleResponse)),
    tag = "suggestions"
)]
pub async fn restore_article(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<ArticleResponse>> {
    state.suggestion_service.restore(id).await.map_err(AppError::from)?;
    Ok(Json(full_article(&state, id).await?))
}

#[utoipa::path(
    post, path = "/api/articles/{id}/archive",
    params(("id" = Uuid, Path, description = "ID статьи")),
    responses((status = 200, description = "Статья перенесена в архив", body = ArticleResponse)),
    tag = "articles"
)]
pub async fn archive_article(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<ArticleResponse>> {
    state.suggestion_service.archive(id).await.map_err(AppError::from)?;
    Ok(Json(full_article(&state, id).await?))
}
