# Core Service

Реализация Core Service согласно корневому `README.md` проекта: хранение статей,
целевых площадок публикации, задач публикации по площадкам, календарное
представление и REST API с OpenAPI-документацией. Публикация в конкретные соцсети
(Telegram и др.) выполняется отдельными Publisher-микросервисами по контракту
`shared-contracts::PublishRequest/PublishResult` — сами Publisher-сервисы в этом
этапе не реализуются, только контракт и HTTP-клиент к ним (`HttpPublicationGateway`).

## Запуск

1. Поднять PostgreSQL (например, `docker run -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=core_db -p 5432:5432 postgres:16`).
2. Скопировать `.env.example` в `.env` и поправить `DATABASE_URL` при необходимости.
3. Запустить сервис — миграции применяются автоматически при старте:

   ```bash
   cargo run --bin core-service
   ```

4. Открыть в браузере:
   - `http://localhost:8080/` — календарь (редирект на текущий месяц)
   - `http://localhost:8080/docs` — Swagger UI с OpenAPI-документацией
   - `http://localhost:8080/api-docs/openapi.json` — сама OpenAPI-спецификация

## Генерация OpenAPI-спецификации без БД

Вспомогательный бинарник печатает спецификацию в stdout, не поднимая сервер и не
требуя подключения к БД (полезно для CI / статической генерации документации):

```bash
cargo run --bin print_openapi > openapi.json
```

## Что реализовано

- Доменный слой (`domain`) — `Article`, `ArticleVideoLink` (через `VideoLink`),
  `PublicationTarget`, `PublicationTask`, `PublicationLog`, без зависимостей от
  Axum/sqlx.
- Порты (`application/ports`) и сервисы (`application/*_service.rs`,
  `PublicationOrchestrator`) — бизнес-логика не зависит от конкретной БД или
  транспорта.
- Репозитории на `sqlx`/PostgreSQL (`infrastructure/db`) — через `sqlx::query`
  (runtime, не compile-time проверяемые макросы), т.к. это не требует наличия
  живой БД на этапе сборки.
- `HttpPublicationGateway` — реализация единого контракта публикации поверх
  простого HTTP-вызова с реестром `platform -> base_url`.
- `HttpMediaClient` — клиент к Media Service; при отсутствии `MEDIA_SERVICE_URL`
  корректно возвращает ошибку, не блокируя остальной функционал.
- Cron-джоба (`infrastructure/scheduler`) — интервальный планировщик поверх
  `tokio::time::interval`, вызывающий `PublicationOrchestrator::tick()`.
- REST API (`presentation/api`) с аннотациями `utoipa::path` и агрегатором
  `ApiDoc` (`presentation/api/openapi.rs`), Swagger UI на `/docs`.
- HTML-интерфейс (`presentation/web`, Askama-шаблоны в `templates/`) — календарь
  месяц/неделя в стиле Google Calendar, форма создания/редактирования статьи с
  загрузкой картинки, несколькими ссылками на видео и выбором целевых площадок.

## Известные упрощения (сознательно оставлено на следующие итерации)

- Планировщик — фиксированный интервал, а не полноценные cron-выражения (легко
  заменить на `tokio-cron-scheduler`, не трогая `PublicationOrchestrator`).
- `cover_image_url` в `PublishRequest` пока всегда `None` — резолвинг реальной
  картинки через `MediaClient` по `article.cover_media_id` помечен `TODO` в
  `PublicationOrchestrator::process_task`.
- Авторизация не реализована (как и договаривались) — добавляется отдельным
  `Router::layer` поверх готового Core Service.

## Важное примечание о проверке компиляции

Этот код был написан и тщательно вычитан вручную (сигнатуры трейтов, импорты,
типы в SQL-биндингах, соответствие полей Askama-шаблонов), но в среде, где
готовился проект, не было доступа к тулчейну Rust (сетевые ограничения песочницы
не дали установить `rustc`/`cargo`), поэтому `cargo build`/`cargo check` ни разу
не запускались. Перед деплоем обязательно прогоните:

```bash
cargo check --workspace
cargo clippy --workspace
```

и пришлите вывод, если появятся ошибки — поправим.
