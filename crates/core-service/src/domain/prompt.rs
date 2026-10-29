use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DomainError;

/// Узел справочника промптов: папка (для группировки) или промпт с ключом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Folder,
    Prompt,
}

impl PromptKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptKind::Folder => "folder",
            PromptKind::Prompt => "prompt",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "folder" => Ok(PromptKind::Folder),
            "prompt" => Ok(PromptKind::Prompt),
            other => Err(DomainError::Validation(format!("unknown prompt kind: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: PromptKind,
    /// Уникальный (без учёта регистра) ключ, по которому промпт запрашивают
    /// из запланированных задач. У папок ключа нет.
    pub key: Option<String>,
    pub title: String,
    pub description: String,
    pub body: String,
    pub version: i32,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVersion {
    pub id: Uuid,
    pub prompt_id: Uuid,
    pub version: i32,
    pub title: String,
    pub description: String,
    pub body: String,
    pub change_note: Option<String>,
    /// Кто сохранил версию: `user` (через интерфейс) или `claude` (через API/MCP).
    pub author: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPromptNode {
    pub parent_id: Option<Uuid>,
    pub kind: PromptKind,
    pub key: Option<String>,
    pub title: String,
    pub description: String,
    pub body: String,
    pub change_note: Option<String>,
    pub author: String,
}

#[derive(Debug, Clone, Default)]
pub struct PromptNodeUpdate {
    pub key: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub body: Option<String>,
    pub change_note: Option<String>,
    pub author: Option<String>,
}

/// Промпт с раскрытыми включениями `{{> key}}`.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPrompt {
    pub key: String,
    pub title: String,
    pub version: i32,
    pub body: String,
    /// Все задействованные промпты (сам промпт первым) с их текущими версиями.
    pub refs: Vec<super::PromptRef>,
    pub warnings: Vec<String>,
}

/// Ключ: латиница, цифры, `_`, `-`, `.`; 2–100 символов. Регистр не важен при поиске.
pub fn validate_key(key: &str) -> Result<(), DomainError> {
    let ok_len = (2..=100).contains(&key.len());
    let ok_chars = key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if ok_len && ok_chars {
        Ok(())
    } else {
        Err(DomainError::Validation(format!(
            "invalid prompt key '{key}': use 2-100 latin letters, digits, '_', '-' or '.'"
        )))
    }
}

/// Находит включения вида `{{> key}}` в тексте промпта.
/// Возвращает (диапазон байт вхождения, ключ).
pub fn find_includes(body: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out = Vec::new();
    let mut offset = 0;
    while let Some(start) = body[offset..].find("{{>") {
        let abs_start = offset + start;
        let Some(end_rel) = body[abs_start..].find("}}") else { break };
        let abs_end = abs_start + end_rel + 2;
        let key = body[abs_start + 3..abs_end - 2].trim().to_string();
        if !key.is_empty() && validate_key(&key).is_ok() {
            out.push((abs_start..abs_end, key));
        }
        offset = abs_end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_includes() {
        let found = find_includes("a {{> author_style}} b {{>telegram_format }} c {{ not }}");
        let keys: Vec<_> = found.iter().map(|(_, k)| k.as_str()).collect();
        assert_eq!(keys, vec!["author_style", "telegram_format"]);
    }

    #[test]
    fn validates_keys() {
        assert!(validate_key("Rust_news_template").is_ok());
        assert!(validate_key("news.rust-v2").is_ok());
        assert!(validate_key("с пробелом").is_err());
        assert!(validate_key("x").is_err());
    }
}
