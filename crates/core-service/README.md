# Core Service

Хранение постов и промптов, планирование и запуск публикаций, веб-интерфейс,
REST API и MCP-сервер для Claude. Запуск, переменные окружения и подключение Claude —
в корневом `README.md`.

## Устройство

Гексагональная архитектура: домен и бизнес-логика не зависят от Axum и sqlx.

- `domain/` — `Article` (статусы `suggested → draft → scheduled`, `rejected`, `archived`;
  происхождение, теги, версии промптов, источники, отзыв автора), `PromptNode`/`PromptVersion`,
  `InterestSnapshot`, `PublicationTarget`, `PublicationTask`, проверка разметки Telegram
  (`telegram_html.rs`).
- `application/` — порты и сервисы: `ArticleService`, `SuggestionService` (цикл «Claude
  предлагает — автор решает»), `SchedulingService`, `PublicationOrchestrator` (одна логика
  публикации для планировщика и кнопки «Сейчас»), `PromptService` (дерево, `{{> key}}`,
  версии, diff, Markdown), `CalendarService` (дни в `APP_TIMEZONE`), `AnalyticsService`,
  `InterestService`.
- `infrastructure/` — репозитории на `sqlx` (runtime-запросы, без живой БД при сборке),
  HTTP-клиенты к Publisher и Media Service, интервальный планировщик.
- `presentation/` — REST (`api/`, OpenAPI через utoipa), MCP (`mcp.rs`, Streamable HTTP,
  stateless JSON-RPC), страницы (`web/` + `templates/`), авторизация (`auth.rs`:
  cookie-сессия по паролю и Bearer-токен).

Статика: `static/css/app.css` собирается Tailwind из `frontend/app.css` (`make css`),
`static/js/app.js` — Alpine-компоненты, `static/icons.svg` — спрайт иконок.

## Полезное

- `cargo run --bin print_openapi > openapi.json` — спецификация без запуска сервера и БД.
- Миграции (`migrations/`) применяются при старте.
- Стартовые промпты (`seed/prompts/*.md`) загружаются только в пустой справочник.
- В сетях без доступа к GitHub сборка `utoipa-swagger-ui` падает на скачивании архива —
  укажите локальный файл: `SWAGGER_UI_DOWNLOAD_URL=file:///path/to/swagger-ui-5.17.14.zip`.

## Известные упрощения

- Планировщик — фиксированный интервал (`SCHEDULER_TICK_INTERVAL_SECONDS`), а не cron.
- Обложка — прямая ссылка на картинку; загрузка через Media Service работает, только если он подключён.
- Просмотры постов Telegram Bot API не отдаёт, поэтому аналитика — по собственному конвейеру
  (публикации, решения по предложениям, промпты), без охватов.
