#!/usr/bin/env bash
# Применяет миграции core-service к локальной БД (docker-compose.infra.yml)
# и готовит .sqlx/ в корне workspace — чтобы Dockerfile мог собираться с
# SQLX_OFFLINE=true (COPY .sqlx ./.sqlx), без подключения к БД на этапе сборки.
#
# ВАЖНО про наш проект (в отличие от шаблона, из которого этот скрипт взят):
#   - БД есть только у core-service. telegram-publisher и shared-contracts
#     к базе не обращаются, поэтому нет ни *_DATABASE_URL на каждый сервис,
#     ни фичи sqlx.toml [macros] database-url-var — она была нужна, когда
#     несколько сервисов с РАЗНЫМИ БД одновременно используют query!-макросы.
#   - Репозитории core-service (crates/core-service/src/infrastructure/db)
#     написаны через runtime API — `sqlx::query()` / `sqlx::query_as()` с
#     готовой SQL-строкой и `.bind(...)`, а НЕ через compile-time макросы
#     `sqlx::query!`/`query_as!`. Это осознанный выбор (см. README
#     core-service) — чтобы `cargo check` не требовал живой БД на этапе
#     сборки/CI. Из-за этого `cargo sqlx prepare` сейчас не найдёт ни одного
#     макроса и создаст пустой `.sqlx/` — это ОЖИДАЕМО, а не ошибка.
#     Скрипт оставлен рабочим на будущее: если где-то в коде появятся
#     query!-макросы (например, в Media Service или новом Publisher-е),
#     `cargo sqlx prepare` начнёт реально кэшировать их метаданные без каких-
#     либо дополнительных правок здесь.
#
# Запускать из корня монорепо после `docker compose -f docker-compose.infra.yml up -d`
set -euo pipefail

CORE_DB="postgres://postgres:postgres@localhost:5432/core_db"

echo "→ core-service: применяю миграции"
sqlx migrate run --source crates/core-service/migrations --database-url "$CORE_DB"

echo "→ генерирую .sqlx кэш для всего workspace (включая тесты)"
export DATABASE_URL="$CORE_DB"

# Гарантируем, что каталог существует даже если prepare не найдёт ни одного
# query!-макроса (см. пояснение выше) — иначе `COPY .sqlx ./.sqlx` в
# Dockerfile упадёт на отсутствующем пути.
mkdir -p .sqlx
touch .sqlx/.gitkeep

cargo sqlx prepare --workspace -- --all-targets

echo "✓ Готово. .sqlx/ создан в корне — закоммитьте его в git."