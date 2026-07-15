use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use shared_contracts::PublishRequest;

use crate::telegram;
use crate::AppState;

/// Единая точка входа контракта Core Service <-> Publisher-микросервисы.
/// HTTP-статус 200 возвращается и при успехе, и при бизнес-ошибке публикации —
/// исход кодируется полем `status` внутри `PublishResult`. Не-2xx оставлен для
/// случаев, когда сам запрос некорректен на транспортном уровне (это делает
/// автоматически axum-экстрактор `Json`, если тело не парсится).
#[utoipa::path(get, path = "/publish", responses((status = OK)))]
pub async fn publish(State(state): State<AppState>, Json(request): Json<PublishRequest>) -> impl axum::response::IntoResponse {
    tracing::info!(task_id = %request.task_id, target = %request.target.external_id, "received publish request");
    let result = telegram::publish(&state.bot, request).await;

    if let Some(error) = &result.error {
        tracing::warn!(task_id = %result.task_id, error = %error, "publish failed");
    } else {
        tracing::info!(task_id = %result.task_id, external_post_id = ?result.external_post_id, "publish succeeded");
    }

    (StatusCode::OK, Json(result))
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}
