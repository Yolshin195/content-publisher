# Content Publisher

Личный конвейер контента для Telegram-канала: Claude по расписанию собирает новости
по технологиям и идеи из ваших чатов и предлагает посты, вы просматриваете их,
правите и ставите в план, а сервис публикует их в нужное время.

```
Claude (задачи по расписанию) ──MCP──► core-service ──► telegram-publisher ──► Telegram
        ▲                                │  ▲
        └──── промпты по ключу ◄─────────┘  └── вы: Входящие, План, Редактор, Промпты
```

## Возможности

- **Входящие** — предложения Claude с обоснованием, источниками и версией промпта.
  Принять, отредактировать, запланировать или отклонить с причиной (Claude учитывает
  причины отказов). Сверху — «Сейчас вам интересно»: темы, которые Claude выделил из ваших чатов.
- **План** — календарь месяц / неделя / список в вашем часовом поясе; посты перетаскиваются
  между днями, черновики без даты — в боковой колонке.
- **Редактор поста** — открывается поверх любой страницы (и отдельной страницей):
  HTML-разметка Telegram с панелью форматирования, живое превью «как в Telegram»,
  счётчик лимита 4096/1024, проверка разметки, площадки, статусы публикации, повтор/отмена.
- **Промпты** — древовидный справочник. У каждого промпта есть **ключ**
  (`rust_news_template`), по которому его берёт Claude; общие части подключаются через
  `{{> author_style}}`. Каждое сохранение — новая версия с diff и откатом; вкладка
  «Посты по версиям» показывает, как меняется подача от версии к версии. Экспорт/импорт `.md`
  и синхронизация с git (`make prompts-pull`).
- **Аналитика** — публикации по неделям, доля принятых предложений по каждой версии
  промпта, теги, время выхода постов, ошибки публикации, история интересов.
- **Настройки** — каналы публикации и инструкция по подключению Claude.
- Тёмная тема, адаптивная вёрстка, горячие клавиши (`N` — новый пост, `Ctrl+S` — сохранить,
  `Ctrl+B/I/U/K` — форматирование).

## Состав

| Крейт | Что делает |
|---|---|
| `crates/core-service` | Посты, промпты, планирование, веб-интерфейс (Askama + Tailwind + Alpine.js), REST API, MCP-сервер для Claude |
| `crates/telegram-publisher` | Публикует пост в Telegram через бота по общему контракту `POST /publish` |
| `crates/shared-contracts` | Общие DTO между сервисами |

## Запуск в продакшене

1. Создать `.env` файлы:

   ```bash
   make env
   ```

2. Заполнить `crates/telegram-publisher/.env` (`TELEGRAM_BOT_TOKEN`; бот должен быть
   администратором канала) и `crates/core-service/.env`:

   | Переменная | Зачем |
   |---|---|
   | `ADMIN_PASSWORD` | пароль входа в интерфейс — **обязательно** |
   | `API_TOKEN` | токен для Claude (MCP) и REST API, например `openssl rand -hex 32` |
   | `PUBLIC_BASE_URL` | внешний адрес, например `https://publisher.example.com` (ссылки в ответах Claude, Secure-cookie) |
   | `APP_TIMEZONE` | ваш часовой пояс, например `Asia/Bangkok` или `Europe/Moscow` |
   | `SESSION_SECRET` | необязательно: секрет cookie-сессий (по умолчанию выводится из пароля) |

3. Поднять стек (Postgres + оба сервиса, миграции применяются автоматически):

   ```bash
   make up
   ```

4. Поставить перед сервисом HTTPS-прокси — Claude подключается к MCP только по HTTPS.
   Пример для Caddy:

   ```
   publisher.example.com {
       reverse_proxy localhost:8080
   }
   ```

5. Открыть интерфейс, войти и в **Настройках** добавить канал (`@username` или `-100…`).

Проверка живости: `GET /health` (проверяет и БД). Swagger UI: `/docs`.

## Подключение Claude

1. В claude.ai: **Настройки → Коннекторы → Добавить свой коннектор**, адрес:

   ```
   https://publisher.example.com/mcp?token=<API_TOKEN>
   ```

2. Попросить Claude создать задачи по расписанию (готовые тексты есть в интерфейсе,
   **Настройки → Claude**). Например:

   - каждый день в 06:00 — «Возьми промпт `news_pipeline` и выполни его для темы Rust:
     промпт поиска — `rust_news_research`, шаблон — `rust_news_template`. Опубликуй выпуск в 07:00»;
   - каждый вечер — «Возьми промпт `chat_digest` и выполни его».

Инструменты MCP:

| Инструмент | Назначение |
|---|---|
| `get_prompt`, `list_prompts`, `save_prompt`, `prompt_history` | промпты по ключу, их дерево и история |
| `create_suggestion` | предложить пост (или сразу запланировать с `auto_publish`) — разметка и длина проверяются сервером |
| `list_posts`, `get_post`, `update_post`, `schedule_post` | посты, чтобы не повторяться и учитывать отзывы |
| `record_interests`, `get_interests` | снимки «чем автор интересуется сейчас» |
| `get_analytics`, `list_targets` | сводка и каналы |

Тот же функционал доступен через REST (`Authorization: Bearer <API_TOKEN>`), см. `/docs`.

## Промпты

При первом запуске загружается стартовый набор (`crates/core-service/seed/prompts`):
`author_style`, `telegram_format`, `news_pipeline`, `rust_news_research`, `rust_news_template`,
`chat_digest`, `chat_post_template`. Их стоит сразу подогнать под себя — особенно `author_style`.

Чтобы хранить промпты в git и видеть изменения через `git diff`:

```bash
CP_URL=https://publisher.example.com CP_TOKEN=<API_TOKEN> make prompts-pull   # сервис → prompts/*.md
git diff prompts/ && git commit -am "Промпты: ..."
CP_URL=https://publisher.example.com CP_TOKEN=<API_TOKEN> make prompts-push   # prompts/*.md → сервис
```

## Разработка

```bash
make dev-infra-up                 # только Postgres
cargo run --bin core-service      # http://localhost:8080
make css                          # пересобрать Tailwind после правок шаблонов (нужен Node.js)
make test                         # юнит-тесты
```

Собранный `static/css/app.css` и `static/vendor/alpine.min.js` лежат в git, поэтому для
Docker-сборки Node.js не нужен. Без `ADMIN_PASSWORD` и `API_TOKEN` авторизация выключена —
так удобно локально, но не в продакшене.
