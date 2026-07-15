use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::error::DomainError;

/// Реестр поддерживаемых соцсетей. Добавление новой платформы = новая запись
/// в этом enum + новый Publisher-микросервис с тем же HTTP-контрактом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Telegram,
    Vk,
    X,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Telegram => "telegram",
            Platform::Vk => "vk",
            Platform::X => "x",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "telegram" => Ok(Platform::Telegram),
            "vk" => Ok(Platform::Vk),
            "x" => Ok(Platform::X),
            other => Err(DomainError::Validation(format!("unknown platform: {other}"))),
        }
    }
}

/// Обобщённая «точка публикации» — заменяет специфичный для Telegram «Channel».
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationTarget {
    pub id: Uuid,
    pub platform: Platform,
    pub external_id: String,
    pub display_name: String,
    pub is_active: bool,
    pub config: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPublicationTarget {
    pub platform: Platform,
    pub external_id: String,
    pub display_name: String,
    pub config: Value,
}
