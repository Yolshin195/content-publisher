use std::sync::Arc;

use uuid::Uuid;

use super::ports::{ArticleFilter, ArticleRepository};
use crate::domain::{
    normalize_tags, slugify, validate_sources, validate_url, Article, ArticleUpdate, DomainError, NewArticle,
    VideoLink, VideoPlatform,
};

pub struct ArticleService {
    repo: Arc<dyn ArticleRepository>,
}

impl ArticleService {
    pub fn new(repo: Arc<dyn ArticleRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, mut new_article: NewArticle) -> Result<Article, DomainError> {
        new_article.title = new_article.title.trim().to_string();
        if new_article.title.is_empty() {
            return Err(DomainError::Validation("title must not be empty".into()));
        }
        if new_article.slug.trim().is_empty() {
            new_article.slug = slugify(&new_article.title);
        }
        new_article.tags = normalize_tags(new_article.tags);
        new_article.cover_image_url = new_article.cover_image_url.filter(|u| !u.trim().is_empty());
        if let Some(url) = &new_article.cover_image_url {
            validate_url(url, "cover_image_url")?;
        }
        validate_sources(&new_article.sources)?;
        self.repo.create(new_article).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Article, DomainError> {
        self.repo.get(id).await
    }

    pub async fn list(&self, filter: ArticleFilter) -> Result<(Vec<Article>, i64), DomainError> {
        self.repo.list(filter).await
    }

    pub async fn update(&self, id: Uuid, mut update: ArticleUpdate) -> Result<Article, DomainError> {
        if let Some(title) = update.title.as_mut() {
            *title = title.trim().to_string();
            if title.is_empty() {
                return Err(DomainError::Validation("title must not be empty".into()));
            }
        }
        update.tags = update.tags.map(normalize_tags);
        update.cover_image_url = update.cover_image_url.map(|u| u.filter(|u| !u.trim().is_empty()));
        if let Some(Some(url)) = &update.cover_image_url {
            validate_url(url, "cover_image_url")?;
        }
        if let Some(sources) = &update.sources {
            validate_sources(sources)?;
        }
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
        validate_url(&url, "video url")?;
        self.repo.add_video_link(article_id, platform, url, is_primary).await
    }

    pub async fn remove_video_link(&self, article_id: Uuid, link_id: Uuid) -> Result<(), DomainError> {
        self.repo.remove_video_link(article_id, link_id).await
    }

    pub async fn list_video_links(&self, article_id: Uuid) -> Result<Vec<VideoLink>, DomainError> {
        self.repo.list_video_links(article_id).await
    }

    pub async fn tag_counts(&self) -> Result<Vec<(String, i64)>, DomainError> {
        self.repo.tag_counts().await
    }
}
