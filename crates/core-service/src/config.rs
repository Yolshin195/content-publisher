use std::collections::HashMap;
use std::path::PathBuf;

use chrono_tz::Tz;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub scheduler_tick_interval_secs: u64,
    pub publish_max_attempts: i32,
    pub media_service_url: Option<String>,
    /// Реестр `platform -> base_url` Publisher-микросервисов (Telegram, VK, ...).
    pub publisher_registry: HashMap<String, String>,
    /// Часовой пояс автора: границы дней в календаре, время в интерфейсе, аналитика.
    pub timezone: Tz,
    /// Публичный адрес сервиса (для ссылок в ответах MCP/API), без `/` на конце.
    pub public_base_url: String,
    /// Пароль входа в веб-интерфейс. Без него (и без API_TOKEN) авторизация выключена.
    pub admin_password: Option<String>,
    /// Токен для API и MCP (`Authorization: Bearer ...` или `?token=...` для MCP).
    pub api_token: Option<String>,
    /// Секрет подписи сессионной cookie; по умолчанию выводится из пароля.
    pub session_secret: Option<String>,
    /// Ставить ли cookie с флагом Secure (true за HTTPS).
    pub cookie_secure: bool,
    pub static_dir: PathBuf,
    /// Метка версии статики для сброса кэша браузера (`?v=...`).
    pub asset_version: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        dotenvy::from_path(env_path).ok();

        let mut publisher_registry = HashMap::new();
        if let Ok(url) = std::env::var("PUBLISHER_TELEGRAM_URL") {
            publisher_registry.insert("telegram".to_string(), url);
        }
        if let Ok(url) = std::env::var("PUBLISHER_VK_URL") {
            publisher_registry.insert("vk".to_string(), url);
        }

        let server_port = std::env::var("SERVER_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8080);
        let timezone = std::env::var("APP_TIMEZONE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim().parse::<Tz>().unwrap_or_else(|_| panic!("APP_TIMEZONE '{v}' is not a valid IANA zone")))
            .unwrap_or(Tz::UTC);
        let public_base_url = non_empty("PUBLIC_BASE_URL")
            .unwrap_or_else(|| format!("http://localhost:{server_port}"))
            .trim_end_matches('/')
            .to_string();
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
            .unwrap_or_else(|| public_base_url.starts_with("https://"));

        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port,
            scheduler_tick_interval_secs: std::env::var("SCHEDULER_TICK_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            publish_max_attempts: std::env::var("PUBLISH_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            media_service_url: non_empty("MEDIA_SERVICE_URL"),
            publisher_registry,
            timezone,
            public_base_url,
            admin_password: non_empty("ADMIN_PASSWORD"),
            api_token: non_empty("API_TOKEN"),
            session_secret: non_empty("SESSION_SECRET"),
            cookie_secure,
            asset_version: String::new(),
            static_dir: resolve_static_dir(),
        }
        .with_asset_version()
    }

    fn with_asset_version(mut self) -> Self {
        let stamp: u64 = ["css/app.css", "js/app.js"]
            .iter()
            .filter_map(|f| std::fs::metadata(self.static_dir.join(f)).ok()?.modified().ok())
            .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .max()
            .unwrap_or(0);
        self.asset_version = format!("{}-{stamp:x}", env!("CARGO_PKG_VERSION"));
        self
    }

    pub fn auth_enabled(&self) -> bool {
        self.admin_password.is_some() || self.api_token.is_some()
    }
}

fn non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// `STATIC_DIR`, иначе `./static` (Docker-образ), иначе статика из исходников
/// крейта — чтобы `cargo run` работал из любого каталога.
fn resolve_static_dir() -> PathBuf {
    if let Some(dir) = non_empty("STATIC_DIR") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from("static");
    if local.join("css/app.css").exists() {
        return local;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
}
