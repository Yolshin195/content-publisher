//! MCP-сервер (Model Context Protocol, транспорт Streamable HTTP) — через него Claude
//! работает с приложением как с коннектором: берёт промпты по ключу, предлагает посты,
//! записывает интересы автора, смотрит аналитику.
//!
//! Реализован минимальный stateless-вариант транспорта: `POST /mcp` принимает
//! JSON-RPC сообщение и сразу отвечает JSON (без SSE-потока), `GET /mcp` — 405.
//! Этого достаточно для инструментов (tools), которые здесь только и нужны.

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::application::ports::ArticleFilter;
use crate::application::prompt_service::PromptDraft;
use crate::application::SuggestionInput;
use crate::domain::{
    build_trends, ArticleUpdate, DomainError, InterestTopic, NewInterestSnapshot, PromptKind, SourceLink,
};
use crate::presentation::dto::ArticleResponse;
use crate::state::AppState;

const SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const DEFAULT_VERSION: &str = "2025-06-18";

pub async fn handle_get() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")], "MCP endpoint: use POST with JSON-RPC").into_response()
}

pub async fn handle_post(State(state): State<AppState>, _headers: HeaderMap, Json(message): Json<Value>) -> Response {
    match message {
        Value::Array(batch) => {
            let mut responses = Vec::new();
            for msg in batch {
                if let Some(resp) = handle_message(&state, msg).await {
                    responses.push(resp);
                }
            }
            if responses.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                Json(Value::Array(responses)).into_response()
            }
        }
        msg => match handle_message(&state, msg).await {
            Some(resp) => Json(resp).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    }
}

/// Возвращает `None` для уведомлений и ответов (у них нет `id` или нет `method`).
async fn handle_message(state: &AppState, msg: Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).map(str::to_string);
    let (Some(id), Some(method)) = (id, method) else { return None };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    let result = match method.as_str() {
        "initialize" => Ok(initialize(state, &params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(state, &params).await,
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err((-32601, format!("method not found: {other}"))),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

fn initialize(state: &AppState, params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(DEFAULT_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) { requested } else { DEFAULT_VERSION };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "content-publisher", "title": "Content Publisher", "version": env!("CARGO_PKG_VERSION") },
        "instructions": instructions(state),
    })
}

fn instructions(state: &AppState) -> String {
    format!(
        "Content Publisher — планировщик постов автора для Telegram-канала.\n\
         \n\
         Как работать:\n\
         1. Задания приходят в виде «возьми промпт <key>». Вызови get_prompt(key) и выполняй его \
            инструкции буквально: там стиль автора, формат поста и правила поиска информации. \
            Включения {{{{> other_key}}}} уже раскрыты.\n\
         2. Перед тем как предлагать посты, вызови list_posts (недавние посты и предложения), чтобы \
            не повторяться, и учитывай поле feedback у отклонённых (state=rejected) — это прямые \
            пожелания автора.\n\
         3. Готовый пост отправляй через create_suggestion. content_html — HTML-разметка Telegram: \
            только b, i, u, s, a, code, pre, blockquote, tg-spoiler; переносы строк — обычным \\n; \
            символы <, >, & в тексте экранируй (&lt; &gt; &amp;). Сервер проверит разметку и длину \
            (лимит 4096 видимых символов, с обложкой 1024) и вернёт ошибку, если что-то не так — исправь и повтори.\n\
         4. Указывай prompt_key (и extra_prompt_keys для вспомогательных промптов) — версии \
            промптов фиксируются в посте, автор сравнивает результат разных версий.\n\
         5. auto_publish=true + publish_at — пост сразу встанет в очередь публикации без ревью \
            (используй, только если промпт прямо это разрешает). Иначе пост попадёт автору во «Входящие».\n\
         6. После анализа чатов сохраняй интересы автора через record_interests.\n\
         \n\
         Время — RFC3339 с часовым поясом. Часовой пояс автора: {tz}. Сейчас: {now}.\n\
         Интерфейс: {base}",
        tz = state.config.timezone,
        now = Utc::now().with_timezone(&state.config.timezone).format("%Y-%m-%d %H:%M %:z"),
        base = state.config.public_base_url,
    )
}

fn tool(name: &str, title: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "title": title, "description": description, "inputSchema": schema })
}

fn tool_definitions() -> Vec<Value> {
    let source_schema = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": { "title": { "type": "string" }, "url": { "type": "string", "format": "uri" } },
            "required": ["url"]
        }
    });
    vec![
        tool(
            "get_prompt",
            "Получить промпт по ключу",
            "Возвращает текст промпта из справочника по его ключу (регистр не важен), например \
             rust_news_template. Включения {{> key}} раскрыты. Используй, когда в задании сказано \
             «возьми промпт/настройки <key>».",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Ключ промпта" },
                    "version": { "type": "integer", "description": "Конкретная версия (по умолчанию текущая)" },
                    "resolve": { "type": "boolean", "description": "Раскрыть {{> key}} (по умолчанию true)" }
                },
                "required": ["key"]
            }),
        ),
        tool(
            "list_prompts",
            "Справочник промптов",
            "Дерево промптов: папки, ключи, названия, описания и текущие версии.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "save_prompt",
            "Создать или изменить промпт",
            "Создаёт промпт с ключом или сохраняет новую версию существующего (старые версии \
             остаются в истории, автор видит diff). Меняй промпты только по просьбе автора.",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Латиница, цифры, _ - ." },
                    "body": { "type": "string", "description": "Полный новый текст промпта (Markdown)" },
                    "title": { "type": "string" },
                    "description": { "type": "string", "description": "Одна строка: для чего промпт" },
                    "folder_path": { "type": "string", "description": "Папка, например «Новости/Rust»" },
                    "change_note": { "type": "string", "description": "Что и зачем изменено" }
                },
                "required": ["key", "body", "change_note"]
            }),
        ),
        tool(
            "prompt_history",
            "История версий промпта",
            "Список версий промпта: номер, дата, автор изменения (user/claude), комментарий.",
            json!({ "type": "object", "properties": { "key": { "type": "string" } }, "required": ["key"] }),
        ),
        tool(
            "create_suggestion",
            "Предложить пост",
            "Создаёт пост-предложение для автора (попадает во «Входящие»), либо — с auto_publish — \
             сразу ставит его в очередь публикации на publish_at. Разметка — HTML Telegram.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Заголовок (публикуется жирной первой строкой)" },
                    "content_html": { "type": "string", "description": "Текст поста в HTML-разметке Telegram, переносы — \\n" },
                    "excerpt": { "type": "string", "description": "1–2 предложения для списка" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "cover_image_url": { "type": "string", "description": "Картинка-обложка (тогда лимит текста 1024)" },
                    "prompt_key": { "type": "string", "description": "Ключ промпта-шаблона, по которому написан пост" },
                    "extra_prompt_keys": { "type": "array", "items": { "type": "string" }, "description": "Другие использованные промпты" },
                    "rationale": { "type": "string", "description": "Почему автору стоит это опубликовать" },
                    "sources": source_schema,
                    "publish_at": { "type": "string", "format": "date-time", "description": "Предлагаемое время публикации, RFC3339" },
                    "auto_publish": { "type": "boolean", "description": "Сразу запланировать без ревью (только если промпт разрешает)" },
                    "target_ids": { "type": "array", "items": { "type": "string" }, "description": "Площадки; по умолчанию все активные" }
                },
                "required": ["title", "content_html"]
            }),
        ),
        tool(
            "list_posts",
            "Список постов",
            "Недавние посты и предложения (без полного текста): статус, теги, дата, промпт, отзыв \
             автора. Используй, чтобы не повторять темы и учитывать отзывы на отклонённые предложения.",
            json!({
                "type": "object",
                "properties": {
                    "state": { "type": "string", "description": "suggested, draft, scheduled, archived, rejected — можно через запятую" },
                    "tag": { "type": "string" },
                    "origin": { "type": "string", "enum": ["ai", "manual"] },
                    "prompt_key": { "type": "string" },
                    "query": { "type": "string", "description": "Поиск по заголовку и тексту" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
                }
            }),
        ),
        tool(
            "get_post",
            "Пост целиком",
            "Полный пост: текст, обоснование, источники, отзыв автора, статус публикации, проблемы разметки.",
            json!({ "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }),
        ),
        tool(
            "update_post",
            "Изменить пост",
            "Правит текст/заголовок/теги поста (например, по комментарию автора). Передавай только изменяемые поля.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "content_html": { "type": "string" },
                    "excerpt": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "rationale": { "type": "string" },
                    "sources": source_schema
                },
                "required": ["id"]
            }),
        ),
        tool(
            "schedule_post",
            "Запланировать публикацию",
            "Ставит пост в очередь публикации на указанное время. Делай это только по явной просьбе автора или промпта.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "publish_at": { "type": "string", "format": "date-time" },
                    "target_ids": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["id", "publish_at"]
            }),
        ),
        tool(
            "record_interests",
            "Сохранить интересы автора",
            "Сохраняет снимок интересов автора по итогам анализа чатов и диалогов: краткое резюме и \
             темы с весом 1–10. Отображается автору как «чем я интересуюсь сейчас».",
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "2–4 предложения: чем автор занят и интересуется" },
                    "topics": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "weight": { "type": "integer", "minimum": 1, "maximum": 10 },
                                "note": { "type": "string" }
                            },
                            "required": ["name", "weight"]
                        }
                    },
                    "period_from": { "type": "string", "format": "date-time" },
                    "period_to": { "type": "string", "format": "date-time" },
                    "source": { "type": "string", "description": "chats (по умолчанию) или другой источник" }
                },
                "required": ["summary", "topics"]
            }),
        ),
        tool(
            "get_interests",
            "Интересы автора",
            "Последний снимок интересов и тренды тем по нескольким последним снимкам.",
            json!({ "type": "object", "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 50 } } }),
        ),
        tool(
            "get_analytics",
            "Аналитика",
            "Сводка: сколько опубликовано, сколько предложений принято/отклонено, эффективность версий промптов, популярные теги.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_targets",
            "Площадки публикации",
            "Каналы, куда публикуются посты (id, название, активность).",
            json!({ "type": "object", "properties": {} }),
        ),
    ]
}

type RpcResult = Result<Value, (i64, String)>;

async fn call_tool(state: &AppState, params: &Value) -> RpcResult {
    let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "missing tool name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let outcome = match name {
        "get_prompt" => get_prompt(state, args).await,
        "list_prompts" => list_prompts(state).await,
        "save_prompt" => save_prompt(state, args).await,
        "prompt_history" => prompt_history(state, args).await,
        "create_suggestion" => create_suggestion(state, args).await,
        "list_posts" => list_posts(state, args).await,
        "get_post" => get_post(state, args).await,
        "update_post" => update_post(state, args).await,
        "schedule_post" => schedule_post(state, args).await,
        "record_interests" => record_interests(state, args).await,
        "get_interests" => get_interests(state, args).await,
        "get_analytics" => get_analytics(state).await,
        "list_targets" => list_targets(state).await,
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };
    Ok(match outcome {
        Ok(ToolOutput::Text(text)) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Ok(ToolOutput::Json(value)) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
            "structuredContent": value,
            "isError": false
        }),
        Err(message) => json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
    })
}

enum ToolOutput {
    Text(String),
    Json(Value),
}

type ToolResult = Result<ToolOutput, String>;

fn parse<T: for<'de> Deserialize<'de>>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}

fn domain_err(e: DomainError) -> String {
    e.to_string()
}

fn parse_uuid(s: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s.trim()).map_err(|_| format!("invalid id: {s}"))
}

fn parse_uuids(list: Vec<String>) -> Result<Vec<Uuid>, String> {
    list.iter().map(|s| parse_uuid(s)).collect()
}

fn local(state: &AppState, dt: Option<DateTime<Utc>>) -> Option<String> {
    dt.map(|d| d.with_timezone(&state.config.timezone).format("%Y-%m-%d %H:%M %:z").to_string())
}

fn post_url(state: &AppState, id: Uuid) -> String {
    format!("{}/articles/{id}", state.config.public_base_url)
}

fn compact_post(state: &AppState, a: &ArticleResponse) -> Value {
    json!({
        "id": a.id,
        "title": a.title,
        "state": a.state,
        "origin": a.origin,
        "tags": a.tags,
        "excerpt": a.excerpt,
        "scheduled_at": local(state, a.scheduled_at),
        "publication": a.publication.status,
        "published_at": local(state, a.publication.published_at),
        "prompt": a.prompt_key.as_ref().map(|k| format!("{k}@v{}", a.prompt_version.unwrap_or(0))),
        "feedback": a.feedback,
        "created_at": local(state, Some(a.created_at)),
        "url": post_url(state, a.id),
    })
}

fn full_post(state: &AppState, a: &ArticleResponse) -> Value {
    let mut v = compact_post(state, a);
    let obj = v.as_object_mut().expect("object");
    obj.insert("content_html".into(), json!(a.content_html));
    obj.insert("rationale".into(), json!(a.rationale));
    obj.insert("sources".into(), json!(a.sources));
    obj.insert("cover_image_url".into(), json!(a.cover_image_url));
    obj.insert("prompt_refs".into(), json!(a.prompt_refs));
    obj.insert("text_length".into(), json!(a.text_length));
    obj.insert("telegram_problems".into(), json!(a.telegram_problems));
    obj.insert("permalink".into(), json!(a.publication.permalink));
    v
}

#[derive(Deserialize)]
struct GetPromptArgs {
    key: String,
    version: Option<i32>,
    resolve: Option<bool>,
}

async fn get_prompt(state: &AppState, args: Value) -> ToolResult {
    let args: GetPromptArgs = parse(args)?;
    let svc = &state.prompt_service;
    if let Some(version) = args.version {
        let node = svc.get_by_key(&args.key).await.map_err(domain_err)?;
        let v = svc.version(node.id, version).await.map_err(domain_err)?;
        return Ok(ToolOutput::Text(format!("# {} (key: {}, версия {})\n\n{}", v.title, args.key, v.version, v.body)));
    }
    if args.resolve.unwrap_or(true) {
        let r = svc.resolve(&args.key).await.map_err(domain_err)?;
        let refs: Vec<String> = r.refs.iter().map(|x| format!("{}@v{}", x.key, x.version)).collect();
        let mut text = format!("# {} (key: {}, версия {})\n", r.title, r.key, r.version);
        if refs.len() > 1 {
            text.push_str(&format!("Использованы промпты: {}\n", refs.join(", ")));
        }
        for w in &r.warnings {
            text.push_str(&format!("⚠️ {w}\n"));
        }
        text.push('\n');
        text.push_str(&r.body);
        return Ok(ToolOutput::Text(text));
    }
    let node = svc.get_by_key(&args.key).await.map_err(domain_err)?;
    Ok(ToolOutput::Text(format!(
        "# {} (key: {}, версия {})\n\n{}",
        node.title,
        node.key.unwrap_or_default(),
        node.version,
        node.body
    )))
}

async fn list_prompts(state: &AppState) -> ToolResult {
    let tree = state.prompt_service.tree().await.map_err(domain_err)?;
    fn walk(nodes: &[crate::application::prompt_service::PromptTreeNode], depth: usize, out: &mut String) {
        for n in nodes {
            let indent = "  ".repeat(depth);
            match n.kind {
                PromptKind::Folder => out.push_str(&format!("{indent}📁 {}\n", n.title)),
                PromptKind::Prompt => {
                    out.push_str(&format!("{indent}- {} — {} (v{})", n.key.as_deref().unwrap_or("?"), n.title, n.version));
                    if !n.description.is_empty() {
                        out.push_str(&format!(": {}", n.description));
                    }
                    out.push('\n');
                }
            }
            walk(&n.children, depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(&tree, 0, &mut out);
    if out.is_empty() {
        out.push_str("Справочник промптов пуст.");
    }
    Ok(ToolOutput::Text(out))
}

#[derive(Deserialize)]
struct SavePromptArgs {
    key: String,
    body: String,
    title: Option<String>,
    description: Option<String>,
    folder_path: Option<String>,
    change_note: Option<String>,
}

async fn save_prompt(state: &AppState, args: Value) -> ToolResult {
    let a: SavePromptArgs = parse(args)?;
    let draft = PromptDraft {
        key: a.key,
        body: a.body,
        title: a.title,
        description: a.description,
        folder_path: a.folder_path,
        change_note: a.change_note,
    };
    let node = state.prompt_service.save_by_key(draft, "claude").await.map_err(domain_err)?;
    Ok(ToolOutput::Json(json!({
        "key": node.key,
        "version": node.version,
        "url": format!("{}/prompts?key={}", state.config.public_base_url, node.key.clone().unwrap_or_default()),
    })))
}

#[derive(Deserialize)]
struct KeyArgs {
    key: String,
}

async fn prompt_history(state: &AppState, args: Value) -> ToolResult {
    let a: KeyArgs = parse(args)?;
    let node = state.prompt_service.get_by_key(&a.key).await.map_err(domain_err)?;
    let versions = state.prompt_service.versions(node.id).await.map_err(domain_err)?;
    let list: Vec<Value> = versions
        .iter()
        .map(|v| {
            json!({
                "version": v.version,
                "author": v.author,
                "change_note": v.change_note,
                "created_at": local(state, Some(v.created_at)),
                "length": v.body.chars().count(),
            })
        })
        .collect();
    Ok(ToolOutput::Json(json!({ "key": node.key, "current_version": node.version, "versions": list })))
}

#[derive(Deserialize)]
struct SourceArg {
    #[serde(default)]
    title: String,
    url: String,
}

#[derive(Deserialize)]
struct CreateSuggestionArgs {
    title: String,
    content_html: String,
    excerpt: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    cover_image_url: Option<String>,
    prompt_key: Option<String>,
    #[serde(default)]
    extra_prompt_keys: Vec<String>,
    rationale: Option<String>,
    #[serde(default)]
    sources: Vec<SourceArg>,
    publish_at: Option<DateTime<Utc>>,
    #[serde(default)]
    auto_publish: bool,
    #[serde(default)]
    target_ids: Vec<String>,
}

async fn create_suggestion(state: &AppState, args: Value) -> ToolResult {
    let a: CreateSuggestionArgs = parse(args)?;
    let article = state
        .suggestion_service
        .create(SuggestionInput {
            title: a.title,
            content_html: a.content_html,
            excerpt: a.excerpt,
            tags: a.tags,
            cover_image_url: a.cover_image_url,
            prompt_key: a.prompt_key,
            extra_prompt_keys: a.extra_prompt_keys,
            rationale: a.rationale,
            sources: a.sources.into_iter().map(|s| SourceLink { title: s.title, url: s.url }).collect(),
            publish_at: a.publish_at,
            auto_publish: a.auto_publish,
            target_ids: parse_uuids(a.target_ids)?,
        })
        .await
        .map_err(domain_err)?;
    let resp = ArticleResponse::from(article);
    Ok(ToolOutput::Json(json!({
        "id": resp.id,
        "state": resp.state,
        "scheduled_at": local(state, resp.scheduled_at),
        "prompt_refs": resp.prompt_refs,
        "text_length": resp.text_length,
        "url": post_url(state, resp.id),
    })))
}

#[derive(Deserialize)]
struct ListPostsArgs {
    state: Option<String>,
    tag: Option<String>,
    origin: Option<String>,
    prompt_key: Option<String>,
    query: Option<String>,
    limit: Option<i64>,
}

async fn list_posts(state: &AppState, args: Value) -> ToolResult {
    let a: ListPostsArgs = parse(args)?;
    let filter = ArticleFilter {
        state: a.state,
        origin: a.origin,
        tag: a.tag,
        prompt_key: a.prompt_key,
        q: a.query,
        sort: Some("created".into()),
        page: 1,
        per_page: a.limit.unwrap_or(20).clamp(1, 100),
        ..Default::default()
    };
    let (articles, total) = state.article_service.list(filter).await.map_err(domain_err)?;
    let posts: Vec<Value> =
        articles.into_iter().map(ArticleResponse::from).map(|a| compact_post(state, &a)).collect();
    Ok(ToolOutput::Json(json!({ "total": total, "posts": posts })))
}

#[derive(Deserialize)]
struct IdArgs {
    id: String,
}

async fn get_post(state: &AppState, args: Value) -> ToolResult {
    let a: IdArgs = parse(args)?;
    let id = parse_uuid(&a.id)?;
    let article = state.article_service.get(id).await.map_err(domain_err)?;
    Ok(ToolOutput::Json(full_post(state, &ArticleResponse::from(article))))
}

#[derive(Deserialize)]
struct UpdatePostArgs {
    id: String,
    title: Option<String>,
    content_html: Option<String>,
    excerpt: Option<String>,
    tags: Option<Vec<String>>,
    rationale: Option<String>,
    sources: Option<Vec<SourceArg>>,
}

async fn update_post(state: &AppState, args: Value) -> ToolResult {
    let a: UpdatePostArgs = parse(args)?;
    let id = parse_uuid(&a.id)?;
    let article = state
        .article_service
        .update(
            id,
            ArticleUpdate {
                title: a.title,
                content_html: a.content_html,
                excerpt: a.excerpt.map(Some),
                cover_media_id: None,
                cover_image_url: None,
                tags: a.tags,
                rationale: a.rationale.map(Some),
                sources: a
                    .sources
                    .map(|s| s.into_iter().map(|s| SourceLink { title: s.title, url: s.url }).collect()),
            },
        )
        .await
        .map_err(domain_err)?;
    Ok(ToolOutput::Json(full_post(state, &ArticleResponse::from(article))))
}

#[derive(Deserialize)]
struct SchedulePostArgs {
    id: String,
    publish_at: DateTime<Utc>,
    #[serde(default)]
    target_ids: Vec<String>,
}

async fn schedule_post(state: &AppState, args: Value) -> ToolResult {
    let a: SchedulePostArgs = parse(args)?;
    let id = parse_uuid(&a.id)?;
    let tasks =
        state.scheduling_service.schedule(id, a.publish_at, parse_uuids(a.target_ids)?).await.map_err(domain_err)?;
    Ok(ToolOutput::Json(json!({
        "id": id,
        "scheduled_at": local(state, Some(a.publish_at)),
        "targets": tasks.iter().map(|t| t.target_id).collect::<Vec<_>>(),
        "url": post_url(state, id),
    })))
}

#[derive(Deserialize)]
struct TopicArg {
    name: String,
    weight: i32,
    note: Option<String>,
}

#[derive(Deserialize)]
struct RecordInterestsArgs {
    summary: String,
    topics: Vec<TopicArg>,
    period_from: Option<DateTime<Utc>>,
    period_to: Option<DateTime<Utc>>,
    source: Option<String>,
}

async fn record_interests(state: &AppState, args: Value) -> ToolResult {
    let a: RecordInterestsArgs = parse(args)?;
    let snap = state
        .interest_service
        .record(NewInterestSnapshot {
            source: a.source.unwrap_or_default(),
            summary: a.summary,
            topics: a.topics.into_iter().map(|t| InterestTopic { name: t.name, weight: t.weight, note: t.note }).collect(),
            period_from: a.period_from,
            period_to: a.period_to,
        })
        .await
        .map_err(domain_err)?;
    Ok(ToolOutput::Json(json!({ "id": snap.id, "topics": snap.topics.len(), "url": format!("{}/inbox", state.config.public_base_url) })))
}

#[derive(Deserialize)]
struct LimitArgs {
    limit: Option<i64>,
}

async fn get_interests(state: &AppState, args: Value) -> ToolResult {
    let a: LimitArgs = parse(args)?;
    let snaps = state.interest_service.recent(a.limit.unwrap_or(8)).await.map_err(domain_err)?;
    let trends = build_trends(&snaps);
    let latest = snaps.first().map(|s| {
        json!({
            "summary": s.summary,
            "topics": s.topics,
            "created_at": local(state, Some(s.created_at)),
        })
    });
    Ok(ToolOutput::Json(json!({
        "latest": latest,
        "snapshots": snaps.len(),
        "trends": trends.iter().take(20).map(|t| json!({
            "name": t.name, "total_weight": t.total_weight, "mentions": t.mentions,
            "last_weight": t.last_weight, "last_seen": local(state, Some(t.last_seen)),
        })).collect::<Vec<_>>(),
    })))
}

async fn get_analytics(state: &AppState) -> ToolResult {
    let o = state.analytics_service.overview(8).await.map_err(domain_err)?;
    Ok(ToolOutput::Json(json!({
        "total_posts": o.total_posts,
        "published_total": o.published_total,
        "published_last_30d": o.published_last_30d,
        "scheduled_upcoming": o.scheduled_upcoming,
        "suggestions": {
            "pending": o.suggestions_pending,
            "accepted": o.suggestions_accepted,
            "rejected": o.suggestions_rejected,
        },
        "failed_tasks": o.failed_tasks,
        "prompts": o.prompts,
        "top_tags": o.tags,
        "weeks": o.weeks,
    })))
}

async fn list_targets(state: &AppState) -> ToolResult {
    let targets = state.target_service.list().await.map_err(domain_err)?;
    Ok(ToolOutput::Json(json!(targets
        .into_iter()
        .map(|t| json!({
            "id": t.id, "platform": t.platform.as_str(), "name": t.display_name,
            "external_id": t.external_id, "is_active": t.is_active,
        }))
        .collect::<Vec<_>>())))
}
