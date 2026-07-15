use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::dto::{ErrorBody, ErrorResponse};
use crate::domain::DomainError;

pub struct AppError(pub DomainError);

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        AppError(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            DomainError::ArticleNotFound => (StatusCode::NOT_FOUND, "ARTICLE_NOT_FOUND"),
            DomainError::TargetNotFound => (StatusCode::NOT_FOUND, "TARGET_NOT_FOUND"),
            DomainError::TaskNotFound => (StatusCode::NOT_FOUND, "TASK_NOT_FOUND"),
            DomainError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"),
            DomainError::InvalidStateTransition(_) => (StatusCode::CONFLICT, "INVALID_STATE_TRANSITION"),
            DomainError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            DomainError::Gateway(_) => (StatusCode::BAD_GATEWAY, "GATEWAY_ERROR"),
        };

        let message = self.0.to_string();
        tracing::error!(code = code, message = %message, "request failed");

        (status, Json(ErrorResponse { error: ErrorBody { code: code.to_string(), message } })).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
