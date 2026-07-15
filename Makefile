# Makefile для Content Publisher — подготовка окружения и запуск через Podman.
# Требуются: podman (4+), podman-compose ИЛИ встроенный `podman compose`.
# Rust-тулчейн на хосте нужен только для `make sqlx-prepare` (см. ниже) —
# сама сборка и запуск сервисов идут внутри контейнеров.

SHELL := /usr/bin/env bash

# Используем podman-compose, если он установлен отдельным бинарником,
# иначе — встроенный провайдер `podman compose`.
COMPOSE := $(shell command -v podman-compose >/dev/null 2>&1 && echo podman-compose || echo "podman compose")

INFRA_FILE   := docker-compose.infra.yml
STACK_FILE   := docker-compose.yml

CORE_ENV     := crates/core-service/.env
TELEGRAM_ENV := crates/telegram-publisher/.env

.DEFAULT_GOAL := help

.PHONY: help
help: ## Показать список доступных команд
	@echo "Content Publisher — команды Makefile (Podman):"
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | 		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

## ── Подготовка окружения ─────────────────────────────────────────────────────

.PHONY: env
env: ## Создать .env файлы сервисов из .env.example, если их ещё нет
	@if [ ! -f $(CORE_ENV) ]; then 		cp crates/core-service/.env.example $(CORE_ENV); 		echo "→ создан $(CORE_ENV)"; 	else 		echo "→ $(CORE_ENV) уже существует, пропускаю"; 	fi
	@if [ ! -f $(TELEGRAM_ENV) ]; then 		cp crates/telegram-publisher/.env.example $(TELEGRAM_ENV); 		echo "→ создан $(TELEGRAM_ENV) — впишите в него реальный TELEGRAM_BOT_TOKEN!"; 	else 		echo "→ $(TELEGRAM_ENV) уже существует, пропускаю"; 	fi

.PHONY: sqlx-stub
sqlx-stub: ## Гарантировать наличие .sqlx/ (нужно для COPY .sqlx в Dockerfile)
	@mkdir -p .sqlx
	@touch .sqlx/.gitkeep
	@echo "→ .sqlx/ на месте (см. prepare-sqlx.sh — код использует runtime sqlx::query,"
	@echo "  поэтому кэш сейчас пуст, но COPY в Dockerfile не упадёт)"

.PHONY: sqlx-prepare
sqlx-prepare: ## Реально пересобрать .sqlx-кэш (нужны локальные cargo, sqlx-cli и поднятая инфра)
	@command -v sqlx >/dev/null 2>&1 || { echo "sqlx-cli не найден. Установите: cargo install sqlx-cli --no-default-features --features postgres,rustls"; exit 1; }
	./prepare-sqlx.sh

## ── Локальная разработка (сервисы через `cargo run`, БД — в Podman) ─────────

.PHONY: dev-infra-up
dev-infra-up: env ## Поднять только Postgres (для `cargo run` на хосте)
	$(COMPOSE) -f $(INFRA_FILE) up -d
	@echo "→ Postgres поднят. Дальше: ./prepare-sqlx.sh && cargo run --bin core-service"

.PHONY: dev-infra-down
dev-infra-down: ## Остановить Postgres, поднятый для локальной разработки
	$(COMPOSE) -f $(INFRA_FILE) down

## ── Полный стек в контейнерах ────────────────────────────────────────────────

.PHONY: build
build: env sqlx-stub ## Собрать образы core-service и telegram-publisher
	$(COMPOSE) -f $(STACK_FILE) build

.PHONY: up
up: env sqlx-stub ## Подготовить всё и запустить полный стек (Postgres + оба сервиса)
	$(COMPOSE) -f $(STACK_FILE) up -d --build
	@echo "→ core-service:       http://localhost:8080  (Swagger UI — /docs)"
	@echo "→ telegram-publisher: http://localhost:8081/health"

.PHONY: down
down: ## Остановить полный стек (контейнеры остаются, volume с БД сохраняется)
	$(COMPOSE) -f $(STACK_FILE) down

.PHONY: restart
restart: down up ## Перезапустить полный стек

.PHONY: logs
logs: ## Логи всех сервисов стека (Ctrl+C для выхода)
	$(COMPOSE) -f $(STACK_FILE) logs -f

.PHONY: ps
ps: ## Статус контейнеров стека
	$(COMPOSE) -f $(STACK_FILE) ps

## ── Обслуживание ─────────────────────────────────────────────────────────────

.PHONY: clean
clean: ## Остановить стек и удалить volume с данными Postgres (БД будет потеряна!)
	$(COMPOSE) -f $(STACK_FILE) down -v

.PHONY: fclean
fclean: clean ## clean + удалить собранные образы
	-podman rmi content-publisher/core-service content-publisher/telegram-publisher

.PHONY: check
check: ## Быстрая локальная проверка компиляции (нужен Rust-тулчейн на хосте)
	cargo check --workspace