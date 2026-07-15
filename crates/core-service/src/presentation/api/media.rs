use axum::extract::State;
use axum::Json;
use shared_contracts::MediaUploadUrlResponse;

use crate::presentation::error_response::{AppError, AppResult};
use crate::state::AppState;

#[utoipa::path(
    post, path = "/api/media/upload-url",
    responses((status = 200, description = "Presigned URL для прямой загрузки файла в Media Service", body = MediaUploadUrlResponse)),
    tag = "media"
)]
pub async fn create_upload_url(State(state): State<AppState>) -> AppResult<Json<MediaUploadUrlResponse>> {
    let resp = state.media_client.create_upload_url().await.map_err(AppError::from)?;
    Ok(Json(resp))
}
