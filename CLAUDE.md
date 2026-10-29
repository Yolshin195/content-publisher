# Content Publisher — заметки для работы с репозиторием

Rust workspace (`crates/core-service`, `crates/telegram-publisher`, `crates/shared-contracts`),
Postgres, интерфейс на Askama + Tailwind v4 + Alpine.js. Язык интерфейса, комментариев и
сообщений об ошибках — русский.

## Команды

- `cargo test --workspace`, `cargo clippy --workspace --all-targets`
- `make css` — после правок `crates/core-service/templates/**` или `static/js/app.js`
  пересобрать `static/css/app.css` (классы Tailwind ищутся в шаблонах, JS и `src/**/*.rs`).
  Собранный CSS коммитится.
- Локальный запуск: Postgres из `compose.infra.yml`, затем `cargo run --bin core-service`.
- Если сборка `utoipa-swagger-ui` не может скачать архив с GitHub — собрать zip из npm-пакета
  `swagger-ui-dist@5.17.14` (каталог `swagger-ui-5.17.14/dist/`) и указать
  `SWAGGER_UI_DOWNLOAD_URL=file:///…/swagger-ui-5.17.14.zip`.

## Соглашения

- Гексагональная архитектура: `domain` → `application/ports` + сервисы → `infrastructure` → `presentation`.
  SQL — только в `infrastructure/db`, runtime `sqlx::query` (без макросов `query!`).
- Новая миграция — следующий номер в `crates/core-service/migrations`; старые не меняются.
- Любая новая возможность для Claude — одновременно REST-эндпоинт (с `utoipa::path`) и, если
  нужна в задачах по расписанию, инструмент в `presentation/mcp.rs`.
- Текст постов — HTML-разметка Telegram; проверка в `domain/telegram_html.rs` и зеркально в
  `static/js/app.js` (`tgProblems`) — меняйте обе.
- Время показывается и вводится в `APP_TIMEZONE`, хранится в UTC.
