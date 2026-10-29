use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use uuid::Uuid;

use super::ports::PromptRepository;
use crate::domain::{
    find_includes, validate_key, DomainError, NewPromptNode, PromptKind, PromptNode, PromptNodeUpdate, PromptRef,
    PromptVersion, ResolvedPrompt,
};

/// Узел дерева для интерфейса и API (без текста промпта — он грузится отдельно).
#[derive(Debug, Clone, Serialize)]
pub struct PromptTreeNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: PromptKind,
    pub key: Option<String>,
    pub title: String,
    pub description: String,
    pub version: i32,
    /// Путь из названий папок-предков: `Новости/Rust`.
    pub path: String,
    pub children: Vec<PromptTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    /// `equal` | `insert` | `delete`
    pub tag: &'static str,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

/// Файл экспорта промпта в Markdown (для хранения в git и просмотра diff).
#[derive(Debug, Clone, Serialize)]
pub struct PromptFile {
    pub path: String,
    pub content: String,
}

/// Поля, извлечённые из Markdown-файла промпта с front matter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedPromptMarkdown {
    pub key: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub path: Option<String>,
    pub body: String,
}

/// Промпт, сохраняемый по ключу (Claude, импорт Markdown, REST `PUT /by-key`).
#[derive(Debug, Clone, Default)]
pub struct PromptDraft {
    pub key: String,
    pub body: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Путь папок вида `Новости/Rust`; недостающие папки создаются.
    pub folder_path: Option<String>,
    pub change_note: Option<String>,
}

const MAX_INCLUDE_DEPTH: usize = 8;

pub struct PromptService {
    repo: Arc<dyn PromptRepository>,
}

impl PromptService {
    pub fn new(repo: Arc<dyn PromptRepository>) -> Self {
        Self { repo }
    }

    pub async fn tree(&self) -> Result<Vec<PromptTreeNode>, DomainError> {
        let nodes = self.repo.list_all().await?;
        Ok(build_tree(&nodes))
    }

    pub async fn get(&self, id: Uuid) -> Result<PromptNode, DomainError> {
        self.repo.get(id).await
    }

    pub async fn get_by_key(&self, key: &str) -> Result<PromptNode, DomainError> {
        self.repo.get_by_key(key.trim()).await
    }

    /// Путь папок до узла (`Новости/Rust`), пустая строка — корень.
    pub async fn path_of(&self, node: &PromptNode) -> Result<String, DomainError> {
        let nodes = self.repo.list_all().await?;
        let by_id: HashMap<Uuid, &PromptNode> = nodes.iter().map(|n| (n.id, n)).collect();
        Ok(folder_path(node.parent_id, &by_id))
    }

    /// Текст промпта с раскрытыми `{{> key}}` (рекурсивно, с защитой от циклов).
    pub async fn resolve(&self, key: &str) -> Result<ResolvedPrompt, DomainError> {
        let root = self.repo.get_by_key(key.trim()).await?;
        if root.kind != PromptKind::Prompt {
            return Err(DomainError::Validation(format!("'{key}' is a folder, not a prompt")));
        }
        let mut refs = vec![PromptRef { key: root.key.clone().unwrap_or_default(), version: root.version }];
        let mut warnings = Vec::new();
        let mut stack = vec![root.key.clone().unwrap_or_default().to_lowercase()];
        let body = self.expand(&root.body, &mut stack, &mut refs, &mut warnings).await?;
        Ok(ResolvedPrompt {
            key: root.key.unwrap_or_default(),
            title: root.title,
            version: root.version,
            body,
            refs,
            warnings,
        })
    }

    fn expand<'a>(
        &'a self,
        body: &'a str,
        stack: &'a mut Vec<String>,
        refs: &'a mut Vec<PromptRef>,
        warnings: &'a mut Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, DomainError>> + Send + 'a>> {
        Box::pin(async move {
            let includes = find_includes(body);
            if includes.is_empty() {
                return Ok(body.to_string());
            }
            let mut out = String::with_capacity(body.len());
            let mut last = 0;
            for (range, key) in includes {
                out.push_str(&body[last..range.start]);
                last = range.end;
                let lower = key.to_lowercase();
                if stack.contains(&lower) {
                    warnings.push(format!("циклическое включение {{{{> {key}}}}} пропущено"));
                    continue;
                }
                if stack.len() >= MAX_INCLUDE_DEPTH {
                    warnings.push(format!("слишком глубокая вложенность на {{{{> {key}}}}}"));
                    continue;
                }
                match self.repo.get_by_key(&key).await {
                    Ok(node) if node.kind == PromptKind::Prompt => {
                        if !refs.iter().any(|r| r.key.eq_ignore_ascii_case(&key)) {
                            refs.push(PromptRef { key: node.key.clone().unwrap_or(key.clone()), version: node.version });
                        }
                        stack.push(lower);
                        let expanded = self.expand(&node.body, stack, refs, warnings).await?;
                        stack.pop();
                        out.push_str(expanded.trim_end());
                    }
                    Ok(_) | Err(DomainError::PromptNotFound(_)) => {
                        warnings.push(format!("промпт '{key}' из {{{{> {key}}}}} не найден"));
                        out.push_str(&body[range]);
                    }
                    Err(e) => return Err(e),
                }
            }
            out.push_str(&body[last..]);
            Ok(out)
        })
    }

    /// Текущие версии промптов (и всего, что они подключают) — фиксируются в посте
    /// при генерации, чтобы потом сравнивать результат разных версий.
    pub async fn collect_refs(&self, keys: &[String]) -> Result<Vec<PromptRef>, DomainError> {
        let mut refs: Vec<PromptRef> = Vec::new();
        for key in keys {
            match self.resolve(key).await {
                Ok(resolved) => {
                    for r in resolved.refs {
                        if !refs.iter().any(|x| x.key.eq_ignore_ascii_case(&r.key)) {
                            refs.push(r);
                        }
                    }
                }
                Err(DomainError::PromptNotFound(_)) => {
                    return Err(DomainError::Validation(format!("prompt '{key}' not found")));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(refs)
    }

    pub async fn create(&self, mut node: NewPromptNode) -> Result<PromptNode, DomainError> {
        node.title = node.title.trim().to_string();
        node.key = node.key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
        match node.kind {
            PromptKind::Prompt => {
                let key =
                    node.key.as_deref().ok_or_else(|| DomainError::Validation("prompt key is required".into()))?;
                validate_key(key)?;
                if node.title.is_empty() {
                    node.title = key.to_string();
                }
            }
            PromptKind::Folder => {
                if node.title.is_empty() {
                    return Err(DomainError::Validation("folder title is required".into()));
                }
                node.key = None;
            }
        }
        if let Some(parent_id) = node.parent_id {
            self.ensure_folder(parent_id).await?;
        }
        self.repo.create(node).await
    }

    pub async fn update(&self, id: Uuid, mut update: PromptNodeUpdate) -> Result<PromptNode, DomainError> {
        let current = self.repo.get(id).await?;
        if let Some(key) = update.key.as_mut() {
            *key = key.trim().to_string();
            match current.kind {
                PromptKind::Prompt => validate_key(key)?,
                PromptKind::Folder => *key = String::new(),
            }
        }
        if let Some(title) = update.title.as_mut() {
            *title = title.trim().to_string();
            if title.is_empty() {
                return Err(DomainError::Validation("title must not be empty".into()));
            }
        }
        self.repo.update(id, update).await
    }

    /// Создать или обновить промпт по ключу — основной способ для Claude.
    pub async fn save_by_key(&self, draft: PromptDraft, author: &str) -> Result<PromptNode, DomainError> {
        let PromptDraft { key, body, title, description, folder_path, change_note } = draft;
        let key = key.trim();
        validate_key(key)?;
        let parent_id = match folder_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            Some(path) => Some(self.ensure_folder_path(path).await?),
            None => None,
        };

        match self.repo.get_by_key(key).await {
            Ok(existing) => {
                if existing.kind != PromptKind::Prompt {
                    return Err(DomainError::Validation(format!("'{key}' is a folder")));
                }
                let updated = self
                    .repo
                    .update(
                        existing.id,
                        PromptNodeUpdate {
                            key: None,
                            title,
                            description,
                            body: Some(body),
                            change_note,
                            author: Some(author.to_string()),
                        },
                    )
                    .await?;
                match parent_id {
                    Some(pid) if Some(pid) != existing.parent_id => self.move_node(updated.id, Some(pid), i32::MAX).await,
                    _ => Ok(updated),
                }
            }
            Err(DomainError::PromptNotFound(_)) => {
                self.repo
                    .create(NewPromptNode {
                        parent_id,
                        kind: PromptKind::Prompt,
                        key: Some(key.to_string()),
                        title: title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| key.to_string()),
                        description: description.unwrap_or_default(),
                        body,
                        change_note,
                        author: author.to_string(),
                    })
                    .await
            }
            Err(e) => Err(e),
        }
    }

    /// Находит или создаёт цепочку папок по пути `A/B/C`, возвращает id последней.
    pub async fn ensure_folder_path(&self, path: &str) -> Result<Uuid, DomainError> {
        let nodes = self.repo.list_all().await?;
        let mut parent: Option<Uuid> = None;
        for segment in path.split('/').map(str::trim).filter(|s| !s.is_empty()) {
            let existing = nodes.iter().find(|n| {
                n.kind == PromptKind::Folder && n.parent_id == parent && n.title.eq_ignore_ascii_case(segment)
            });
            parent = Some(match existing {
                Some(n) => n.id,
                None => {
                    self.repo
                        .create(NewPromptNode {
                            parent_id: parent,
                            kind: PromptKind::Folder,
                            key: None,
                            title: segment.to_string(),
                            description: String::new(),
                            body: String::new(),
                            change_note: None,
                            author: "user".into(),
                        })
                        .await?
                        .id
                }
            });
        }
        parent.ok_or_else(|| DomainError::Validation("empty folder path".into()))
    }

    pub async fn move_node(&self, id: Uuid, parent_id: Option<Uuid>, position: i32) -> Result<PromptNode, DomainError> {
        let nodes = self.repo.list_all().await?;
        if let Some(pid) = parent_id {
            if pid == id {
                return Err(DomainError::Validation("cannot move a node into itself".into()));
            }
            let by_id: HashMap<Uuid, &PromptNode> = nodes.iter().map(|n| (n.id, n)).collect();
            let parent = by_id.get(&pid).ok_or_else(|| DomainError::PromptNotFound(pid.to_string()))?;
            if parent.kind != PromptKind::Folder {
                return Err(DomainError::Validation("target must be a folder".into()));
            }
            // Нельзя переносить папку внутрь собственного потомка.
            let mut cursor = parent.parent_id;
            while let Some(c) = cursor {
                if c == id {
                    return Err(DomainError::Validation("cannot move a folder into its own subfolder".into()));
                }
                cursor = by_id.get(&c).and_then(|n| n.parent_id);
            }
        }
        let siblings = nodes.iter().filter(|n| n.parent_id == parent_id && n.id != id).count() as i32;
        self.repo.move_node(id, parent_id, position.clamp(0, siblings)).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.repo.delete(id).await
    }

    pub async fn versions(&self, id: Uuid) -> Result<Vec<PromptVersion>, DomainError> {
        self.repo.list_versions(id).await
    }

    pub async fn version(&self, id: Uuid, version: i32) -> Result<PromptVersion, DomainError> {
        self.repo.get_version(id, version).await
    }

    /// Построчный diff между двумя версиями (`to` = None — текущий текст).
    pub async fn diff(&self, id: Uuid, from: i32, to: Option<i32>) -> Result<Vec<DiffLine>, DomainError> {
        let old = self.repo.get_version(id, from).await?.body;
        let new = match to {
            Some(v) => self.repo.get_version(id, v).await?.body,
            None => self.repo.get(id).await?.body,
        };
        Ok(diff_lines(&old, &new))
    }

    pub async fn restore(&self, id: Uuid, version: i32, author: &str) -> Result<PromptNode, DomainError> {
        let v = self.repo.get_version(id, version).await?;
        self.repo
            .update(
                id,
                PromptNodeUpdate {
                    key: None,
                    title: Some(v.title),
                    description: Some(v.description),
                    body: Some(v.body),
                    change_note: Some(format!("Восстановлено из версии {version}")),
                    author: Some(author.to_string()),
                },
            )
            .await
    }

    pub async fn export_markdown(&self, id: Uuid) -> Result<PromptFile, DomainError> {
        let node = self.repo.get(id).await?;
        if node.kind != PromptKind::Prompt {
            return Err(DomainError::Validation("only prompts can be exported".into()));
        }
        let path = self.path_of(&node).await?;
        Ok(prompt_file(&node, &path))
    }

    /// Все промпты как набор .md файлов, разложенных по папкам дерева.
    pub async fn export_all(&self) -> Result<Vec<PromptFile>, DomainError> {
        let nodes = self.repo.list_all().await?;
        let by_id: HashMap<Uuid, &PromptNode> = nodes.iter().map(|n| (n.id, n)).collect();
        let mut files: Vec<PromptFile> = nodes
            .iter()
            .filter(|n| n.kind == PromptKind::Prompt)
            .map(|n| prompt_file(n, &folder_path(n.parent_id, &by_id)))
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    pub async fn import_markdown(&self, content: &str, author: &str) -> Result<PromptNode, DomainError> {
        let parsed = parse_prompt_markdown(content)?;
        let draft = PromptDraft {
            key: parsed.key,
            body: parsed.body,
            title: parsed.title,
            description: parsed.description,
            folder_path: parsed.path,
            change_note: Some(if author == "seed" { "Стартовая версия" } else { "Импорт из Markdown" }.into()),
        };
        self.save_by_key(draft, author).await
    }

    /// Стартовый набор промптов — загружается только в пустой справочник, чтобы
    /// не воскрешать промпты, которые автор удалил сознательно.
    pub async fn seed_if_empty(&self, files: &[&str]) -> Result<usize, DomainError> {
        if !self.repo.list_all().await?.is_empty() {
            return Ok(0);
        }
        for content in files {
            self.import_markdown(content, "seed").await?;
        }
        Ok(files.len())
    }

    async fn ensure_folder(&self, id: Uuid) -> Result<(), DomainError> {
        let parent = self.repo.get(id).await?;
        if parent.kind != PromptKind::Folder {
            return Err(DomainError::Validation("parent must be a folder".into()));
        }
        Ok(())
    }
}

fn folder_path(mut parent: Option<Uuid>, by_id: &HashMap<Uuid, &PromptNode>) -> String {
    let mut parts = Vec::new();
    let mut seen = HashSet::new();
    while let Some(pid) = parent {
        if !seen.insert(pid) {
            break;
        }
        match by_id.get(&pid) {
            Some(n) => {
                parts.push(n.title.clone());
                parent = n.parent_id;
            }
            None => break,
        }
    }
    parts.reverse();
    parts.join("/")
}

pub fn build_tree(nodes: &[PromptNode]) -> Vec<PromptTreeNode> {
    let by_id: HashMap<Uuid, &PromptNode> = nodes.iter().map(|n| (n.id, n)).collect();
    let mut children: HashMap<Option<Uuid>, Vec<&PromptNode>> = HashMap::new();
    for n in nodes {
        // Узел с потерянным родителем показываем в корне, а не теряем.
        let parent = n.parent_id.filter(|p| by_id.contains_key(p));
        children.entry(parent).or_default().push(n);
    }
    fn build(
        parent: Option<Uuid>,
        children: &HashMap<Option<Uuid>, Vec<&PromptNode>>,
        by_id: &HashMap<Uuid, &PromptNode>,
        depth: usize,
    ) -> Vec<PromptTreeNode> {
        let Some(list) = children.get(&parent) else { return Vec::new() };
        let mut list = list.clone();
        // Папки сверху, затем по позиции и названию.
        list.sort_by(|a, b| {
            (a.kind != PromptKind::Folder)
                .cmp(&(b.kind != PromptKind::Folder))
                .then(a.position.cmp(&b.position))
                .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        list.into_iter()
            .map(|n| PromptTreeNode {
                id: n.id,
                parent_id: n.parent_id,
                kind: n.kind,
                key: n.key.clone(),
                title: n.title.clone(),
                description: n.description.clone(),
                version: n.version,
                path: folder_path(n.parent_id, by_id),
                children: if depth < 32 { build(Some(n.id), children, by_id, depth + 1) } else { Vec::new() },
            })
            .collect()
    }
    build(None, &children, &by_id, 0)
}

pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    // Последняя строка без перевода строки иначе считалась бы изменённой.
    let with_newline = |s: &str| if s.is_empty() || s.ends_with('\n') { s.to_string() } else { format!("{s}\n") };
    let (old, new) = (with_newline(old), with_newline(new));
    let diff = TextDiff::from_lines(&old, &new);
    let mut out = Vec::new();
    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => "equal",
            ChangeTag::Insert => "insert",
            ChangeTag::Delete => "delete",
        };
        out.push(DiffLine {
            tag,
            old_line: change.old_index().map(|i| i + 1),
            new_line: change.new_index().map(|i| i + 1),
            text: change.value().trim_end_matches('\n').to_string(),
        });
    }
    out
}

fn yaml_value(v: &str) -> String {
    let single_line = v.replace(['\r', '\n'], " ");
    if single_line.is_empty()
        || single_line.contains(": ")
        || single_line.starts_with(['"', '\'', '#', '-', '[', '{'])
        || single_line.trim() != single_line
    {
        format!("\"{}\"", single_line.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        single_line
    }
}

fn prompt_file(node: &PromptNode, path: &str) -> PromptFile {
    let key = node.key.clone().unwrap_or_default();
    let mut content = String::from("---\n");
    content.push_str(&format!("key: {}\n", yaml_value(&key)));
    content.push_str(&format!("title: {}\n", yaml_value(&node.title)));
    if !node.description.is_empty() {
        content.push_str(&format!("description: {}\n", yaml_value(&node.description)));
    }
    if !path.is_empty() {
        content.push_str(&format!("path: {}\n", yaml_value(path)));
    }
    content.push_str(&format!("version: {}\n", node.version));
    content.push_str("---\n\n");
    content.push_str(node.body.trim_end());
    content.push('\n');

    let dir: String = path.split('/').filter(|s| !s.is_empty()).map(sanitize_segment).collect::<Vec<_>>().join("/");
    let file = format!("{}.md", sanitize_segment(&key));
    PromptFile { path: if dir.is_empty() { file } else { format!("{dir}/{file}") }, content }
}

fn sanitize_segment(s: &str) -> String {
    let cleaned: String =
        s.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() { "_".into() } else { trimmed }
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
        v[1..v.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        v.to_string()
    }
}

/// Разбирает Markdown с front matter:
/// ```text
/// ---
/// key: rust_news_template
/// title: Rust — новостной пост
/// path: Новости/Rust
/// ---
/// текст промпта
/// ```
pub fn parse_prompt_markdown(content: &str) -> Result<ParsedPromptMarkdown, DomainError> {
    let content = content.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let rest = content
        .strip_prefix("---\n")
        .ok_or_else(|| DomainError::Validation("markdown must start with '---' front matter containing 'key:'".into()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| DomainError::Validation("front matter is not closed with '---'".into()))?;
    let header = &rest[..end];
    let body_start = rest[end + 4..].find('\n').map(|i| end + 4 + i + 1).unwrap_or(rest.len());
    let body = rest[body_start..].trim_matches('\n').to_string();

    let mut parsed = ParsedPromptMarkdown { body, ..Default::default() };
    for line in header.lines() {
        let Some((name, value)) = line.split_once(':') else { continue };
        let value = unquote(value);
        match name.trim() {
            "key" => parsed.key = value,
            "title" => parsed.title = Some(value).filter(|v| !v.is_empty()),
            "description" => parsed.description = Some(value),
            "path" => parsed.path = Some(value).filter(|v| !v.is_empty()),
            _ => {}
        }
    }
    if parsed.key.is_empty() {
        return Err(DomainError::Validation("front matter must contain 'key'".into()));
    }
    validate_key(&parsed.key)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn node(id: u128, parent: Option<u128>, kind: PromptKind, title: &str, key: Option<&str>) -> PromptNode {
        PromptNode {
            id: Uuid::from_u128(id),
            parent_id: parent.map(Uuid::from_u128),
            kind,
            key: key.map(String::from),
            title: title.into(),
            description: "Описание: с двоеточием".into(),
            body: "line1\nline2".into(),
            version: 3,
            position: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn markdown_roundtrip() {
        let nodes = [
            node(1, None, PromptKind::Folder, "Новости", None),
            node(2, Some(1), PromptKind::Folder, "Rust", None),
            node(3, Some(2), PromptKind::Prompt, "Rust: шаблон", Some("rust_news_template")),
        ];
        let by_id: HashMap<Uuid, &PromptNode> = nodes.iter().map(|n| (n.id, n)).collect();
        let file = prompt_file(&nodes[2], &folder_path(nodes[2].parent_id, &by_id));
        assert_eq!(file.path, "Новости/Rust/rust_news_template.md");

        let parsed = parse_prompt_markdown(&file.content).unwrap();
        assert_eq!(parsed.key, "rust_news_template");
        assert_eq!(parsed.title.as_deref(), Some("Rust: шаблон"));
        assert_eq!(parsed.description.as_deref(), Some("Описание: с двоеточием"));
        assert_eq!(parsed.path.as_deref(), Some("Новости/Rust"));
        assert_eq!(parsed.body, "line1\nline2");
    }

    #[test]
    fn tree_puts_folders_first() {
        let nodes = vec![
            node(1, None, PromptKind::Prompt, "a", Some("a_key")),
            node(2, None, PromptKind::Folder, "z", None),
            node(3, Some(2), PromptKind::Prompt, "inner", Some("inner")),
        ];
        let tree = build_tree(&nodes);
        assert_eq!(tree[0].title, "z");
        assert_eq!(tree[0].children[0].path, "z");
        assert_eq!(tree[1].title, "a");
    }

    #[test]
    fn diff_marks_changes() {
        let d = diff_lines("a\nb\nc\n", "a\nB\nc\n");
        let tags: Vec<_> = d.iter().map(|l| l.tag).collect();
        assert_eq!(tags, vec!["equal", "delete", "insert", "equal"]);

        let d = diff_lines("x\nlast", "y\nlast\nmore");
        let tags: Vec<_> = d.iter().map(|l| l.tag).collect();
        assert_eq!(tags, vec!["delete", "insert", "equal", "insert"]);
    }

    #[test]
    fn rejects_markdown_without_key() {
        assert!(parse_prompt_markdown("---\ntitle: x\n---\nbody").is_err());
        assert!(parse_prompt_markdown("no front matter").is_err());
    }
}
