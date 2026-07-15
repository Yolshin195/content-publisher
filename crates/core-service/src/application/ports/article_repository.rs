use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{Article, ArticleState, ArticleUpdate, DomainError, NewArticle, VideoLink, VideoPlatform};

#[derive(Debug, Clone, Default)]
pub struct ArticleFilter {
    pub state: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: i64,
    pub per_page: i64,
}

#[async_trait]
pub trait ArticleRepository: Send + Sync {
    async fn create(&self, new_article: NewArticle) -> Result<Article, DomainError>;
    async fn get(&self, id: Uuid) -> Result<Article, DomainError>;
    async fn list(&self, filter: ArticleFilter) -> Result<(Vec<Article>, i64), DomainError>;
    async fn update(&self, id: Uuid, update: ArticleUpdate) -> Result<Article, DomainError>;
    async fn set_state(
        &self,
        id: Uuid,
        state: ArticleState,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Article, DomainError>;
    async fn soft_delete(&self, id: Uuid) -> Result<(), DomainError>;
    async fn list_by_date_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Article>, DomainError>;

    async fn add_video_link(
        &self,
        article_id: Uuid,
        platform: VideoPlatform,
        url: String,
        is_primary: bool,
    ) -> Result<VideoLink, DomainError>;
    async fn remove_video_link(&self, article_id: Uuid, link_id: Uuid) -> Result<(), DomainError>;
    async fn list_video_links(&self, article_id: Uuid) -> Result<Vec<VideoLink>, DomainError>;
}
