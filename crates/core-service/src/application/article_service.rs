use std::sync::Arc;

use uuid::Uuid;

use super::ports::{ArticleFilter, ArticleRepository};
use crate::domain::{Article, ArticleUpdate, DomainError, NewArticle, VideoLink, VideoPlatform};

pub struct ArticleService {
    repo: Arc<dyn ArticleRepository>,
}

impl ArticleService {
    pub fn new(repo: Arc<dyn ArticleRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, new_article: NewArticle) -> Result<Article, DomainError> {
        if new_article.title.trim().is_empty() {
            return Err(DomainError::Validation("title must not be empty".into()));
        }
        if new_article.slug.trim().is_empty() {
            return Err(DomainError::Validation("slug must not be empty".into()));
        }
        self.repo.create(new_article).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Article, DomainError> {
        self.repo.get(id).await
    }

    pub async fn list(&self, filter: ArticleFilter) -> Result<(Vec<Article>, i64), DomainError> {
        self.repo.list(filter).await
    }

    pub async fn update(&self, id: Uuid, update: ArticleUpdate) -> Result<Article, DomainError> {
        self.repo.update(id, update).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.repo.soft_delete(id).await
    }

    pub async fn add_video_link(
        &self,
        article_id: Uuid,
        platform: VideoPlatform,
        url: String,
        is_primary: bool,
    ) -> Result<VideoLink, DomainError> {
        if url.trim().is_empty() {
            return Err(DomainError::Validation("video url must not be empty".into()));
        }
        self.repo.add_video_link(article_id, platform, url, is_primary).await
    }

    pub async fn remove_video_link(&self, article_id: Uuid, link_id: Uuid) -> Result<(), DomainError> {
        self.repo.remove_video_link(article_id, link_id).await
    }

    pub async fn list_video_links(&self, article_id: Uuid) -> Result<Vec<VideoLink>, DomainError> {
        self.repo.list_video_links(article_id).await
    }
}
