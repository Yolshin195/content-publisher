//! Авторизация: один пароль для веб-интерфейса (подписанная cookie-сессия) и
//! токен для API/MCP (Claude). Безопасно по умолчанию: без `ADMIN_PASSWORD`/`API_TOKEN`
//! сервис не стартует (см. `AppConfig::validate_auth`); открытый режим — только
//! по явному флагу `AUTH_DISABLED=true` для локальной разработки.

use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use askama::Template;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::dto::{ErrorBody, ErrorResponse};
use crate::config::AppConfig;
use crate::state::AppState;

const SESSION_COOKIE: &str = "cp_session";
const SESSION_TTL_DAYS: i64 = 30;

/// Кто выполняет запрос — пишется в историю версий промптов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    User,
    Claude,
}

impl Actor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Actor::User => "user",
            Actor::Claude => "claude",
        }
    }
}

fn session_key(config: &AppConfig) -> Vec<u8> {
    let material = match (&config.session_secret, &config.admin_password, &config.api_token) {
        (Some(secret), _, _) => format!("secret:{secret}"),
        (None, Some(password), _) => format!("password:{password}"),
        (None, None, Some(token)) => format!("token:{token}"),
        (None, None, None) => "insecure-dev".to_string(),
    };
    Sha256::digest(material.as_bytes()).to_vec()
}

fn sign(config: &AppConfig, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(&session_key(config)).expect("hmac accepts any key length");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Сравнение секретов за постоянное время (через хэши одинаковой длины).
fn secret_eq(a: &str, b: &str) -> bool {
    let ha = Sha256::digest(a.as_bytes());
    let hb = Sha256::digest(b.as_bytes());
    ha.iter().zip(hb.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn new_session_cookie(config: &AppConfig) -> String {
    let expires = Utc::now().timestamp() + SESSION_TTL_DAYS * 86_400;
    let value = format!("{expires}.{}", sign(config, &expires.to_string()));
    format!(
        "{SESSION_COOKIE}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_TTL_DAYS * 86_400,
        if config.cookie_secure { "; Secure" } else { "" }
    )
}

fn clear_session_cookie(config: &AppConfig) -> String {
    format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        if config.cookie_secure { "; Secure" } else { "" }
    )
}

fn valid_session(config: &AppConfig, headers: &HeaderMap) -> bool {
    let Some(cookie_header) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else { return false };
    cookie_header.split(';').filter_map(|c| c.trim().strip_prefix(&format!("{SESSION_COOKIE}="))).any(|value| {
        let Some((expires, sig)) = value.split_once('.') else { return false };
        let Ok(expires_at) = expires.parse::<i64>() else { return false };
        expires_at > Utc::now().timestamp() && secret_eq(sig, &sign(config, expires))
    })
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(str::trim)
}

fn query_token(query: Option<&str>) -> Option<String> {
    query?.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == "token").then(|| v.to_string())
    })
}

fn valid_token(config: &AppConfig, token: Option<&str>) -> bool {
    match (&config.api_token, token) {
        (Some(expected), Some(given)) => secret_eq(expected, given),
        _ => false,
    }
}

fn is_public_path(path: &str) -> bool {
    path == "/health" || path == "/login" || path.starts_with("/static/") || path == "/favicon.ico"
}

fn is_machine_path(path: &str) -> bool {
    path.starts_with("/api/") || path == "/mcp" || path.starts_with("/mcp/")
}

pub async fn auth_middleware(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let config = &state.config;
    let path = req.uri().path().to_string();

    if !config.auth_enabled() {
        // AUTH_DISABLED=true (локальная разработка): Claude может представиться заголовком X-Actor: claude.
        let actor = match req.headers().get("x-actor").and_then(|v| v.to_str().ok()) {
            Some("claude") => Actor::Claude,
            _ if path.starts_with("/mcp") => Actor::Claude,
            _ => Actor::User,
        };
        req.extensions_mut().insert(actor);
        return next.run(req).await;
    }
    if is_public_path(&path) {
        req.extensions_mut().insert(Actor::User);
        return next.run(req).await;
    }

    let mcp_query_token = if path.starts_with("/mcp") { query_token(req.uri().query()) } else { None };
    let token = bearer_token(req.headers()).map(str::to_string).or(mcp_query_token);
    if valid_token(config, token.as_deref()) {
        req.extensions_mut().insert(Actor::Claude);
        return next.run(req).await;
    }
    if valid_session(config, req.headers()) {
        req.extensions_mut().insert(Actor::User);
        return next.run(req).await;
    }

    if is_machine_path(&path) {
        let body = ErrorResponse {
            error: ErrorBody {
                code: "UNAUTHORIZED".into(),
                message: "missing or invalid token: use 'Authorization: Bearer <API_TOKEN>'".into(),
            },
        };
        return (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], Json(body)).into_response();
    }

    let next_url = req.uri().path_and_query().map(|pq| pq.as_str().to_string()).unwrap_or_else(|| "/".into());
    Redirect::to(&format!("/login?next={}", urlencode(&next_url))).into_response()
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Разрешаем редирект только на локальные пути — защита от open redirect.
fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.starts_with("/\\") => n.to_string(),
        _ => "/".to_string(),
    }
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    next: String,
    error: bool,
    auth_enabled: bool,
}

#[derive(Deserialize)]
pub struct LoginQuery {
    next: Option<String>,
    error: Option<String>,
}

pub async fn login_page(State(state): State<AppState>, Query(q): Query<LoginQuery>) -> impl IntoResponse {
    let tpl = LoginTemplate {
        next: safe_next(q.next.as_deref()),
        error: q.error.is_some(),
        auth_enabled: state.config.auth_enabled(),
    };
    Html(tpl.render().unwrap_or_else(|e| format!("template error: {e}")))
}

#[derive(Deserialize)]
pub struct LoginForm {
    password: String,
    next: Option<String>,
}

pub async fn login_submit(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    let config = &state.config;
    let next = safe_next(form.next.as_deref());
    // Если отдельного пароля нет — входим по API-токену.
    let expected = config.admin_password.as_ref().or(config.api_token.as_ref());
    match expected {
        Some(expected) if secret_eq(expected, form.password.trim()) => {
            ([(header::SET_COOKIE, new_session_cookie(config))], Redirect::to(&next)).into_response()
        }
        Some(_) => {
            // Притормаживаем перебор пароля.
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            tracing::warn!("failed login attempt");
            Redirect::to(&format!("/login?error=1&next={}", urlencode(&next))).into_response()
        }
        None => Redirect::to(&next).into_response(),
    }
}

pub async fn logout(State(state): State<AppState>) -> Response {
    ([(header::SET_COOKIE, clear_session_cookie(&state.config))], Redirect::to("/login")).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_must_be_local() {
        assert_eq!(safe_next(Some("/calendar")), "/calendar");
        assert_eq!(safe_next(Some("//evil.com")), "/");
        assert_eq!(safe_next(Some("https://evil.com")), "/");
        assert_eq!(safe_next(None), "/");
    }

    #[test]
    fn query_token_is_parsed() {
        assert_eq!(query_token(Some("a=1&token=abc")).as_deref(), Some("abc"));
        assert_eq!(query_token(Some("a=1")), None);
    }

    #[test]
    fn secret_comparison() {
        assert!(secret_eq("abc", "abc"));
        assert!(!secret_eq("abc", "abd"));
    }
}
