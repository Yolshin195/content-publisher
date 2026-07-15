use std::collections::HashMap;

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

        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: std::env::var("SERVER_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8080),
            scheduler_tick_interval_secs: std::env::var("SCHEDULER_TICK_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            publish_max_attempts: std::env::var("PUBLISH_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            media_service_url: std::env::var("MEDIA_SERVICE_URL").ok(),
            publisher_registry,
        }
    }
}
