use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::application::ports::ArticleFilter;
use crate::domain::{ArticleUpdate, NewArticle, VideoPlatform};
use crate::presentation::dto::*;
use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[utoipa::path(
    get, path = "/api/articles",
    params(ListArticlesQuery),
    responses((status = 200, description = "Список статей", body = ListArticlesResponse)),
    tag = "articles"
)]
pub async fn list_articles(
    State(state): State<AppState>,
    Query(q): Query<ListArticlesQuery>,
) -> AppResult<Json<ListArticlesResponse>> {
    let filter =
        ArticleFilter { state: q.state.clone(), from: q.from, to: q.to, page: q.page, per_page: q.per_page };
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
    let article = state.article_service.get(id).await.map_err(AppError::from)?;
    let links = state.article_service.list_video_links(id).await.map_err(AppError::from)?;
    let mut resp = ArticleResponse::from(article);
    resp.video_links = Some(links.into_iter().map(VideoLinkResponse::from).collect());
    Ok(Json(resp))
}

#[utoipa::path(
    post, path = "/api/articles",
    request_body = CreateArticleRequest,
    responses((status = 201, description = "Статья создана", body = ArticleResponse)),
    tag = "articles"
)]
pub async fn create_article(
    State(state): State<AppState>,
    Json(body): Json<CreateArticleRequest>,
) -> AppResult<(StatusCode, Json<ArticleResponse>)> {
    let article = state
        .article_service
        .create(NewArticle {
            title: body.title,
            slug: body.slug,
            content_html: body.content_html,
            excerpt: body.excerpt,
            cover_media_id: body.cover_media_id,
        })
        .await
        .map_err(AppError::from)?;
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
    let article = state
        .article_service
        .update(
            id,
            ArticleUpdate {
                title: body.title,
                content_html: body.content_html,
                excerpt: body.excerpt.map(Some),
                cover_media_id: body.cover_media_id.map(Some),
            },
        )
        .await
        .map_err(AppError::from)?;
    Ok(Json(ArticleResponse::from(article)))
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
