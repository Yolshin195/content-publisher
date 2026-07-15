use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("article not found")]
    ArticleNotFound,
    #[error("publication target not found")]
    TargetNotFound,
    #[error("publication task not found")]
    TaskNotFound,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("invalid state transition: {0}")]
    InvalidStateTransition(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("gateway error: {0}")]
    Gateway(String),
}
