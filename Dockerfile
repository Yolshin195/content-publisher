# ─────────────────────────────────────────────────────────────────────────────
# Универсальный Dockerfile для любого сервиса Content Publisher workspace.
# Контекст сборки — КОРЕНЬ монорепо (Cargo.lock лежит там).
#
# Сборка:
#   docker build -f Dockerfile --build-arg APP_NAME=core-service -t content-publisher/core-service .
#   docker build -f Dockerfile --build-arg APP_NAME=telegram-publisher -t content-publisher/telegram-publisher .
#   podman build -f Dockerfile --build-arg APP_NAME=core-service -t content-publisher/core-service .
#   podman build -f Dockerfile --build-arg APP_NAME=telegram-publisher -t content-publisher/telegram-publisher .
#
# shared-contracts — библиотека без бинарника (только src/lib.rs), сама по себе
# не собирается и не запускается как APP_NAME, но участвует в графе зависимостей
# (path-зависимость и core-service, и telegram-publisher), поэтому её манифест и
# заглушка тоже обязательны на шаге кэширования зависимостей ниже.
#
# При добавлении нового сервиса в workspace (например, Publisher для VK/X или
# Media Service) — добавить его Cargo.toml и заглушку(и) исходников в блок
# кэширования зависимостей ниже (см. подводный камень playbook §16: "Docker
# build падает после добавления нового крейта").
# ─────────────────────────────────────────────────────────────────────────────

# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:1.96-slim-bookworm AS builder

ARG APP_NAME

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 1. Workspace-манифесты — Cargo.lock из корня монорепо
COPY Cargo.toml Cargo.lock ./
COPY crates/shared-contracts/Cargo.toml   ./crates/shared-contracts/Cargo.toml
COPY crates/core-service/Cargo.toml       ./crates/core-service/Cargo.toml
COPY crates/telegram-publisher/Cargo.toml ./crates/telegram-publisher/Cargo.toml
# При добавлении нового сервиса — добавить строку выше

# 2. Заглушки для ВСЕХ членов workspace — иначе cargo не разрешит граф зависимостей.
#    shared-contracts    — библиотека без бинарника (только lib.rs).
#    core-service        — бинарник + библиотека + дополнительный бинарник
#                           print_openapi (src/bin/*.rs) — заглушку нужно
#                           положить и под него.
#    telegram-publisher   — только бинарник (main.rs), без lib.rs.
RUN mkdir -p crates/shared-contracts/src \
             crates/core-service/src/bin \
             crates/telegram-publisher/src && \
    echo ''             > crates/shared-contracts/src/lib.rs           && \
    echo 'fn main() {}' > crates/core-service/src/main.rs              && \
    echo ''             > crates/core-service/src/lib.rs               && \
    echo 'fn main() {}' > crates/core-service/src/bin/print_openapi.rs && \
    echo 'fn main() {}' > crates/telegram-publisher/src/main.rs
# При добавлении нового сервиса — добавить mkdir + соответствующие заглушки выше

# 3. Кэш зависимостей только для нужного крейта (слой переиспользуется,
#    пока Cargo.toml целевого сервиса не меняется)
RUN cargo build --release -p ${APP_NAME} && rm -rf crates

# 4. Основная сборка — копируем реальные исходники целевого сервиса.
#    mkdir -p migrations/static гарантирует, что эти каталоги существуют даже
#    у сервисов без миграций/статики (сейчас — telegram-publisher), иначе COPY
#    в runtime-стадии ниже упадёт на отсутствующем пути.
ENV SQLX_OFFLINE=true
COPY .sqlx ./.sqlx
COPY crates ./crates
RUN touch crates/${APP_NAME}/src/main.rs && \
    if [ -f crates/${APP_NAME}/src/lib.rs ]; then touch crates/${APP_NAME}/src/lib.rs; fi && \
    mkdir -p crates/${APP_NAME}/migrations crates/${APP_NAME}/static && \
    cargo build --release -p ${APP_NAME}

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

ARG APP_NAME

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -ms /bin/bash appuser

USER appuser
WORKDIR /app

COPY --from=builder /app/target/release/${APP_NAME} ./service

# Миграции (core-service) sqlx::migrate! встраивает в бинарник ещё на этапе
# компиляции, поэтому каталог на рантайме строго не обязателен — копируется
# для консистентности между сервисами и на случай будущего перехода на
# runtime-миграции.
# Статика (CSS/JS календаря core-service) — наоборот, ОБЯЗАТЕЛЬНА в рантайме:
# раздаётся через tower-http ServeDir с диска, а не встраивается в бинарник.
# Для сервисов без миграций/статики (telegram-publisher, будущие Publisher-ы)
# builder-стадия гарантированно создала пустые каталоги — COPY не упадёт.
COPY --from=builder /app/crates/${APP_NAME}/migrations ./migrations
COPY --from=builder /app/crates/${APP_NAME}/static ./static

# Healthcheck читает PORT из окружения контейнера (задаётся в docker-compose
# для каждого сервиса отдельно) — поэтому используется CMD-SHELL, а не ARG,
# т.к. ARG фиксируется на build-time и не годится для общего образа.
# ВАЖНО: telegram-publisher уже отдаёт GET /health (200 OK). core-service пока
# такого эндпоинта не имеет — при первом деплое core-service нужно либо
# добавить в него /health, либо временно указать в docker-compose другой путь
# для проверки (например, "/docs").
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -f "http://localhost:${PORT:-8080}/health" || exit 1

ENTRYPOINT ["./service"]
